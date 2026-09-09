//! Exact artifact/source relationships complement semantic similarity. These
//! links select context; they do not certify that generated or edited prose is
//! entailed by its original document.

use std::collections::HashSet;

use rusqlite::{params, OptionalExtension};

use crate::{database::Database, error::CommandError};

use super::{
    adapters::{materialize_source, SourceSeed},
    contracts::{SourceKind, SourceRecord},
    repository,
};

pub(crate) fn expand(
    database: &Database,
    selected: &[SourceRecord],
) -> Result<Vec<SourceRecord>, CommandError> {
    let seeds = database.with_connection(|connection| {
        let mut result = Vec::new();
        for source in selected.iter().take(4) {
            let parent: Option<String> = connection.query_row(
                "SELECT parent_key FROM knowledge_sources WHERE id = ?1 AND active = 1",
                [&source.id], |row| row.get(0),
            ).optional().map_err(CommandError::database)?;
            let topics = match source.kind {
                SourceKind::Card => {
                    let mut statement = connection.prepare(
                        "SELECT nt.document_id, nt.topic_id, g.id, p.evidence_chunk_indices_json
                         FROM document_note_topics nt
                         LEFT JOIN document_learning_plans lp ON lp.document_id = nt.document_id AND lp.state = 'active'
                         LEFT JOIN document_learning_plan_topics p ON p.plan_id = lp.id AND p.topic_id = nt.topic_id
                         LEFT JOIN document_study_guides g ON g.document_id = nt.document_id AND g.state = 'active'
                         WHERE nt.note_id = ?1 ORDER BY nt.relationship_kind, nt.topic_id LIMIT 3",
                    ).map_err(CommandError::database)?;
                    let topics = statement.query_map([&source.note_id], read_topic)
                        .map_err(CommandError::database)?.collect::<Result<Vec<_>, _>>()
                        .map_err(CommandError::database)?;
                    topics
                }
                SourceKind::Guide | SourceKind::Document => {
                    let mut statement = connection.prepare(
                        "SELECT lp.document_id, p.topic_id, g.id, p.evidence_chunk_indices_json
                         FROM document_learning_plans lp JOIN document_learning_plan_topics p ON p.plan_id = lp.id
                         LEFT JOIN document_study_guides g ON g.document_id = lp.document_id AND g.state = 'active'
                         WHERE lp.document_id = ?1 AND lp.state = 'active'
                           AND ((?2 = 'guide' AND (?3 = 'overview' OR p.topic_id = ?3))
                             OR (?2 = 'document' AND EXISTS(SELECT 1 FROM json_each(p.evidence_chunk_indices_json)
                                                          WHERE CAST(value AS TEXT) = ?3)))
                         ORDER BY p.sort_index LIMIT 3",
                    ).map_err(CommandError::database)?;
                    let topics = statement.query_map(params![source.document_id, source.kind.as_str(), parent], read_topic)
                        .map_err(CommandError::database)?.collect::<Result<Vec<_>, _>>()
                        .map_err(CommandError::database)?;
                    topics
                }
                SourceKind::Conversation => Vec::new(),
            };
            for (document_id, topic_id, guide_id, chunks) in topics {
                if let Some(guide_id) = guide_id {
                    result.push(SourceSeed { kind: SourceKind::Guide, owner_id: guide_id, parent_key: topic_id.clone() });
                }
                if let Some(chunks) = chunks {
                    let chunks: Vec<i64> = serde_json::from_str(&chunks).map_err(CommandError::serialization)?;
                    for chunk in chunks.into_iter().take(3) {
                        result.push(SourceSeed { kind: SourceKind::Document, owner_id: document_id.clone(), parent_key: chunk.to_string() });
                    }
                }
                let mut statement = connection.prepare(
                    "SELECT c.id FROM document_note_topics nt JOIN cards c ON c.note_id = nt.note_id
                     JOIN notes n ON n.id = c.note_id JOIN decks d ON d.id = n.deck_id
                     WHERE nt.document_id = ?1 AND nt.topic_id = ?2 AND c.deleted_at IS NULL
                       AND n.deleted_at IS NULL AND d.deleted_at IS NULL AND d.archived_at IS NULL
                     ORDER BY c.id LIMIT 2",
                ).map_err(CommandError::database)?;
                let ids = statement.query_map(params![document_id, topic_id], |row| row.get::<_, String>(0))
                    .map_err(CommandError::database)?.collect::<Result<Vec<_>, _>>()
                    .map_err(CommandError::database)?;
                result.extend(ids.into_iter().map(|owner_id| SourceSeed {
                    kind: SourceKind::Card, owner_id, parent_key: "presentation".to_string(),
                }));
            }
        }
        Ok(result)
    })?;
    let mut seen = selected
        .iter()
        .map(|source| source.id.clone())
        .collect::<HashSet<_>>();
    let mut seed_keys = HashSet::new();
    let mut result = Vec::new();
    // Explicit knowledge links survive generator rewording and can point to
    // evidence that a broad topic association alone would not select.
    database.with_connection(|connection| {
        for source in selected.iter().take(4) {
            for unit in repository::linked_units(connection, source)? {
                if result.len() >= 12 {
                    break;
                }
                if seen.contains(&unit.source_id) {
                    continue;
                }
                if let Some(evidence) = repository::load_source(connection, &unit.source_id)? {
                    if evidence.content_hash == unit.source_hash {
                        seen.insert(evidence.id.clone());
                        result.push(evidence);
                    }
                }
            }
        }
        Ok(())
    })?;
    for seed in seeds {
        if result.len() >= 12 {
            return Ok(result);
        }
        if !seed_keys.insert(format!(
            "{}:{}:{}",
            seed.kind.as_str(),
            seed.owner_id,
            seed.parent_key
        )) {
            continue;
        }
        for source in materialize_source(database, &seed)? {
            if seen.insert(source.id.clone()) {
                result.push(source);
            }
            if result.len() >= 12 {
                return Ok(result);
            }
        }
    }
    Ok(result)
}

type TopicRow = (String, String, Option<String>, Option<String>);

fn read_topic(row: &rusqlite::Row<'_>) -> rusqlite::Result<TopicRow> {
    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_knowledge_links_resolve_original_evidence_without_a_topic_match() {
        let (database, original, catalog, _) = super::super::authoring::tests::fixture();
        let mut reference = original.clone();
        reference.segment_key = "1:0:0".to_string();
        reference.text = "Related material written in different words.".to_string();
        reference.refresh_identity().unwrap();
        database
            .with_connection(|connection| {
                repository::publish_source(connection, &reference, "1", 2)?;
                repository::link_artifact_units(
                    connection,
                    &reference,
                    &[catalog.units[0].id.clone()],
                    "mentions",
                    2,
                )
            })
            .unwrap();
        let expanded = expand(&database, &[reference]).unwrap();
        assert_eq!(expanded.first().unwrap().id, original.id);
        database
            .with_connection(|connection| {
                let units = repository::load_units(connection, &expanded[0].id)?;
                assert_eq!(units[0].id, catalog.units[0].id);
                Ok(())
            })
            .unwrap();
    }
}
