//! Reuse already-paid partitions when a later part of the same topic fails.
//! Uses the existing checkpoint table and the existing best-effort writer.
use rusqlite::{Connection, OptionalExtension};

use super::{hash_text, preservation, GroundedStudyGuideSection, KnowledgeCatalog, LearningTopic};
use crate::{database::Database, error::CommandError};

pub(super) fn contract_key(topic_contract: &str, partition_identity: &str) -> String {
    hash_text(&format!(
        "guide-partition-v1|{topic_contract}|{partition_identity}"
    ))
}

pub(super) fn load(
    database: &Database,
    document_id: &str,
    contract: &str,
    topic: &LearningTopic,
    catalog: &KnowledgeCatalog,
) -> Option<GroundedStudyGuideSection> {
    match database.with_connection(|connection| {
        load_connection(connection, document_id, contract, topic, catalog)
    }) {
        Ok(value) => value,
        Err(error) => {
            crate::logging::debug_target(
                "ai.document.study_guide",
                "study_guide_partition_checkpoint_unavailable",
                "Could not reuse a partition checkpoint; no content was logged.",
                serde_json::json!({"code":error.code}),
            );
            None
        }
    }
}

fn load_connection(
    connection: &Connection,
    document_id: &str,
    contract: &str,
    topic: &LearningTopic,
    catalog: &KnowledgeCatalog,
) -> Result<Option<GroundedStudyGuideSection>, CommandError> {
    let row: Option<(String, String, String)> = connection
        .query_row(
            "SELECT evidence_hash, section_json, content_hash
             FROM document_study_guide_checkpoints
             WHERE document_id = ?1 AND contract_key = ?2 AND topic_id = ?3",
            rusqlite::params![document_id, contract, topic.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(CommandError::database)?;
    let Some((evidence_hash, json, content_hash)) = row else {
        return Ok(None);
    };
    if evidence_hash != topic.evidence_hash || hash_text(&json) != content_hash {
        return Ok(None);
    }
    let Ok(section) = serde_json::from_str::<GroundedStudyGuideSection>(&json) else {
        return Ok(None);
    };
    if section.knowledge_support.is_empty()
        || !catalog.validates(&section.knowledge_support, catalog.units.len().max(1))
        || catalog.fingerprint()? != section.catalog_fingerprint
    {
        return Ok(None);
    }
    let Ok(content) = preservation::normalize(section.content) else {
        return Ok(None);
    };
    Ok(Some(GroundedStudyGuideSection { content, ..section }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document_domain::{SourceLocator, SourceUnitKind},
        document_study_guides::StudyGuideSectionContent,
    };

    fn topic() -> LearningTopic {
        LearningTopic {
            id: "e".repeat(32),
            parent_topic_id: None,
            sort_index: 0,
            title: "Topic".into(),
            summary: "Not teaching prose".into(),
            learning_objectives: vec![],
            key_concepts: vec![],
            source_locator: SourceLocator::new(SourceUnitKind::Page, vec![1], None).unwrap(),
            evidence_chunk_indices: vec![0],
            evidence_hashes: vec!["f".repeat(64)],
            evidence_hash: "f".repeat(64),
            anchor_hash: "1".repeat(64),
        }
    }

    #[test]
    fn completed_partition_is_reusable_but_changed_context_or_corrupt_content_is_not() {
        let connection = Connection::open_in_memory().unwrap();
        // Focused SQL reader fixture, not a replacement for migration tests.
        connection
            .execute_batch(
                "CREATE TABLE document_study_guide_checkpoints (
                    document_id TEXT, contract_key TEXT, topic_id TEXT,
                    evidence_hash TEXT, section_json TEXT, content_hash TEXT,
                    PRIMARY KEY(document_id, contract_key, topic_id)
                );",
            )
            .unwrap();
        let catalog = crate::knowledge::authoring::tests::catalog_fixture();
        let topic = topic();
        let section = GroundedStudyGuideSection {
            content: StudyGuideSectionContent {
                overview: "The first paid partition remains intact after a later part fails."
                    .into(),
                ..Default::default()
            },
            knowledge_support: catalog.select(&["a".repeat(32)], 64).unwrap(),
            catalog_fingerprint: catalog.fingerprint().unwrap(),
        };
        let json = serde_json::to_string(&section).unwrap();
        let key = contract_key("model-source-plan-contract", "first-partition");
        connection
            .execute(
                "INSERT INTO document_study_guide_checkpoints VALUES (?1,?2,?3,?4,?5,?6)",
                rusqlite::params![
                    "document",
                    key,
                    topic.id,
                    topic.evidence_hash,
                    json,
                    hash_text(&json)
                ],
            )
            .unwrap();
        let loaded = load_connection(&connection, "document", &key, &topic, &catalog)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.content.overview, section.content.overview);
        assert!(load_connection(
            &connection,
            "document",
            &contract_key("changed-model-source-plan-contract", "first-partition"),
            &topic,
            &catalog,
        )
        .unwrap()
        .is_none());
        assert!(load_connection(
            &connection,
            "document",
            &contract_key("model-source-plan-contract", "second-partition"),
            &topic,
            &catalog,
        )
        .unwrap()
        .is_none());
        let mut changed = topic.clone();
        changed.evidence_hash = "0".repeat(64);
        assert!(
            load_connection(&connection, "document", &key, &changed, &catalog)
                .unwrap()
                .is_none()
        );
        connection
            .execute(
                "UPDATE document_study_guide_checkpoints SET section_json = '{}'",
                [],
            )
            .unwrap();
        assert!(
            load_connection(&connection, "document", &key, &topic, &catalog)
                .unwrap()
                .is_none()
        );
    }
}
