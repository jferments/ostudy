use super::*;
use crate::{
    authoring::card_types_and_templates::generated_import::import_generated_flashcards_bytes_with_commit_context,
    document_card_topics::{link_imported_notes_to_topics, SourceGroundedGeneratedCard},
    knowledge::authoring::{self, tests::fixture},
};

fn grounded(
    catalog: &KnowledgeCatalog,
    locator: crate::document_domain::SourceLocator,
) -> SourceGroundedGeneratedCard {
    let card = json!({"type":"interactive","question":{
        "kind":"true_false","statement":"A triangle has three sides.",
        "correct":true,"explanation":"This is the defining number of sides."}});
    let proposal = AssessmentProposal {
        criterion: "The defining number of sides of a triangle".to_string(),
        ambiguous: false,
        components: ["prompt", "statement"].into_iter().map(|key| ComponentProposal {
            key: key.to_string(),
            stance: EvidenceStance::Supports,
            knowledge_unit_ids: vec![catalog.units[0].id.clone()],
        }).collect(),
    };
    let admitted = admit_card(card, proposal, catalog, &policy::GenerationPolicy::default()).unwrap();
    SourceGroundedGeneratedCard {
        assessment: record_from_card(&admitted).unwrap(),
        knowledge_support: authoring::card_support(&admitted).unwrap(),
        card: authoring::without_card_support(admitted),
        source_locator: locator,
    }
}

#[test]
fn typed_note_component_evidence_topics_and_schedules_publish_atomically() {
    let (database, source, catalog, plan) = fixture();
    let card = grounded(&catalog, source.locator.clone().unwrap());
    let payload = json!({"format":"ostudy-flashcards","formatVersion":2,
        "deck":{"mode":"create","name":"Grounded questions"},
        "cards":[card.card.clone(),card.card.clone()]}).to_string();
    let records = vec![card.clone(), card];
    let result = import_generated_flashcards_bytes_with_commit_context(
        &database,
        payload.as_bytes(),
        |connection, _, imported, now| {
            assert_eq!(imported.created_notes.len(), 1);
            assert_eq!(imported.created_notes[0].source_index, 0);
            link_imported_notes_to_topics(connection, &source.owner_id, &"3".repeat(32),
                &plan, &records, imported, now)
        },
    ).unwrap();
    assert_eq!(result.created_note_count, 1);
    assert_eq!(result.created_card_count, 1);
    assert_eq!(result.skipped_duplicate_count, 1);
    database.with_connection(|connection| {
        let (record, revision, matches): (String, i64, i64) = connection.query_row(
            "SELECT e.record_json, e.note_revision, e.question_hash = ?1
             FROM question_generation_evidence e JOIN notes n ON n.id = e.note_id
             WHERE n.revision = e.note_revision AND e.document_id = ?2",
            rusqlite::params![records[0].assessment.as_ref().unwrap().question_hash, source.owner_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).map_err(CommandError::database)?;
        assert_eq!(revision, 1);
        assert_eq!(matches, 1);
        let stored: AssessmentRecord = serde_json::from_str(&record).unwrap();
        assert_eq!(Some(stored), records[0].assessment);
        let review_count: i64 = connection.query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))
            .map_err(CommandError::database)?;
        assert_eq!(review_count, 0);
        let errors: i64 = connection.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0))
            .map_err(CommandError::database)?;
        assert_eq!(errors, 0);
        Ok(())
    }).unwrap();
}

#[test]
fn stale_component_evidence_or_failed_finalization_rolls_back_all_new_rows() {
    for corrupt in [false, true] {
        let (database, source, catalog, plan) = fixture();
        let mut card = grounded(&catalog, source.locator.clone().unwrap());
        if corrupt {
            card.assessment.as_mut().unwrap().components[0].units[0].source_hash = "f".repeat(64);
        }
        let payload = json!({"format":"ostudy-flashcards","formatVersion":2,
            "deck":{"mode":"create","name":"Rollback questions"},
            "cards":[card.card.clone()]}).to_string();
        let result = import_generated_flashcards_bytes_with_commit_context(
            &database,
            payload.as_bytes(),
            |connection, _, imported, now| {
                link_imported_notes_to_topics(connection, &source.owner_id, &"3".repeat(32),
                    &plan, std::slice::from_ref(&card), imported, now)?;
                Err(CommandError::new("injected_finalization_failure", "Stop before commit"))
            },
        );
        let error = result.unwrap_err();
        if corrupt { assert_ne!(error.code, "injected_finalization_failure"); }
        else { assert_eq!(error.code, "injected_finalization_failure"); }
        database.with_connection(|connection| {
            for table in ["decks", "notes", "cards", "card_schedules", "review_events",
                "question_generation_evidence", "document_note_topics", "knowledge_artifact_links"] {
                let count: i64 = connection.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert_eq!(count, 0, "transaction leaked {table}");
            }
            Ok(())
        }).unwrap();
    }
}

#[test]
fn policy_changes_invalidate_candidates_without_invalidating_canonical_knowledge() {
    let catalog = authoring::tests::catalog_fixture();
    let before = catalog.fingerprint().unwrap();
    let mut policy = policy::GenerationPolicy::default();
    let first = policy::checkpoint_contract(Some("contract".to_string()), &policy).unwrap();
    policy.max_compound_items = 4;
    let second = policy::checkpoint_contract(Some("contract".to_string()), &policy).unwrap();
    assert_ne!(first, second);
    assert_eq!(before, catalog.fingerprint().unwrap());
    assert_eq!(policy::checkpoint_contract(None, &policy).unwrap(), None);
}
