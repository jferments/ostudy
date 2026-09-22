//! Durable reuse for collection Study Guide authoring.
//!
//! Cache identity is derived from the exact semantic evidence for one window,
//! while admission separately rechecks current source hashes. Unrelated
//! collection diagnostics therefore do not invalidate reusable paid work.

use std::collections::BTreeMap;

use rusqlite::{params, OptionalExtension, TransactionBehavior};
use serde_json::json;

use crate::{
    ai::{document_intelligence::DocumentTaskContext, generation},
    database::Database,
    document_study_guides::{
        validate_study_guide_section_content, GroundedStudyGuideSection,
        STUDY_GUIDE_PROMPT_VERSION, STUDY_GUIDE_SCHEMA_VERSION,
    },
    error::CommandError,
    knowledge::{
        authoring::KnowledgeCatalog,
        collection_view::CollectionGuideWindow,
    },
    util::unix_time_millis,
};

const COLLECTION_SECTION_CACHE_VERSION: i64 = 1;

pub(crate) fn guide_contract(task: &DocumentTaskContext) -> Result<String, CommandError> {
    Ok(generation::hash(
        serde_json::to_string(&(
            "collection-guide-section",
            COLLECTION_SECTION_CACHE_VERSION,
            STUDY_GUIDE_PROMPT_VERSION,
            STUDY_GUIDE_SCHEMA_VERSION,
            task.contract_identity()?,
        ))
        .map_err(CommandError::serialization)?
        .as_bytes(),
    ))
}

pub(crate) fn section_key(
    task: &DocumentTaskContext,
    collection_id: &str,
    collection_title: &str,
    window: &CollectionGuideWindow,
    relationship_context: &str,
) -> Result<String, CommandError> {
    Ok(generation::hash(
        serde_json::to_string(&(
            "collection-guide-window",
            COLLECTION_SECTION_CACHE_VERSION,
            guide_contract(task)?,
            collection_id,
            collection_title,
            &window.block_key,
            &window.title,
            &window.topic_ids,
            &window.topic_context,
            relationship_context,
            window.catalog.request_fingerprint()?,
        ))
        .map_err(CommandError::serialization)?
        .as_bytes(),
    ))
}

pub(crate) fn load_section(
    database: &Database,
    collection_id: &str,
    cache_key: &str,
    contract_hash: &str,
    catalog: &KnowledgeCatalog,
    diagnostics: Option<&generation::Context>,
) -> Result<Option<GroundedStudyGuideSection>, CommandError> {
    let dependencies = source_dependencies(catalog)?;
    let stored = database.with_connection(|connection| {
        validate_dependencies(connection, &dependencies)?;
        let row = connection
            .query_row(
                "SELECT result_json,result_hash
                 FROM collection_guide_checkpoints
                 WHERE cache_key=?1 AND collection_id=?2 AND contract_hash=?3",
                params![cache_key, collection_id, contract_hash],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(CommandError::database)?;
        let Some((serialized, expected_hash)) = row else {
            return Ok(None);
        };
        let mut statement = connection
            .prepare(
                "SELECT source_id FROM collection_guide_checkpoint_sources
                 WHERE cache_key=?1 ORDER BY source_id",
            )
            .map_err(CommandError::database)?;
        let saved_sources = statement
            .query_map([cache_key], |row| row.get::<_, String>(0))
            .map_err(CommandError::database)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(CommandError::database)?;
        if saved_sources != dependencies.keys().cloned().collect::<Vec<_>>()
            || generation::hash(serialized.as_bytes()) != expected_hash
        {
            return Ok(None);
        }
        let section: GroundedStudyGuideSection =
            match serde_json::from_str(&serialized) {
                Ok(section) => section,
                Err(_) => return Ok(None),
            };
        if !catalog.validates(&section.knowledge_support, catalog.units.len())
            || validate_study_guide_section_content(section.content.clone()).is_err()
        {
            return Ok(None);
        }
        Ok(Some(section))
    })?;
    if stored.is_some() {
        if let Some(diagnostics) = diagnostics {
            diagnostics.reused("guide_authoring", cache_key);
        }
    }
    Ok(stored)
}

pub(crate) fn save_section(
    database: &Database,
    collection_id: &str,
    cache_key: &str,
    contract_hash: &str,
    catalog: &KnowledgeCatalog,
    section: &GroundedStudyGuideSection,
) -> Result<(), CommandError> {
    if !catalog.validates(&section.knowledge_support, catalog.units.len()) {
        return Err(CommandError::new(
            "collection_checkpoint_invalid",
            "A generated collection section no longer matches its admitted knowledge.",
        ));
    }
    validate_study_guide_section_content(section.content.clone())?;
    let dependencies = source_dependencies(catalog)?;
    let serialized = serde_json::to_string(section).map_err(CommandError::serialization)?;
    if serialized.len() > 2 * 1024 * 1024 {
        return Err(CommandError::new(
            "collection_checkpoint_too_large",
            "The generated collection section exceeds its durable checkpoint bound.",
        ));
    }
    let result_hash = generation::hash(serialized.as_bytes());
    database.with_connection(|connection| {
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(CommandError::database)?;
        validate_dependencies(&tx, &dependencies)?;
        tx.execute(
            "INSERT INTO collection_guide_checkpoints(
                 cache_key,collection_id,contract_hash,result_json,result_hash,created_at
             ) VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(cache_key) DO UPDATE SET
                 collection_id=excluded.collection_id,
                 contract_hash=excluded.contract_hash,
                 result_json=excluded.result_json,
                 result_hash=excluded.result_hash,
                 created_at=excluded.created_at",
            params![
                cache_key,
                collection_id,
                contract_hash,
                serialized,
                result_hash,
                unix_time_millis()?
            ],
        )
        .map_err(CommandError::database)?;
        tx.execute(
            "DELETE FROM collection_guide_checkpoint_sources WHERE cache_key=?1",
            [cache_key],
        )
        .map_err(CommandError::database)?;
        for source_id in dependencies.keys() {
            tx.execute(
                "INSERT INTO collection_guide_checkpoint_sources(cache_key,source_id)
                 VALUES(?1,?2)",
                params![cache_key, source_id],
            )
            .map_err(CommandError::database)?;
        }
        tx.commit().map_err(CommandError::database)
    })
}

fn source_dependencies(catalog: &KnowledgeCatalog) -> Result<BTreeMap<String, String>, CommandError> {
    let mut result = BTreeMap::new();
    for unit in &catalog.units {
        insert_dependency(&mut result, &unit.source_id, &unit.source_hash)?;
        if let Some(grounding) = &unit.content.grounding {
            for evidence in &grounding.evidence {
                insert_dependency(&mut result, &evidence.source_id, &evidence.source_hash)?;
            }
        }
    }
    Ok(result)
}

fn insert_dependency(
    dependencies: &mut BTreeMap<String, String>,
    source_id: &str,
    source_hash: &str,
) -> Result<(), CommandError> {
    if dependencies
        .insert(source_id.to_string(), source_hash.to_string())
        .is_some_and(|previous| previous != source_hash)
    {
        return Err(CommandError::new(
            "source_set_changed",
            "The selected evidence contains conflicting source revisions.",
        ));
    }
    Ok(())
}

fn validate_dependencies(
    connection: &rusqlite::Connection,
    dependencies: &BTreeMap<String, String>,
) -> Result<(), CommandError> {
    for (source_id, expected_hash) in dependencies {
        let current = connection
            .query_row(
                "SELECT content_hash FROM knowledge_sources
                 WHERE id=?1 AND active=1",
                [source_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(CommandError::database)?;
        if current.as_ref() != Some(expected_hash) {
            return Err(CommandError::new(
                "source_set_changed",
                "The source evidence changed while a collection section was being authored.",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document_study_guides::StudyGuideSectionContent,
        knowledge::authoring::tests::fixture,
    };

    #[test]
    fn durable_section_reuse_preserves_admitted_result_and_invalidates_on_source_change() {
        let (database, source, catalog, _) = fixture();
        let collection_id = "c".repeat(32);
        database
            .with_connection(|connection| {
                connection
                    .execute(
                        "INSERT INTO document_collections(
                             id,parent_id,name,sort_order,archived_at,created_at,updated_at,revision
                         ) VALUES(?1,NULL,'Reuse fixture',1000,NULL,1,1,1)",
                        [&collection_id],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            })
            .unwrap();

        let support = catalog
            .select(&[catalog.units[0].id.clone()], 64)
            .expect("fixture support should select");
        let section = GroundedStudyGuideSection {
            content: StudyGuideSectionContent {
                overview: "Triangles have three sides and are defined by three connected edges."
                    .to_string(),
                key_points: vec![
                    "A triangle has three sides.".to_string(),
                    "The statement is grounded in the selected source.".to_string(),
                ],
                ..Default::default()
            },
            knowledge_support: support,
            catalog_fingerprint: catalog.fingerprint().unwrap(),
        };
        let key = "a".repeat(64);
        let contract = "b".repeat(64);
        save_section(
            &database,
            &collection_id,
            &key,
            &contract,
            &catalog,
            &section,
        )
        .expect("section should checkpoint");

        let mut operation =
            generation::Operation::begin(&database, None, None, Some(&collection_id), "fixture")
                .expect("diagnostic operation should begin");
        let context = operation.context();
        let loaded = load_section(
            &database,
            &collection_id,
            &key,
            &contract,
            &catalog,
            Some(&context),
        )
        .expect("checkpoint should load")
        .expect("checkpoint should be reusable");
        operation.finish(true);
        assert_eq!(loaded.content.overview, section.content.overview);
        database
            .with_connection(|connection| {
                let reused: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM generation_reuse_events
                         WHERE operation_id=?1 AND stage='guide_authoring'",
                        [context.operation_id.as_str()],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(reused, 1);
                connection
                    .execute(
                        "UPDATE knowledge_sources SET content_hash=?1 WHERE id=?2",
                        params!["d".repeat(64), source.id],
                    )
                    .map_err(CommandError::database)?;
                let remaining: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM collection_guide_checkpoints WHERE cache_key=?1",
                        [&key],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(remaining, 0);
                Ok(())
            })
            .unwrap();
    }

    #[test]
    fn request_identity_ignores_unrelated_preparation_counters() {
        let (_, _, catalog, _) = fixture();
        let original = catalog.request_fingerprint().unwrap();
        let mut changed = catalog.clone();
        changed.preparation.unprepared_sources += 100;
        changed.preparation.omitted_sources += 100;
        assert_eq!(original, changed.request_fingerprint().unwrap());
    }
}
