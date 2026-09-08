//! Rebuildable, device-local sqlite-vec cache. Only IDs, hashes and vectors live
//! here; authored material and source provenance remain in the library database.
//! Scoped distance queries do not load the vector corpus into Rust or JavaScript.

use std::{
    ffi::{c_char, c_int},
    path::Path,
    sync::OnceLock,
};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{error::CommandError, task_cancellation::CancellationSignal};

use super::contracts::{bounded, invalid, is_hash};

const MAX_CACHE_BYTES: i64 = 512 * 1024 * 1024;
const MAX_QUERY_ENTRIES: usize = 1_700_000;
const MAX_VECTOR_DIMENSIONS: usize = 4096;
static EXTENSION: OnceLock<Result<(), String>> = OnceLock::new();

type SqliteExtensionEntry = unsafe extern "C" fn(
    *mut rusqlite::ffi::sqlite3,
    *mut *mut c_char,
    *const rusqlite::ffi::sqlite3_api_routines,
) -> c_int;

pub(crate) struct VectorIndex {
    connection: Connection,
}

#[derive(Clone, Debug)]
pub(crate) struct IndexKey {
    pub id: String,
    pub source_id: String,
    pub content_hash: String,
}

pub(crate) struct IndexWrite {
    pub key: IndexKey,
    pub vector: Vec<f32>,
}

#[derive(Clone, Debug)]
pub(crate) struct VectorHit {
    pub key: IndexKey,
    pub distance: f64,
}

impl VectorIndex {
    pub(crate) fn open(path: &Path) -> Result<Self, CommandError> {
        register_extension()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(CommandError::io)?;
        }
        let connection = Connection::open(path).map_err(cache_error)?;
        Self::initialize(connection)
    }

    fn initialize(connection: Connection) -> Result<Self, CommandError> {
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA busy_timeout = 3000;
                 PRAGMA cache_size = -16384;
                 PRAGMA mmap_size = 0;
                 PRAGMA foreign_keys = ON;
                 CREATE TABLE IF NOT EXISTS semantic_cache_metadata (
                     key TEXT PRIMARY KEY, value TEXT NOT NULL
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS embedding_spaces (
                     id TEXT PRIMARY KEY CHECK(length(id) = 64),
                     dimensions INTEGER NOT NULL CHECK(dimensions BETWEEN 1 AND 4096)
                 ) STRICT;
                 CREATE TABLE IF NOT EXISTS embedding_entries (
                     space_id TEXT NOT NULL REFERENCES embedding_spaces(id) ON DELETE CASCADE,
                     entry_id TEXT NOT NULL CHECK(length(entry_id) BETWEEN 1 AND 128),
                     source_id TEXT NOT NULL CHECK(length(source_id) = 32),
                     content_hash TEXT NOT NULL CHECK(length(content_hash) = 64),
                     vector BLOB NOT NULL CHECK(length(vector) BETWEEN 4 AND 16384 AND length(vector) % 4 = 0),
                     PRIMARY KEY(space_id, entry_id)
                 ) STRICT;
                 CREATE INDEX IF NOT EXISTS embedding_entries_source_idx ON embedding_entries(source_id);
                 CREATE TEMP TABLE IF NOT EXISTS eligible_entries (
                     entry_id TEXT PRIMARY KEY, content_hash TEXT NOT NULL
                 ) WITHOUT ROWID;",
            )
            .map_err(cache_error)?;
        let version: String = connection
            .query_row("SELECT vec_version()", [], |row| row.get(0))
            .map_err(cache_error)?;
        if version != "v0.1.9" {
            return Err(invalid("The semantic index extension does not match the audited build."));
        }
        let format = if cfg!(target_endian = "little") {
            "semantic-vector-cache-v1-le"
        } else {
            "semantic-vector-cache-v1-be"
        };
        let existing: Option<String> = connection
            .query_row(
                "SELECT value FROM semantic_cache_metadata WHERE key = 'format'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(cache_error)?;
        if existing.as_deref().is_some_and(|value| value != format) {
            return Err(invalid("The semantic cache format changed. Clear its index and prepare the material again."));
        }
        connection
            .execute(
                "INSERT OR IGNORE INTO semantic_cache_metadata (key, value) VALUES ('format', ?1)",
                [format],
            )
            .map_err(cache_error)?;
        Ok(Self { connection })
    }

    pub(crate) fn contains(&self, space: &str, key: &IndexKey) -> Result<bool, CommandError> {
        self.connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM embedding_entries
                 WHERE space_id = ?1 AND entry_id = ?2 AND content_hash = ?3 AND source_id = ?4)",
                params![space, key.id, key.content_hash, key.source_id],
                |row| row.get(0),
            )
            .map_err(cache_error)
    }

    pub(crate) fn dimensions(&self, space: &str) -> Result<Option<usize>, CommandError> {
        self.connection
            .query_row(
                "SELECT dimensions FROM embedding_spaces WHERE id = ?1",
                [space],
                |row| row.get::<_, usize>(0),
            )
            .optional()
            .map_err(cache_error)
    }

    pub(crate) fn write(
        &mut self,
        space: &str,
        entries: &[IndexWrite],
    ) -> Result<(), CommandError> {
        if !is_hash(space) || entries.len() > 64 {
            return Err(invalid("The embedding space or cache batch is invalid."));
        }
        let Some(first) = entries.first() else {
            return Ok(());
        };
        let dimensions = first.vector.len();
        for entry in entries {
            validate_vector(&entry.vector, dimensions)?;
            bounded(&entry.key.id, 128, "retrieval entry ID", false)?;
            crate::util::validate_entity_id(&entry.key.source_id, "retrieval source")?;
            if !is_hash(&entry.key.content_hash) {
                return Err(invalid("The retrieval content fingerprint is invalid."));
            }
        }
        if self.dimensions(space)?.is_some_and(|stored| stored != dimensions) {
            return Err(invalid("The embedding provider changed vector dimensions within an existing space."));
        }
        let bytes: i64 = self.connection
            .query_row("SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()", [], |row| row.get(0))
            .map_err(cache_error)?;
        let addition = i64::try_from(entries.len().saturating_mul(dimensions).saturating_mul(4))
            .unwrap_or(i64::MAX);
        if bytes.saturating_add(addition) > MAX_CACHE_BYTES {
            return Err(CommandError::new(
                "semantic_cache_full",
                "The semantic index reached its 512 MiB safety limit. Clear unused indexes or prepare a smaller selection.",
            ));
        }
        let transaction = self.connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(cache_error)?;
        transaction
            .execute(
                "INSERT OR IGNORE INTO embedding_spaces (id, dimensions) VALUES (?1, ?2)",
                params![space, dimensions],
            )
            .map_err(cache_error)?;
        for entry in entries {
            let bytes = vector_bytes(&entry.vector);
            transaction
                .execute(
                    "INSERT INTO embedding_entries (space_id, entry_id, source_id, content_hash, vector)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(space_id, entry_id) DO UPDATE SET source_id = excluded.source_id,
                         content_hash = excluded.content_hash, vector = excluded.vector",
                    params![space, entry.key.id, entry.key.source_id, entry.key.content_hash, bytes],
                )
                .map_err(cache_error)?;
        }
        transaction.commit().map_err(cache_error)
    }

    pub(crate) fn search(
        &mut self,
        space: &str,
        query: &[f32],
        eligible: &[IndexKey],
        limit: usize,
        cancellation: &CancellationSignal,
    ) -> Result<Vec<VectorHit>, CommandError> {
        if eligible.len() > MAX_QUERY_ENTRIES || !(1..=64).contains(&limit) {
            return Err(invalid("The semantic retrieval scope or result limit is too large."));
        }
        let Some(dimensions) = self.dimensions(space)? else {
            return Ok(Vec::new());
        };
        validate_vector(query, dimensions)?;
        let signal = cancellation.clone();
        self.connection.progress_handler(1000, Some(move || signal.is_canceled()));
        let result = self.search_inner(space, query, eligible, limit);
        self.connection.progress_handler(0, None::<fn() -> bool>);
        if cancellation.is_canceled() {
            return Err(CommandError::new("ai_canceled", "Semantic retrieval was canceled."));
        }
        result
    }

    fn search_inner(
        &mut self,
        space: &str,
        query: &[f32],
        eligible: &[IndexKey],
        limit: usize,
    ) -> Result<Vec<VectorHit>, CommandError> {
        let transaction = self.connection.transaction().map_err(cache_error)?;
        transaction.execute("DELETE FROM eligible_entries", []).map_err(cache_error)?;
        {
            let mut insert = transaction
                .prepare("INSERT OR REPLACE INTO eligible_entries (entry_id, content_hash) VALUES (?1, ?2)")
                .map_err(cache_error)?;
            for key in eligible {
                insert.execute(params![key.id, key.content_hash]).map_err(cache_error)?;
            }
        }
        transaction.commit().map_err(cache_error)?;
        let mut statement = self.connection
            .prepare(
                "SELECT e.entry_id, e.source_id, e.content_hash,
                        vec_distance_cosine(e.vector, ?1) AS distance
                 FROM embedding_entries e JOIN eligible_entries a
                   ON a.entry_id = e.entry_id AND a.content_hash = e.content_hash
                 WHERE e.space_id = ?2
                 ORDER BY distance, e.entry_id LIMIT ?3",
            )
            .map_err(cache_error)?;
        let hits = statement
            .query_map(params![vector_bytes(query), space, limit], |row| {
                Ok(VectorHit {
                    key: IndexKey {
                        id: row.get(0)?,
                        source_id: row.get(1)?,
                        content_hash: row.get(2)?,
                    },
                    distance: row.get(3)?,
                })
            })
            .map_err(cache_error)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(cache_error)?;
        Ok(hits)
    }

    pub(crate) fn clear(&mut self) -> Result<(), CommandError> {
        self.connection.execute_batch("DELETE FROM embedding_entries; DELETE FROM embedding_spaces; VACUUM;")
            .map_err(cache_error)
    }
}

fn register_extension() -> Result<(), CommandError> {
    let registered = EXTENSION.get_or_init(|| {
        // SAFETY: sqlite-vec's statically linked entry uses SQLite's documented
        // extension ABI. Its Rust binding intentionally declares an untyped
        // symbol; this conversion supplies the actual ABI expected by SQLite.
        // No library path or user-controlled executable code can be loaded.
        let entry: SqliteExtensionEntry = unsafe {
            std::mem::transmute::<*const (), SqliteExtensionEntry>(
                sqlite_vec::sqlite3_vec_init as *const (),
            )
        };
        // SAFETY: `entry` is the statically linked initializer with process-long
        // lifetime; registration is thread-safe and performed exactly once.
        let status = unsafe { rusqlite::ffi::sqlite3_auto_extension(Some(entry)) };
        if status == rusqlite::ffi::SQLITE_OK {
            Ok(())
        } else {
            Err(format!("SQLite vector extension registration failed ({status})."))
        }
    });
    registered.clone().map_err(|message| CommandError::new("semantic_index_unavailable", message))
}

pub(crate) fn validate_vector(vector: &[f32], dimensions: usize) -> Result<(), CommandError> {
    if !(1..=MAX_VECTOR_DIMENSIONS).contains(&dimensions)
        || vector.len() != dimensions
        || vector.iter().any(|value| !value.is_finite())
        || !vector.iter().any(|value| *value != 0.0)
    {
        return Err(invalid("An embedding was empty, nonfinite, zero, or had incompatible dimensions."));
    }
    Ok(())
}

fn vector_bytes(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|value| value.to_ne_bytes()).collect()
}

fn cache_error(_error: rusqlite::Error) -> CommandError {
    CommandError::new(
        "semantic_index_unavailable",
        "The local semantic index could not be read or updated. The study library is unchanged; clear and prepare the index again.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(id: &str, revision: &str) -> IndexKey {
        IndexKey {
            id: id.to_string(),
            source_id: "a".repeat(32),
            content_hash: super::super::contracts::hash(revision),
        }
    }

    #[test]
    fn vectors_reject_invalid_components_and_dimensions() {
        assert!(validate_vector(&[1.0, f32::NAN], 2).is_err());
        assert!(validate_vector(&[0.0, 0.0], 2).is_err());
        assert!(validate_vector(&[1.0], 2).is_err());
        assert!(validate_vector(&[1.0, 0.0], 2).is_ok());
    }

    #[test]
    fn semantic_search_filters_scope_and_stale_hashes_before_ranking() {
        register_extension().unwrap();
        let mut index = VectorIndex::initialize(Connection::open_in_memory().unwrap()).unwrap();
        let space = super::super::contracts::hash("test-model-space");
        let first = key("source:first", "old");
        let second = key("source:second", "current");
        index.write(&space, &[
            IndexWrite { key: first.clone(), vector: vec![1.0, 0.0] },
            IndexWrite { key: second.clone(), vector: vec![0.9, 0.1] },
        ]).unwrap();
        let cancellation = CancellationSignal::new();
        let hits = index.search(&space, &[1.0, 0.0], &[key("source:first", "new"), second.clone()], 8, &cancellation).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key.id, second.id);
        assert!(hits[0].distance < 0.1);
        let different_space = super::super::contracts::hash("different-model");
        assert!(index.search(&different_space, &[1.0, 0.0], &[first], 8, &cancellation).unwrap().is_empty());
    }
}
