//! Rebuildable device-local vectors. SQLite filters exact source revisions before
//! a bounded streaming cosine scan. Only one vector and the top-k hits are kept
//! in memory; neither the webview nor Rust hydrates the complete vector corpus.
//! This uses the existing bundled SQLite and needs no loadable native extension.

use std::{cmp::Ordering, collections::BinaryHeap, path::Path};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{error::CommandError, task_cancellation::CancellationSignal};

use super::contracts::{bounded, invalid, is_hash};

const MAX_CACHE_BYTES: i64 = 512 * 1024 * 1024;
const MAX_QUERY_ENTRIES: usize = 250_000;
const MAX_VECTOR_DIMENSIONS: usize = 4096;

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

impl PartialEq for VectorHit {
    fn eq(&self, other: &Self) -> bool {
        self.distance.total_cmp(&other.distance) == Ordering::Equal && self.key.id == other.key.id
    }
}

impl Eq for VectorHit {}

impl PartialOrd for VectorHit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for VectorHit {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance
            .total_cmp(&other.distance)
            .then_with(|| self.key.id.cmp(&other.key.id))
    }
}

impl VectorIndex {
    pub(crate) fn open(path: &Path) -> Result<Self, CommandError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(CommandError::io)?;
        }
        Self::initialize(Connection::open(path).map_err(cache_error)?)
    }

    fn initialize(connection: Connection) -> Result<Self, CommandError> {
        connection
            .execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA busy_timeout = 3000;
                 PRAGMA cache_size = -8192;
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
                     entry_id TEXT PRIMARY KEY, source_id TEXT NOT NULL, content_hash TEXT NOT NULL
                 ) WITHOUT ROWID;",
            )
            .map_err(cache_error)?;
        let format = "semantic-vector-cache-v2-f32-le";
        let existing: Option<String> = connection
            .query_row(
                "SELECT value FROM semantic_cache_metadata WHERE key = 'format'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(cache_error)?;
        if existing.as_deref().is_some_and(|value| value != format) {
            return Err(invalid(
                "The semantic cache format changed. Clear its index and prepare the material again.",
            ));
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
                |row| row.get::<_, u32>(0).map(|value| value as usize),
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
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(cache_error)?;
        let stored: Option<u32> = transaction
            .query_row(
                "SELECT dimensions FROM embedding_spaces WHERE id = ?1",
                [space],
                |row| row.get(0),
            )
            .optional()
            .map_err(cache_error)?;
        if stored.is_some_and(|stored| stored as usize != dimensions) {
            return Err(invalid(
                "The embedding provider changed dimensions within an existing space.",
            ));
        }
        let bytes: i64 = transaction
            .query_row(
                "SELECT (page_count - freelist_count) * page_size
                 FROM pragma_page_count(), pragma_freelist_count(), pragma_page_size()",
                [],
                |row| row.get(0),
            )
            .map_err(cache_error)?;
        let addition =
            i64::try_from(entries.len().saturating_mul(dimensions * 4 + 512)).unwrap_or(i64::MAX);
        if bytes.saturating_add(addition) > MAX_CACHE_BYTES {
            return Err(CommandError::new(
                "semantic_cache_full",
                "The semantic index reached its 512 MiB safety limit. Clear unused indexes or prepare a smaller selection.",
            ));
        }
        transaction
            .execute(
                "INSERT OR IGNORE INTO embedding_spaces (id, dimensions) VALUES (?1, ?2)",
                params![space, dimensions as i64],
            )
            .map_err(cache_error)?;
        for entry in entries {
            transaction
                .execute(
                    "INSERT INTO embedding_entries (space_id, entry_id, source_id, content_hash, vector)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(space_id, entry_id) DO UPDATE SET source_id = excluded.source_id,
                         content_hash = excluded.content_hash, vector = excluded.vector",
                    params![space, entry.key.id, entry.key.source_id, entry.key.content_hash,
                        vector_bytes(&entry.vector)],
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
        super::runtime::check_canceled(cancellation)?;
        if eligible.len() > MAX_QUERY_ENTRIES || !(1..=64).contains(&limit) {
            return Err(invalid(
                "The semantic retrieval scope or result limit is too large.",
            ));
        }
        let Some(dimensions) = self.dimensions(space)? else {
            return Ok(Vec::new());
        };
        validate_vector(query, dimensions)?;
        let transaction = self.connection.transaction().map_err(cache_error)?;
        transaction
            .execute("DELETE FROM eligible_entries", [])
            .map_err(cache_error)?;
        {
            let mut insert = transaction
                .prepare(
                    "INSERT OR REPLACE INTO eligible_entries (entry_id, source_id, content_hash)
                     VALUES (?1, ?2, ?3)",
                )
                .map_err(cache_error)?;
            for key in eligible {
                super::runtime::check_canceled(cancellation)?;
                insert
                    .execute(params![key.id, key.source_id, key.content_hash])
                    .map_err(cache_error)?;
            }
        }
        transaction.commit().map_err(cache_error)?;
        let mut statement = self
            .connection
            .prepare(
                "SELECT e.entry_id, e.source_id, e.content_hash, e.vector
                 FROM embedding_entries e JOIN eligible_entries a
                   ON a.entry_id = e.entry_id AND a.source_id = e.source_id
                      AND a.content_hash = e.content_hash
                 WHERE e.space_id = ?1",
            )
            .map_err(cache_error)?;
        let mut rows = statement.query([space]).map_err(cache_error)?;
        let mut nearest = BinaryHeap::with_capacity(limit + 1);
        while let Some(row) = rows.next().map_err(cache_error)? {
            super::runtime::check_canceled(cancellation)?;
            let bytes = row.get_ref(3).map_err(cache_error)?;
            let bytes = bytes
                .as_blob()
                .map_err(|_| invalid("An indexed vector is not a binary vector."))?;
            let hit = VectorHit {
                key: IndexKey {
                    id: row.get(0).map_err(cache_error)?,
                    source_id: row.get(1).map_err(cache_error)?,
                    content_hash: row.get(2).map_err(cache_error)?,
                },
                distance: cosine_distance(query, bytes)?,
            };
            nearest.push(hit);
            if nearest.len() > limit {
                nearest.pop();
            }
        }
        Ok(nearest.into_sorted_vec())
    }

    /// Drain durable invalidation IDs in bounded batches. The library lock
    /// prevents a new source mutation between cache removal and acknowledgement.
    /// A crash after cache removal simply replays an idempotent deletion.
    pub(crate) fn evict_invalidated(
        &mut self,
        database: &crate::database::Database,
    ) -> Result<(), CommandError> {
        loop {
            let count = database.with_connection(|library| {
                let ids = {
                    let mut statement = library.prepare(
                        "SELECT source_id FROM knowledge_cache_evictions ORDER BY source_id LIMIT 256"
                    ).map_err(CommandError::database)?;
                    let rows = statement.query_map([], |row| row.get::<_, String>(0))
                        .map_err(CommandError::database)?
                        .collect::<Result<Vec<_>, _>>().map_err(CommandError::database)?;
                    rows
                };
                if ids.is_empty() { return Ok(0); }
                let json = serde_json::to_string(&ids).map_err(CommandError::serialization)?;
                self.connection.execute(
                    "DELETE FROM embedding_entries WHERE source_id IN (SELECT value FROM json_each(?1))",
                    [&json],
                ).map_err(cache_error)?;
                library.execute(
                    "DELETE FROM knowledge_cache_evictions WHERE source_id IN (SELECT value FROM json_each(?1))",
                    [&json],
                ).map_err(CommandError::database)?;
                Ok(ids.len())
            })?;
            if count < 256 {
                return Ok(());
            }
        }
    }
}

pub(crate) fn validate_vector(vector: &[f32], dimensions: usize) -> Result<(), CommandError> {
    if !(1..=MAX_VECTOR_DIMENSIONS).contains(&dimensions)
        || vector.len() != dimensions
        || vector.iter().any(|value| !value.is_finite())
        || !vector.iter().any(|value| *value != 0.0)
    {
        return Err(invalid(
            "An embedding was empty, nonfinite, zero, or had incompatible dimensions.",
        ));
    }
    Ok(())
}

fn vector_bytes(vector: &[f32]) -> Vec<u8> {
    vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn cosine_distance(query: &[f32], bytes: &[u8]) -> Result<f64, CommandError> {
    if bytes.len() != query.len() * 4 {
        return Err(invalid("An indexed vector has incompatible dimensions."));
    }
    let (mut dot, mut left_norm, mut right_norm) = (0.0_f64, 0.0_f64, 0.0_f64);
    for (left, chunk) in query.iter().zip(bytes.chunks_exact(4)) {
        let right = f64::from(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        if !right.is_finite() {
            return Err(invalid("An indexed vector has a nonfinite component."));
        }
        let left = f64::from(*left);
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        return Err(invalid("An indexed vector has zero norm."));
    }
    Ok((1.0 - dot / (left_norm.sqrt() * right_norm.sqrt())).clamp(0.0, 2.0))
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
    fn durable_eviction_removes_all_model_spaces_and_is_replay_safe() {
        let database = crate::database::Database::open_in_memory().unwrap();
        let mut index = VectorIndex::initialize(Connection::open_in_memory().unwrap()).unwrap();
        let space = super::super::contracts::hash("space");
        let entry = key("source:deleted", "old");
        index
            .write(
                &space,
                &[IndexWrite {
                    key: entry.clone(),
                    vector: vec![1.0, 0.0],
                }],
            )
            .unwrap();
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO knowledge_cache_evictions (source_id) VALUES (?1)",
                        [&entry.source_id],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();
        index.evict_invalidated(&database).unwrap();
        assert!(!index.contains(&space, &entry).unwrap());
        index.evict_invalidated(&database).unwrap();
    }

    #[test]
    fn vectors_reject_invalid_components_and_dimensions() {
        assert!(validate_vector(&[1.0, f32::NAN], 2).is_err());
        assert!(validate_vector(&[0.0, 0.0], 2).is_err());
        assert!(validate_vector(&[1.0], 2).is_err());
        assert!(validate_vector(&[1.0, 0.0], 2).is_ok());
        assert!(cosine_distance(&[1.0, 0.0], &vector_bytes(&[f32::INFINITY, 0.0])).is_err());
    }

    #[test]
    fn semantic_search_filters_scope_and_stale_hashes_before_ranking() {
        let mut index = VectorIndex::initialize(Connection::open_in_memory().unwrap()).unwrap();
        let space = super::super::contracts::hash("test-model-space");
        let first = key("source:first", "old");
        let second = key("source:second", "current");
        index
            .write(
                &space,
                &[
                    IndexWrite {
                        key: first.clone(),
                        vector: vec![1.0, 0.0],
                    },
                    IndexWrite {
                        key: second.clone(),
                        vector: vec![0.9, 0.1],
                    },
                ],
            )
            .unwrap();
        let cancellation = CancellationSignal::new();
        let hits = index
            .search(
                &space,
                &[1.0, 0.0],
                &[key("source:first", "new"), second.clone()],
                8,
                &cancellation,
            )
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key.id, second.id);
        assert!(hits[0].distance < 0.1);
        let different_space = super::super::contracts::hash("different-model");
        assert!(index
            .search(&different_space, &[1.0, 0.0], &[first], 8, &cancellation)
            .unwrap()
            .is_empty());
        cancellation.cancel();
        assert!(index
            .search(&space, &[1.0, 0.0], &[], 8, &cancellation)
            .is_err());
    }

    #[test]
    fn top_k_is_nearest_first_with_stable_ties() {
        let mut index = VectorIndex::initialize(Connection::open_in_memory().unwrap()).unwrap();
        let space = super::super::contracts::hash("ordered");
        let entries = [
            IndexWrite {
                key: key("b", "v1"),
                vector: vec![1.0, 0.0],
            },
            IndexWrite {
                key: key("a", "v1"),
                vector: vec![1.0, 0.0],
            },
            IndexWrite {
                key: key("c", "v1"),
                vector: vec![0.0, 1.0],
            },
        ];
        index.write(&space, &entries).unwrap();
        let eligible = entries
            .iter()
            .map(|entry| entry.key.clone())
            .collect::<Vec<_>>();
        let hits = index
            .search(
                &space,
                &[1.0, 0.0],
                &eligible,
                2,
                &CancellationSignal::new(),
            )
            .unwrap();
        assert_eq!(
            hits.iter()
                .map(|hit| hit.key.id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert!(index
            .write(
                &space,
                &[IndexWrite {
                    key: key("d", "v1"),
                    vector: vec![1.0]
                }]
            )
            .is_err());
    }
}
