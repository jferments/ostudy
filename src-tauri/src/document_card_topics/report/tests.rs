use rusqlite::params;
use serde_json::{json, Value};

use super::*;
use crate::{
    authoring::card_types_and_templates::generated_import::import_generated_flashcards_bytes_with_commit_context,
    database::Database,
    document_domain::{SourceLocator, SourceUnitKind},
};

fn fixture() -> (Database, String, String) {
    let database = Database::open_in_memory().unwrap();
    let document = "a".repeat(32);
    let job = "b".repeat(32);
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO documents (
                         id, title, original_filename, mime_type, byte_size, sha256,
                         source_relative_path, lifecycle_state, created_at, updated_at
                     ) VALUES (?1, 'Numbers', 'numbers.pdf', 'application/pdf', 1,
                               ?2, 'documents/source/numbers.pdf', 'processing', 1, 1)",
                    params![document, "c".repeat(64)],
                )
                .map_err(CommandError::database)?;
            add_job(connection, &document, &job)
        })
        .unwrap();
    (database, document, job)
}

fn add_job(connection: &Connection, document: &str, job: &str) -> Result<(), CommandError> {
    connection
        .execute(
            "INSERT INTO document_generation_jobs (
                 id, document_id, provider_kind, model_id, profile_revision,
                 credential_epoch, network_scope, state, page_count,
                 processed_page_count, generated_candidate_count, created_at, updated_at
             ) VALUES (?1, ?2, 'openai_compatible', 'fixture', 1,
                       0, 'local', 'committing', 1, 1, 0, 1, 1)",
            params![job, document],
        )
        .map_err(CommandError::database)?;
    Ok(())
}

fn cards() -> Vec<SourceGroundedGeneratedCard> {
    let basic = json!({"type":"basic", "front":"Number word?", "back":"One",
        "reverseEnabled":true});
    let matching = json!({"type":"interactive", "question":{
        "kind":"matching", "prompt":"Match number words.", "pairs":[
            {"id":"one", "left":"one", "right":"1"},
            {"id":"two", "left":"two", "right":"2"}]}});
    let catalog = crate::knowledge::authoring::tests::catalog_fixture();
    let support = catalog.select(&["a".repeat(32)], 2).unwrap();
    [basic.clone(), basic, matching]
        .into_iter()
        .map(|card| SourceGroundedGeneratedCard {
            card,
            source_locator: SourceLocator::new(SourceUnitKind::Page, vec![1], None).unwrap(),
            knowledge_support: support.clone(),
            // Reporting tests count existing support; semantic admission has its
            // own component-evidence publication tests and is not inferred here.
            assessment: None,
        })
        .collect()
}

fn commit(
    database: &Database,
    document: &str,
    job: &str,
    rollback: bool,
) -> Result<(), CommandError> {
    let cards = cards();
    let payload = json!({"format":"ostudy-flashcards", "formatVersion":2,
        "deck":{"mode":"create", "name":"Reported questions"},
        "cards":cards.iter().map(|card| &card.card).collect::<Vec<_>>()});
    import_generated_flashcards_bytes_with_commit_context(
        database,
        payload.to_string().as_bytes(),
        |connection, _, committed, now| {
            publish(connection, document, job, &cards, committed, now)?;
            connection
                .execute(
                    "UPDATE document_generation_jobs SET state = 'succeeded' WHERE id = ?1",
                    [job],
                )
                .map_err(CommandError::database)?;
            if rollback {
                return Err(CommandError::new("fixture_rollback", "Reject this publication."));
            }
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn counts_committed_notes_presentations_items_and_unique_source_support() {
    let (database, document, job) = fixture();
    let policy = GenerationPolicy::default();
    begin(&database, &document, &job, &policy, 2).unwrap();
    record_candidates(&database, &job, 4).unwrap();
    commit(&database, &document, &job, false).unwrap();
    database
        .with_connection(|connection| {
            let report = load_latest(connection, &document)?.unwrap();
            assert_eq!(report.requested_policy, policy);
            assert_eq!(report.candidate_note_count, 4);
            assert_eq!(report.selected_note_count, 3);
            assert_eq!(report.curation_removed_count, 1);
            assert_eq!(report.saved_note_count, 2);
            assert_eq!(report.saved_card_count, 3);
            assert_eq!(report.skipped_duplicate_count, 1);
            assert_eq!(report.item_count, 4);
            assert_eq!(report.available_knowledge_units, 2);
            assert_eq!(report.linked_knowledge_units, 1);
            assert_eq!(report.evidence_binding_count, 2);
            assert_eq!(report.types.len(), 2);
            let matching = report
                .types
                .iter()
                .find(|row| row.family == QuestionFamily::Matching)
                .unwrap();
            assert_eq!(
                (matching.note_count, matching.card_count, matching.item_count),
                (1, 1, 2)
            );
            let json = serde_json::to_string(&report).unwrap();
            assert!(!json.contains("Number word?"));
            assert!(!json.contains(&"a".repeat(32)));
            assert!(!json.contains("knowledgeUnitIds"));
            let reviews: i64 = connection
                .query_row("SELECT COUNT(*) FROM review_events", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(reviews, 0);
            Ok(())
        })
        .unwrap();
}

#[test]
fn failed_publication_rolls_back_the_summary_and_retry_keeps_one_completion() {
    let (database, document, job) = fixture();
    begin(&database, &document, &job, &GenerationPolicy::default(), 2).unwrap();
    record_candidates(&database, &job, 3).unwrap();
    assert_eq!(commit(&database, &document, &job, true).unwrap_err().code, "fixture_rollback");
    database
        .with_connection(|connection| {
            assert!(load_latest(connection, &document)?.is_none());
            let notes: i64 = connection
                .query_row("SELECT COUNT(*) FROM notes", [], |row| row.get(0))
                .map_err(CommandError::database)?;
            assert_eq!(notes, 0);
            Ok(())
        })
        .unwrap();
    commit(&database, &document, &job, false).unwrap();
    database
        .with_connection(|connection| {
            assert_eq!(load_latest(connection, &document)?.unwrap().saved_card_count, 3);
            assert!(connection
                .execute("UPDATE question_generation_reports SET candidate_count = 100", [])
                .is_err());
            Ok(())
        })
        .unwrap();
}

#[test]
fn pending_or_failed_jobs_do_not_replace_a_successful_summary() {
    let (database, document, job) = fixture();
    begin(&database, &document, &job, &GenerationPolicy::default(), 2).unwrap();
    record_candidates(&database, &job, 3).unwrap();
    commit(&database, &document, &job, false).unwrap();
    let next = "d".repeat(32);
    database.with_connection(|connection| add_job(connection, &document, &next)).unwrap();
    begin(&database, &document, &next, &GenerationPolicy::default(), 4).unwrap();
    record_candidates(&database, &next, 10).unwrap();
    database
        .with_connection(|connection| {
            let reports = load_latest_many(connection, &[&document, &"f".repeat(32)])?;
            assert_eq!(reports.len(), 1);
            assert_eq!(reports[&document].candidate_note_count, 3);
            assert_eq!(reports[&document].available_knowledge_units, 2);
            connection
                .execute(
                    "UPDATE document_generation_jobs SET state = 'failed' WHERE id = ?1",
                    [&next],
                )
                .map_err(CommandError::database)?;
            assert_eq!(load_latest(connection, &document)?.unwrap().candidate_note_count, 3);
            Ok(())
        })
        .unwrap();
}

#[test]
fn policy_and_catalog_snapshot_cannot_be_replaced_during_a_running_job() {
    let (database, document, job) = fixture();
    let policy = GenerationPolicy::default();
    begin(&database, &document, &job, &policy, 2).unwrap();
    begin(&database, &document, &job, &policy, 2).unwrap();
    let mut changed = policy.clone();
    changed.max_compound_items = 16;
    assert!(begin(&database, &document, &job, &changed, 2).is_err());
    assert!(begin(&database, &document, &job, &policy, 3).is_err());
    record_candidates(&database, &job, 3).unwrap();
    assert!(record_candidates(&database, &job, 4).is_err());
}

#[test]
fn unsupported_or_inconsistent_report_json_is_not_silently_displayed() {
    let (database, document, job) = fixture();
    begin(&database, &document, &job, &GenerationPolicy::default(), 2).unwrap();
    record_candidates(&database, &job, 3).unwrap();
    commit(&database, &document, &job, false).unwrap();
    database
        .with_connection(|connection| {
            let report = load_latest(connection, &document)?.unwrap();
            let mut bad = report.clone();
            bad.saved_card_count += 1;
            assert!(bad.validate().is_err());
            let mut json: Value = serde_json::to_value(&report).unwrap();
            json["unexpected"] = json!(true);
            assert!(serde_json::from_value::<GenerationReport>(json).is_err());
            let mut bad = report;
            bad.schema_version = 2;
            assert!(bad.validate().is_err());
            Ok(())
        })
        .unwrap();
}
