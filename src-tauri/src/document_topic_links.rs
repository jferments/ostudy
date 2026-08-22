//! Stable source-grounded links between document learning topics and card notes.
//!
//! Tags remain learner-editable organization. Topic links are relational
//! provenance: they are derived from the generated note's canonical source
//! locator and the active learning plan, validated locally, and retained across
//! guide prose revisions.

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::{
    database::Database,
    document_domain::SourceLocator,
    document_learning::{topics_for_source_locator, DocumentLearningPlan},
    error::CommandError,
    util::{unix_time_millis, validate_entity_id},
};

const MAX_TOPIC_LINKS_PER_NOTE: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum NoteTopicRelationship {
    Primary,
    Supporting,
}

impl NoteTopicRelationship {
    fn as_db(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Supporting => "supporting",
        }
    }

    fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "primary" => Ok(Self::Primary),
            "supporting" => Ok(Self::Supporting),
            _ => Err(CommandError::new(
                "invalid_document_note_topic_link",
                "A persisted note-topic relationship was invalid.",
            )),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentNoteTopicLink {
    pub document_id: String,
    pub note_id: String,
    pub topic_id: String,
    pub relationship: NoteTopicRelationship,
    pub source_locator: SourceLocator,
    pub evidence_hash: String,
    pub generation_job_id: Option<String>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TopicCardProjection {
    pub topic_id: String,
    pub note_count: usize,
    pub card_count: usize,
    pub studyable_card_count: usize,
}

/// Persist topic links for one generated note inside the caller's card-import
/// transaction. The narrowest overlapping topic becomes primary; wider
/// overlapping topics are supporting. No model-returned topic ID is trusted.
pub(crate) fn link_generated_note_to_learning_plan(
    connection: &Connection,
    document_id: &str,
    note_id: &str,
    generation_job_id: Option<&str>,
    plan: &DocumentLearningPlan,
    source_locator: &SourceLocator,
    now: i64,
) -> Result<Vec<DocumentNoteTopicLink>, CommandError> {
    validate_entity_id(document_id, "document")?;
    validate_entity_id(note_id, "note")?;
    if let Some(job_id) = generation_job_id {
        validate_entity_id(job_id, "document generation job")?;
    }
    if plan.document_id != document_id {
        return Err(CommandError::new(
            "document_learning_plan_mismatch",
            "The generated note cannot be linked to a learning plan from another document.",
        ));
    }

    let note_exists = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM notes WHERE id = ?1 AND deleted_at IS NULL)",
            [note_id],
            |row| row.get::<_, i64>(0),
        )
        .map(|value| value != 0)
        .map_err(CommandError::database)?;
    if !note_exists {
        return Err(CommandError::new(
            "generated_note_not_found",
            "A generated note disappeared before topic provenance could be committed.",
        ));
    }

    let selected = topics_for_source_locator(plan, source_locator)
        .into_iter()
        .take(MAX_TOPIC_LINKS_PER_NOTE)
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(CommandError::new(
            "generated_note_topic_unresolved",
            "The generated note's source evidence did not overlap any learning-plan topic.",
        ));
    }

    connection
        .execute(
            "DELETE FROM document_note_topics WHERE note_id = ?1",
            [note_id],
        )
        .map_err(CommandError::database)?;

    let locator_json =
        serde_json::to_string(source_locator).map_err(CommandError::serialization)?;
    let mut links = Vec::with_capacity(selected.len());
    for (index, topic) in selected.into_iter().enumerate() {
        let relationship = if index == 0 {
            NoteTopicRelationship::Primary
        } else {
            NoteTopicRelationship::Supporting
        };
        connection
            .execute(
                "INSERT INTO document_note_topics (
                     document_id, note_id, topic_id, relationship_kind,
                     source_locator_json, evidence_hash, generation_job_id,
                     created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    document_id,
                    note_id,
                    topic.id,
                    relationship.as_db(),
                    locator_json,
                    topic.evidence_hash,
                    generation_job_id,
                    now,
                ],
            )
            .map_err(CommandError::database)?;
        links.push(DocumentNoteTopicLink {
            document_id: document_id.to_string(),
            note_id: note_id.to_string(),
            topic_id: topic.id.clone(),
            relationship,
            source_locator: source_locator.clone(),
            evidence_hash: topic.evidence_hash.clone(),
            generation_job_id: generation_job_id.map(str::to_string),
            created_at: now,
        });
    }
    Ok(links)
}

pub(crate) fn load_note_topic_links(
    database: &Database,
    note_id: &str,
) -> Result<Vec<DocumentNoteTopicLink>, CommandError> {
    validate_entity_id(note_id, "note")?;
    database
        .with_connection(|connection| load_note_topic_links_from_connection(connection, note_id))
}

pub(crate) fn load_topic_card_projection(
    database: &Database,
    document_id: &str,
    topic_id: &str,
) -> Result<TopicCardProjection, CommandError> {
    validate_entity_id(document_id, "document")?;
    validate_entity_id(topic_id, "learning topic")?;
    database.with_connection(|connection| {
        let topic_matches = connection
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM document_learning_topics
                     WHERE id = ?1 AND document_id = ?2
                 )",
                params![topic_id, document_id],
                |row| row.get::<_, i64>(0),
            )
            .map(|value| value != 0)
            .map_err(CommandError::database)?;
        if !topic_matches {
            return Err(CommandError::new(
                "document_learning_topic_not_found",
                "That learning topic is no longer available for this document.",
            ));
        }

        let (note_count, card_count, studyable_count) = connection
            .query_row(
                "SELECT
                     COUNT(DISTINCT link.note_id),
                     COUNT(DISTINCT card.id),
                     COUNT(DISTINCT CASE
                         WHEN card.deleted_at IS NULL
                          AND card.suspended_at IS NULL
                          AND (card.buried_until IS NULL OR card.buried_until <= ?1)
                         THEN card.id END)
                 FROM document_note_topics link
                 LEFT JOIN notes note
                   ON note.id = link.note_id AND note.deleted_at IS NULL
                 LEFT JOIN cards card
                   ON card.note_id = note.id
                 WHERE link.document_id = ?2 AND link.topic_id = ?3",
                params![unix_time_millis()?, document_id, topic_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .map_err(CommandError::database)?;
        Ok(TopicCardProjection {
            topic_id: topic_id.to_string(),
            note_count: to_usize(note_count)?,
            card_count: to_usize(card_count)?,
            studyable_card_count: to_usize(studyable_count)?,
        })
    })
}

/// Resolve exact card IDs for a topic-scoped practice session. Descendant topic
/// expansion is deliberately performed by the learning-plan caller so hierarchy
/// policy remains explicit rather than hidden inside a SQL title/path heuristic.
pub(crate) fn load_topic_card_ids(
    database: &Database,
    document_id: &str,
    topic_ids: &[String],
) -> Result<Vec<String>, CommandError> {
    validate_entity_id(document_id, "document")?;
    if topic_ids.is_empty() {
        return Ok(Vec::new());
    }
    for topic_id in topic_ids {
        validate_entity_id(topic_id, "learning topic")?;
    }
    database.with_connection(|connection| {
        let placeholders = std::iter::repeat_n("?", topic_ids.len())
            .enumerate()
            .map(|(index, placeholder)| format!("{placeholder}{}", index + 2))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT DISTINCT card.id
             FROM document_note_topics link
             JOIN notes note ON note.id = link.note_id AND note.deleted_at IS NULL
             JOIN cards card ON card.note_id = note.id AND card.deleted_at IS NULL
             WHERE link.document_id = ?1 AND link.topic_id IN ({placeholders})
             ORDER BY card.created_at, card.id"
        );
        let mut values: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(topic_ids.len() + 1);
        values.push(&document_id);
        for topic_id in topic_ids {
            values.push(topic_id);
        }
        let mut statement = connection.prepare(&sql).map_err(CommandError::database)?;
        let rows = statement
            .query_map(values.as_slice(), |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?;
        let card_ids = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        Ok(card_ids)
    })
}

fn load_note_topic_links_from_connection(
    connection: &Connection,
    note_id: &str,
) -> Result<Vec<DocumentNoteTopicLink>, CommandError> {
    let mut statement = connection
        .prepare(
            "SELECT document_id, topic_id, relationship_kind,
                    source_locator_json, evidence_hash,
                    generation_job_id, created_at
             FROM document_note_topics WHERE note_id = ?1
             ORDER BY CASE relationship_kind WHEN 'primary' THEN 0 ELSE 1 END,
                      topic_id",
        )
        .map_err(CommandError::database)?;
    let rows = statement
        .query_map([note_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })
        .map_err(CommandError::database)?;
    let links = rows
        .map(|row| {
            let row = row.map_err(CommandError::database)?;
            Ok(DocumentNoteTopicLink {
                document_id: row.0,
                note_id: note_id.to_string(),
                topic_id: row.1,
                relationship: NoteTopicRelationship::from_db(&row.2)?,
                source_locator: serde_json::from_str(&row.3)
                    .map_err(CommandError::serialization)?,
                evidence_hash: row.4,
                generation_job_id: row.5,
                created_at: row.6,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    Ok(links)
}

fn to_usize(value: i64) -> Result<usize, CommandError> {
    usize::try_from(value).map_err(|_| {
        CommandError::new(
            "invalid_document_topic_projection",
            "A persisted document-topic count was invalid.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document_domain::SourceUnitKind,
        document_learning::{DocumentLearningPlan, LearningTopic, LEARNING_PLAN_COMPILER_VERSION},
    };

    fn locator(start: i64, end: i64) -> SourceLocator {
        SourceLocator::new(SourceUnitKind::Page, (start..=end).collect(), None).unwrap()
    }

    fn plan() -> DocumentLearningPlan {
        DocumentLearningPlan {
            id: "a".repeat(32),
            document_id: "b".repeat(32),
            revision: 1,
            compiler_version: LEARNING_PLAN_COMPILER_VERSION,
            outline_fingerprint: "c".repeat(64),
            plan_fingerprint: "d".repeat(64),
            summary: String::new(),
            learning_objectives: Vec::new(),
            key_concepts: Vec::new(),
            topics: vec![
                LearningTopic {
                    id: "e".repeat(32),
                    parent_topic_id: None,
                    sort_index: 0,
                    title: "Chapter".to_string(),
                    summary: String::new(),
                    learning_objectives: Vec::new(),
                    key_concepts: Vec::new(),
                    source_locator: locator(1, 4),
                    evidence_chunk_indices: vec![0],
                    evidence_hashes: vec!["f".repeat(64)],
                    evidence_hash: "f".repeat(64),
                    anchor_hash: "1".repeat(64),
                },
                LearningTopic {
                    id: "2".repeat(32),
                    parent_topic_id: Some("e".repeat(32)),
                    sort_index: 1,
                    title: "Mechanism".to_string(),
                    summary: String::new(),
                    learning_objectives: Vec::new(),
                    key_concepts: Vec::new(),
                    source_locator: locator(2, 2),
                    evidence_chunk_indices: vec![1],
                    evidence_hashes: vec!["3".repeat(64)],
                    evidence_hash: "3".repeat(64),
                    anchor_hash: "4".repeat(64),
                },
            ],
        }
    }

    #[test]
    fn narrowest_overlap_is_the_primary_topic_candidate() {
        let plan = plan();
        let selected = topics_for_source_locator(&plan, &locator(2, 2));
        assert_eq!(selected[0].title, "Mechanism");
        assert_eq!(selected[1].title, "Chapter");
    }

    #[test]
    fn relationship_names_are_stable_database_contracts() {
        assert_eq!(NoteTopicRelationship::Primary.as_db(), "primary");
        assert_eq!(NoteTopicRelationship::Supporting.as_db(), "supporting");
    }
}
