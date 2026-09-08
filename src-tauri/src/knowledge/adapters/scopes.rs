use rusqlite::{params_from_iter, Connection, OptionalExtension};

use crate::{database::Database, error::CommandError};

use super::{
    super::contracts::{invalid, ScopeChoice, ScopeKind, SourceKind, StudyScope},
    SourceSeed,
};

/// Guide scopes use the stable document ID. Focused requests separately verify
/// the generated guide revision and effective section hash.
pub(crate) fn scope_documents(
    connection: &Connection,
    scope: &StudyScope,
) -> Result<Vec<String>, CommandError> {
    scope.validate()?;
    match scope.kind {
        ScopeKind::General => Ok(Vec::new()),
        ScopeKind::Library => query_ids(
            connection,
            "SELECT id FROM documents WHERE lifecycle_state <> 'archived' ORDER BY id LIMIT 100001",
            None,
        ),
        ScopeKind::Deck => {
            let id = scope.id.as_deref().ok_or_else(|| invalid("Select a deck."))?;
            require_deck(connection, id)?;
            query_ids(
                connection,
                "SELECT document_id FROM document_decks WHERE deck_id = ?1
                 UNION SELECT nt.document_id FROM document_note_topics nt
                 JOIN notes n ON n.id = nt.note_id WHERE n.deck_id = ?1 AND n.deleted_at IS NULL
                 ORDER BY document_id LIMIT 100001",
                Some(id),
            )
        }
        ScopeKind::Document | ScopeKind::Guide => {
            let id = scope.id.as_deref().ok_or_else(|| invalid("Select a document."))?;
            let exists = connection
                .query_row("SELECT 1 FROM documents WHERE id = ?1", [id], |_| Ok(()))
                .optional()
                .map_err(CommandError::database)?;
            if exists.is_none() {
                return Err(invalid("That source document is no longer in the library."));
            }
            if scope.kind == ScopeKind::Guide {
                let guide = connection
                    .query_row(
                        "SELECT 1 FROM document_study_guides WHERE document_id = ?1 AND state = 'active'",
                        [id],
                        |_| Ok(()),
                    )
                    .optional()
                    .map_err(CommandError::database)?;
                if guide.is_none() {
                    return Err(invalid("That source no longer has an active study guide."));
                }
            }
            Ok(vec![id.to_string()])
        }
    }
}

pub(super) fn enumerate_material(
    connection: &Connection,
    scope: &StudyScope,
) -> Result<Vec<SourceSeed>, CommandError> {
    let documents = scope_documents(connection, scope)?;
    let documents_json = serde_json::to_string(&documents).map_err(CommandError::serialization)?;
    let (card_sql, argument) = match scope.kind {
        ScopeKind::General => return Ok(Vec::new()),
        ScopeKind::Library => (
            "SELECT c.id FROM cards c JOIN notes n ON n.id = c.note_id JOIN decks d ON d.id = n.deck_id
             WHERE c.deleted_at IS NULL AND n.deleted_at IS NULL AND d.deleted_at IS NULL
               AND d.archived_at IS NULL ORDER BY c.id LIMIT 100001",
            None,
        ),
        ScopeKind::Deck => (
            "SELECT c.id FROM cards c JOIN notes n ON n.id = c.note_id
             WHERE n.deck_id = ?1 AND c.deleted_at IS NULL AND n.deleted_at IS NULL
             ORDER BY c.id LIMIT 100001",
            scope.id.as_deref(),
        ),
        ScopeKind::Document | ScopeKind::Guide => (
            "SELECT DISTINCT c.id FROM cards c JOIN notes n ON n.id = c.note_id
             JOIN decks d ON d.id = n.deck_id
             WHERE c.deleted_at IS NULL AND n.deleted_at IS NULL AND d.deleted_at IS NULL
               AND (EXISTS(SELECT 1 FROM document_note_topics nt
                           WHERE nt.note_id = n.id AND nt.document_id = ?1)
                    OR EXISTS(SELECT 1 FROM document_decks dd
                              WHERE dd.deck_id = n.deck_id AND dd.document_id = ?1))
             ORDER BY c.id LIMIT 100001",
            scope.id.as_deref(),
        ),
    };
    let mut result = query_ids(connection, card_sql, argument)?
        .into_iter()
        .map(|id| SourceSeed {
            kind: SourceKind::Card,
            owner_id: id,
            parent_key: "presentation".to_string(),
        })
        .collect::<Vec<_>>();
    let mut chunks = connection
        .prepare(
            "SELECT document_id, chunk_index FROM document_semantic_chunk_sources
             WHERE document_id IN (SELECT value FROM json_each(?1))
             ORDER BY document_id, chunk_index LIMIT 100001",
        )
        .map_err(CommandError::database)?;
    result.extend(
        chunks
            .query_map([&documents_json], |row| {
                Ok(SourceSeed {
                    kind: SourceKind::Document,
                    owner_id: row.get(0)?,
                    parent_key: row.get::<_, i64>(1)?.to_string(),
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?,
    );
    let mut guides = connection
        .prepare(
            "SELECT g.id, s.topic_id FROM document_study_guides g
             JOIN document_study_guide_sections s ON s.guide_id = g.id
             WHERE g.document_id IN (SELECT value FROM json_each(?1)) AND g.state = 'active'
             UNION ALL SELECT g.id, 'overview' FROM document_study_guides g
             WHERE g.document_id IN (SELECT value FROM json_each(?1)) AND g.state = 'active'
             ORDER BY 1, 2 LIMIT 100001",
        )
        .map_err(CommandError::database)?;
    result.extend(
        guides
            .query_map([documents_json], |row| {
                Ok(SourceSeed {
                    kind: SourceKind::Guide,
                    owner_id: row.get(0)?,
                    parent_key: row.get(1)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?,
    );
    Ok(result)
}

pub(crate) fn list_scope_choices(database: &Database) -> Result<Vec<ScopeChoice>, CommandError> {
    database.with_connection(|connection| {
        let mut statement = connection
            .prepare(
                "SELECT 'deck', id, name FROM decks WHERE deleted_at IS NULL AND archived_at IS NULL
                 UNION ALL SELECT 'document', id, title FROM documents WHERE lifecycle_state <> 'archived'
                 UNION ALL SELECT 'guide', d.id, d.title FROM documents d
                 JOIN document_study_guides g ON g.document_id = d.id
                 WHERE g.state = 'active' AND d.lifecycle_state <> 'archived'
                 ORDER BY 1, 3, 2 LIMIT 2000",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([], |row| {
                let kind: String = row.get(0)?;
                Ok(ScopeChoice {
                    scope: StudyScope {
                        kind: match kind.as_str() {
                            "deck" => ScopeKind::Deck,
                            "document" => ScopeKind::Document,
                            _ => ScopeKind::Guide,
                        },
                        id: Some(row.get(1)?),
                    },
                    title: row.get(2)?,
                })
            })
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        Ok(rows)
    })
}

fn query_ids(
    connection: &Connection,
    sql: &str,
    argument: Option<&str>,
) -> Result<Vec<String>, CommandError> {
    let mut statement = connection.prepare(sql).map_err(CommandError::database)?;
    let rows = statement
        .query_map(params_from_iter(argument), |row| row.get::<_, String>(0))
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(rows)
}

fn require_deck(connection: &Connection, id: &str) -> Result<(), CommandError> {
    let exists = connection
        .query_row(
            "SELECT 1 FROM decks WHERE id = ?1 AND deleted_at IS NULL",
            [id],
            |_| Ok(()),
        )
        .optional()
        .map_err(CommandError::database)?;
    exists.ok_or_else(|| invalid("That deck is no longer in the library."))
}
