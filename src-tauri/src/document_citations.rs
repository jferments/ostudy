//! Durable source-citation snapshots for document-generated learning artifacts.
//!
//! Citation rows deliberately do not foreign-key back to `documents`: deleting a
//! managed source removes document-owned processing state while generated decks,
//! notes, and their exact source locators remain independently attributable.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    database::Database, document_domain::SourceLocator, error::CommandError,
    util::validate_entity_id,
};

pub(crate) const SOURCE_CITATION_SCHEMA_VERSION: i64 = 1;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SourceCitationDto {
    pub citation_id: String,
    pub schema_version: i64,
    pub source_document_id: String,
    pub source_identity: String,
    pub source_fingerprint_sha256: String,
    pub source_title: String,
    pub original_filename: String,
    pub source_format: String,
    pub source_media_type: String,
    pub source_unit_kind: String,
    pub source_unit_count: i64,
    pub source_removed: bool,
    pub source_locator: Option<SourceLocator>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreservedDocumentCitations {
    pub active_deck_count: i64,
    pub note_count: i64,
}

/// Create or refresh the document-owned snapshot while the source still exists.
///
/// The snapshot ID intentionally equals the stable document ID. This keeps the
/// migration/backfill deterministic and avoids a second identity allocator.
pub(crate) fn upsert_document_snapshot(
    connection: &Connection,
    document_id: &str,
    now: i64,
) -> Result<String, CommandError> {
    validate_entity_id(document_id, "document")?;
    let changed = connection
        .execute(
            "INSERT INTO source_citation_snapshots (
                 id, schema_version, source_document_id, source_identity,
                 source_fingerprint_sha256, source_title, original_filename,
                 source_format, source_media_type, source_unit_kind,
                 source_unit_count, source_removed, created_at, updated_at
             )
             SELECT
                 id, ?2, id, 'sha256:' || sha256, sha256, title,
                 original_filename, source_format, source_media_type,
                 source_unit_kind, source_unit_count, 0, created_at,
                 MAX(updated_at, ?3)
             FROM documents
             WHERE id = ?1
             ON CONFLICT(source_document_id) DO UPDATE SET
                 schema_version = excluded.schema_version,
                 source_identity = excluded.source_identity,
                 source_fingerprint_sha256 = excluded.source_fingerprint_sha256,
                 source_title = excluded.source_title,
                 original_filename = excluded.original_filename,
                 source_format = excluded.source_format,
                 source_media_type = excluded.source_media_type,
                 source_unit_kind = excluded.source_unit_kind,
                 source_unit_count = excluded.source_unit_count,
                 source_removed = 0,
                 updated_at = MAX(
                     source_citation_snapshots.updated_at,
                     excluded.updated_at
                 )",
            params![document_id, SOURCE_CITATION_SCHEMA_VERSION, now],
        )
        .map_err(|error| {
            CommandError::database_at(
                "document_citation_snapshot_write_failed",
                format!("document={document_id}"),
                error,
            )
        })?;
    if changed != 1 {
        return Err(CommandError::new(
            "document_not_found",
            "That document is no longer in the document library.",
        ));
    }
    Ok(document_id.to_string())
}

/// Materialize every surviving deck/note relationship and mark the source
/// unavailable in the same transaction that will delete the document row.
pub(crate) fn preserve_document_citations_before_delete(
    connection: &Connection,
    document_id: &str,
    now: i64,
) -> Result<PreservedDocumentCitations, CommandError> {
    let citation_id = upsert_document_snapshot(connection, document_id, now)?;

    connection
        .execute(
            "INSERT OR IGNORE INTO deck_source_citations (
                 deck_id, citation_id, created_at
             )
             SELECT relation.deck_id, ?2, ?3
             FROM (
                 SELECT deck_id
                 FROM document_decks
                 WHERE document_id = ?1
                 UNION
                 SELECT generated_deck_id
                 FROM documents
                 WHERE id = ?1 AND generated_deck_id IS NOT NULL
                 UNION
                 SELECT note.deck_id
                 FROM document_note_topics AS topic
                 JOIN notes AS note
                   ON note.id = topic.note_id AND note.deleted_at IS NULL
                 WHERE topic.document_id = ?1
             ) AS relation
             JOIN decks AS deck ON deck.id = relation.deck_id",
            params![document_id, citation_id, now],
        )
        .map_err(|error| {
            CommandError::database_at(
                "document_deck_citation_preservation_failed",
                format!("document={document_id}"),
                error,
            )
        })?;

    connection
        .execute(
            "INSERT INTO note_source_citations (
                 note_id, citation_id, source_locator_json, created_at, updated_at
             )
             SELECT relation.note_id, ?2, relation.source_locator_json,
                    relation.created_at, ?3
             FROM document_note_topics AS relation
             JOIN notes AS note ON note.id = relation.note_id
             WHERE relation.document_id = ?1
               AND relation.relationship_kind = 'primary'
             ON CONFLICT(note_id) DO UPDATE SET
                 citation_id = excluded.citation_id,
                 source_locator_json = excluded.source_locator_json,
                 updated_at = excluded.updated_at",
            params![document_id, citation_id, now],
        )
        .map_err(|error| {
            CommandError::database_at(
                "document_note_citation_preservation_failed",
                format!("document={document_id}"),
                error,
            )
        })?;

    let active_deck_count = connection
        .query_row(
            "SELECT COUNT(*)
             FROM deck_source_citations AS citation
             JOIN decks AS deck ON deck.id = citation.deck_id
             WHERE citation.citation_id = ?1
               AND deck.deleted_at IS NULL",
            [citation_id.as_str()],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;
    let note_count = connection
        .query_row(
            "SELECT COUNT(*)
             FROM note_source_citations
             WHERE citation_id = ?1",
            [citation_id.as_str()],
            |row| row.get(0),
        )
        .map_err(CommandError::database)?;

    let changed = connection
        .execute(
            "UPDATE source_citation_snapshots
             SET source_removed = 1, updated_at = MAX(updated_at, ?1)
             WHERE id = ?2",
            params![now, citation_id],
        )
        .map_err(CommandError::database)?;
    if changed != 1 {
        return Err(CommandError::new(
            "document_citation_preservation_failed",
            "The document source citation changed before deletion completed.",
        ));
    }

    Ok(PreservedDocumentCitations {
        active_deck_count,
        note_count,
    })
}

#[tauri::command]
pub fn list_deck_source_citations(
    database: State<'_, Database>,
    deck_id: String,
) -> Result<Vec<SourceCitationDto>, CommandError> {
    database.with_connection(|connection| load_deck_citations(connection, &deck_id))
}

#[tauri::command]
pub fn get_note_source_citation(
    database: State<'_, Database>,
    note_id: String,
) -> Result<Option<SourceCitationDto>, CommandError> {
    database.with_connection(|connection| load_note_citation(connection, &note_id))
}

/// Resolve both live provenance and durable post-deletion snapshots.
///
/// Deck citations follow notes when cards are moved between decks. An explicit
/// `deck_source_citations` row additionally preserves the generated-deck
/// relationship when no source-grounded notes remain.
fn load_deck_citations(
    connection: &Connection,
    deck_id: &str,
) -> Result<Vec<SourceCitationDto>, CommandError> {
    validate_entity_id(deck_id, "deck")?;
    let mut statement = connection
        .prepare(
            "WITH source_ids(source_document_id) AS (
                 SELECT snapshot.source_document_id
                 FROM deck_source_citations AS relation
                 JOIN source_citation_snapshots AS snapshot
                   ON snapshot.id = relation.citation_id
                 WHERE relation.deck_id = ?1
                 UNION
                 SELECT relation.document_id
                 FROM document_decks AS relation
                 JOIN documents AS document ON document.id = relation.document_id
                 WHERE relation.deck_id = ?1
                 UNION
                 SELECT relation.document_id
                 FROM document_note_topics AS relation
                 JOIN notes AS note
                   ON note.id = relation.note_id AND note.deleted_at IS NULL
                 JOIN documents AS document ON document.id = relation.document_id
                 WHERE note.deck_id = ?1
                 UNION
                 SELECT snapshot.source_document_id
                 FROM note_source_citations AS relation
                 JOIN notes AS note
                   ON note.id = relation.note_id AND note.deleted_at IS NULL
                 JOIN source_citation_snapshots AS snapshot
                   ON snapshot.id = relation.citation_id
                 WHERE note.deck_id = ?1
             )
             SELECT
                 COALESCE(snapshot.id, document.id),
                 COALESCE(snapshot.schema_version, ?2),
                 source_ids.source_document_id,
                 COALESCE(
                     CASE
                         WHEN document.id IS NOT NULL
                         THEN 'sha256:' || document.sha256
                     END,
                     snapshot.source_identity
                 ),
                 COALESCE(
                     document.sha256,
                     snapshot.source_fingerprint_sha256
                 ),
                 COALESCE(document.title, snapshot.source_title),
                 COALESCE(
                     document.original_filename,
                     snapshot.original_filename
                 ),
                 COALESCE(document.source_format, snapshot.source_format),
                 COALESCE(
                     document.source_media_type,
                     snapshot.source_media_type
                 ),
                 COALESCE(
                     document.source_unit_kind,
                     snapshot.source_unit_kind
                 ),
                 COALESCE(
                     document.source_unit_count,
                     snapshot.source_unit_count
                 ),
                 CASE WHEN document.id IS NULL THEN 1 ELSE 0 END,
                 NULL
             FROM source_ids
             LEFT JOIN documents AS document
               ON document.id = source_ids.source_document_id
             LEFT JOIN source_citation_snapshots AS snapshot
               ON snapshot.source_document_id = source_ids.source_document_id
             WHERE document.id IS NOT NULL OR snapshot.id IS NOT NULL
             ORDER BY
                 COALESCE(document.title, snapshot.source_title)
                    COLLATE NOCASE,
                 source_ids.source_document_id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map(
            params![deck_id, SOURCE_CITATION_SCHEMA_VERSION],
            citation_row,
        )
        .map_err(CommandError::database)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)
        .and_then(validate_citation_locators)
}

fn load_note_citation(
    connection: &Connection,
    note_id: &str,
) -> Result<Option<SourceCitationDto>, CommandError> {
    validate_entity_id(note_id, "note")?;
    let citation = connection
        .query_row(
            "WITH candidates(
                 precedence, source_document_id, source_locator_json
             ) AS (
                 SELECT
                     0,
                     snapshot.source_document_id,
                     relation.source_locator_json
                 FROM note_source_citations AS relation
                 JOIN source_citation_snapshots AS snapshot
                   ON snapshot.id = relation.citation_id
                 WHERE relation.note_id = ?1
                 UNION ALL
                 SELECT
                     1,
                     relation.document_id,
                     relation.source_locator_json
                 FROM document_note_topics AS relation
                 WHERE relation.note_id = ?1
                   AND relation.relationship_kind = 'primary'
             )
             SELECT
                 COALESCE(snapshot.id, document.id),
                 COALESCE(snapshot.schema_version, ?2),
                 candidates.source_document_id,
                 COALESCE(
                     CASE
                         WHEN document.id IS NOT NULL
                         THEN 'sha256:' || document.sha256
                     END,
                     snapshot.source_identity
                 ),
                 COALESCE(
                     document.sha256,
                     snapshot.source_fingerprint_sha256
                 ),
                 COALESCE(document.title, snapshot.source_title),
                 COALESCE(
                     document.original_filename,
                     snapshot.original_filename
                 ),
                 COALESCE(document.source_format, snapshot.source_format),
                 COALESCE(
                     document.source_media_type,
                     snapshot.source_media_type
                 ),
                 COALESCE(
                     document.source_unit_kind,
                     snapshot.source_unit_kind
                 ),
                 COALESCE(
                     document.source_unit_count,
                     snapshot.source_unit_count
                 ),
                 CASE WHEN document.id IS NULL THEN 1 ELSE 0 END,
                 candidates.source_locator_json
             FROM candidates
             LEFT JOIN documents AS document
               ON document.id = candidates.source_document_id
             LEFT JOIN source_citation_snapshots AS snapshot
               ON snapshot.source_document_id = candidates.source_document_id
             WHERE document.id IS NOT NULL OR snapshot.id IS NOT NULL
             ORDER BY candidates.precedence
             LIMIT 1",
            params![note_id, SOURCE_CITATION_SCHEMA_VERSION],
            citation_row,
        )
        .optional()
        .map_err(CommandError::database)?;
    citation.map(validate_citation_locator).transpose()
}

fn citation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SourceCitationDto> {
    let locator_json: Option<String> = row.get(12)?;
    Ok(SourceCitationDto {
        citation_id: row.get(0)?,
        schema_version: row.get(1)?,
        source_document_id: row.get(2)?,
        source_identity: row.get(3)?,
        source_fingerprint_sha256: row.get(4)?,
        source_title: row.get(5)?,
        original_filename: row.get(6)?,
        source_format: row.get(7)?,
        source_media_type: row.get(8)?,
        source_unit_kind: row.get(9)?,
        source_unit_count: row.get(10)?,
        source_removed: row.get::<_, i64>(11)? != 0,
        source_locator: locator_json
            .map(|value| serde_json::from_str::<SourceLocator>(&value))
            .transpose()
            .map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    12,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?,
    })
}

fn validate_citation_locators(
    citations: Vec<SourceCitationDto>,
) -> Result<Vec<SourceCitationDto>, CommandError> {
    for citation in &citations {
        validate_citation(citation)?;
    }
    Ok(citations)
}

fn validate_citation_locator(
    citation: SourceCitationDto,
) -> Result<SourceCitationDto, CommandError> {
    validate_citation(&citation)?;
    Ok(citation)
}

fn validate_citation(citation: &SourceCitationDto) -> Result<(), CommandError> {
    if citation.schema_version != SOURCE_CITATION_SCHEMA_VERSION {
        return Err(CommandError::new(
            "unsupported_source_citation_version",
            "This source citation was created by an unsupported ostudy version.",
        ));
    }
    if let Some(locator) = &citation.source_locator {
        let source_unit_count = usize::try_from(citation.source_unit_count).map_err(|_| {
            CommandError::new(
                "invalid_source_citation",
                "A saved source citation has an invalid source-unit count.",
            )
        })?;
        locator.validate_within(source_unit_count)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_domain::{SourceLocator, SourceUnitKind};

    fn fixture() -> Connection {
        let connection = Connection::open_in_memory().expect("fixture database should open");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE documents (
                     id TEXT PRIMARY KEY,
                     title TEXT NOT NULL,
                     original_filename TEXT NOT NULL,
                     source_format TEXT NOT NULL,
                     source_media_type TEXT NOT NULL,
                     source_unit_kind TEXT NOT NULL,
                     source_unit_count INTEGER NOT NULL,
                     sha256 TEXT NOT NULL,
                     generated_deck_id TEXT,
                     created_at INTEGER NOT NULL,
                     updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE decks (
                     id TEXT PRIMARY KEY,
                     description TEXT NOT NULL,
                     deleted_at INTEGER
                 );
                 CREATE TABLE notes (
                     id TEXT PRIMARY KEY,
                     deck_id TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
                     deleted_at INTEGER
                 );
                 CREATE TABLE document_decks (
                     document_id TEXT NOT NULL
                         REFERENCES documents(id) ON DELETE CASCADE,
                     deck_id TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE
                 );
                 CREATE TABLE document_note_topics (
                     document_id TEXT NOT NULL
                         REFERENCES documents(id) ON DELETE CASCADE,
                     note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
                     relationship_kind TEXT NOT NULL,
                     source_locator_json TEXT NOT NULL,
                     created_at INTEGER NOT NULL
                 );
                 CREATE TABLE source_citation_snapshots (
                     id TEXT PRIMARY KEY,
                     schema_version INTEGER NOT NULL,
                     source_document_id TEXT NOT NULL UNIQUE,
                     source_identity TEXT NOT NULL,
                     source_fingerprint_sha256 TEXT NOT NULL,
                     source_title TEXT NOT NULL,
                     original_filename TEXT NOT NULL,
                     source_format TEXT NOT NULL,
                     source_media_type TEXT NOT NULL,
                     source_unit_kind TEXT NOT NULL,
                     source_unit_count INTEGER NOT NULL,
                     source_removed INTEGER NOT NULL,
                     created_at INTEGER NOT NULL,
                     updated_at INTEGER NOT NULL
                 );
                 CREATE TABLE deck_source_citations (
                     deck_id TEXT NOT NULL REFERENCES decks(id) ON DELETE CASCADE,
                     citation_id TEXT NOT NULL
                         REFERENCES source_citation_snapshots(id) ON DELETE RESTRICT,
                     created_at INTEGER NOT NULL,
                     PRIMARY KEY(deck_id, citation_id)
                 );
                 CREATE TABLE note_source_citations (
                     note_id TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
                     citation_id TEXT NOT NULL
                         REFERENCES source_citation_snapshots(id) ON DELETE RESTRICT,
                     source_locator_json TEXT,
                     created_at INTEGER NOT NULL,
                     updated_at INTEGER NOT NULL
                 );",
            )
            .expect("fixture schema should initialize");
        connection
    }

    fn insert_document(connection: &Connection, document_id: &str, deck_id: &str) {
        connection
            .execute(
                "INSERT INTO documents (
                     id, title, original_filename, source_format,
                     source_media_type, source_unit_kind, source_unit_count,
                     sha256, generated_deck_id, created_at, updated_at
                 ) VALUES (
                     ?1, 'Renal physiology', 'renal.pdf', 'pdf',
                     'application/pdf', 'page', 10, ?2, ?3, 1, 2
                 )",
                params![document_id, "d".repeat(64), deck_id],
            )
            .unwrap();
    }

    #[test]
    fn source_deletion_preserves_structured_deck_and_note_citations() {
        let connection = fixture();
        let document_id = "a".repeat(32);
        let deck_id = "b".repeat(32);
        let note_id = "c".repeat(32);
        let locator =
            SourceLocator::new(SourceUnitKind::Page, vec![2, 3], Some("Evidence".into())).unwrap();
        let locator_json = serde_json::to_string(&locator).unwrap();
        insert_document(&connection, &document_id, &deck_id);
        connection
            .execute(
                "INSERT INTO decks (id, description, deleted_at)
                 VALUES (?1, 'User-written description', NULL)",
                [deck_id.as_str()],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO notes (id, deck_id, deleted_at)
                 VALUES (?1, ?2, NULL)",
                params![note_id, deck_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO document_decks (document_id, deck_id)
                 VALUES (?1, ?2)",
                params![document_id, deck_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO document_note_topics (
                     document_id, note_id, relationship_kind,
                     source_locator_json, created_at
                 ) VALUES (?1, ?2, 'primary', ?3, 2)",
                params![document_id, note_id, locator_json],
            )
            .unwrap();

        let live_deck_citations = load_deck_citations(&connection, &deck_id).unwrap();
        assert_eq!(live_deck_citations.len(), 1);
        assert!(!live_deck_citations[0].source_removed);
        assert_eq!(live_deck_citations[0].source_title, "Renal physiology");

        let preserved =
            preserve_document_citations_before_delete(&connection, &document_id, 10).unwrap();
        assert_eq!(
            preserved,
            PreservedDocumentCitations {
                active_deck_count: 1,
                note_count: 1,
            }
        );
        connection
            .execute(
                "DELETE FROM documents WHERE id = ?1",
                [document_id.as_str()],
            )
            .unwrap();

        let description: String = connection
            .query_row(
                "SELECT description FROM decks WHERE id = ?1",
                [deck_id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(description, "User-written description");

        let deck_citations = load_deck_citations(&connection, &deck_id).unwrap();
        assert_eq!(deck_citations.len(), 1);
        assert!(deck_citations[0].source_removed);
        assert_eq!(deck_citations[0].source_title, "Renal physiology");

        let note_citation = load_note_citation(&connection, &note_id)
            .unwrap()
            .expect("note citation should survive");
        assert_eq!(note_citation.source_locator, Some(locator));
        assert!(note_citation.source_removed);

        let foreign_key_violations: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(foreign_key_violations, 0);
    }

    #[test]
    fn live_note_citation_follows_a_note_between_decks() {
        let connection = fixture();
        let document_id = "1".repeat(32);
        let source_deck_id = "2".repeat(32);
        let destination_deck_id = "3".repeat(32);
        let note_id = "4".repeat(32);
        let locator = SourceLocator::new(SourceUnitKind::Page, vec![4], None).unwrap();
        let locator_json = serde_json::to_string(&locator).unwrap();

        for deck_id in [&source_deck_id, &destination_deck_id] {
            connection
                .execute(
                    "INSERT INTO decks (id, description, deleted_at)
                     VALUES (?1, '', NULL)",
                    [deck_id.as_str()],
                )
                .unwrap();
        }
        insert_document(&connection, &document_id, &source_deck_id);
        connection
            .execute(
                "INSERT INTO notes (id, deck_id, deleted_at)
                 VALUES (?1, ?2, NULL)",
                params![note_id, source_deck_id],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO document_note_topics (
                     document_id, note_id, relationship_kind,
                     source_locator_json, created_at
                 ) VALUES (?1, ?2, 'primary', ?3, 2)",
                params![document_id, note_id, locator_json],
            )
            .unwrap();

        assert_eq!(
            load_deck_citations(&connection, &source_deck_id)
                .unwrap()
                .len(),
            1
        );
        assert!(
            load_deck_citations(&connection, &destination_deck_id)
                .unwrap()
                .is_empty()
        );

        connection
            .execute(
                "UPDATE notes SET deck_id = ?1 WHERE id = ?2",
                params![destination_deck_id, note_id],
            )
            .unwrap();

        assert!(
            load_deck_citations(&connection, &source_deck_id)
                .unwrap()
                .is_empty()
        );
        let destination = load_deck_citations(&connection, &destination_deck_id).unwrap();
        assert_eq!(destination.len(), 1);
        assert_eq!(destination[0].source_locator, None);
    }
}
