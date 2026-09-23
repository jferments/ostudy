use super::*;
use crate::{
    ai::generation::{self, Operation, RecoveryReason},
    database::Database,
};
use serde_json::Value;
use std::task::Poll;

fn fixture() -> (Database, Operation, Context) {
    let database = Database::open_in_memory().unwrap();
    let operation = Operation::begin(&database, None, None, None, "authoring_fixture").unwrap();
    let context = operation.context();
    (database, operation, context)
}

async fn response(context: &Context, kind: Kind, key: &str) {
    generation::scoped_request(
        Some(context.clone()),
        kind.stage(),
        Some(key),
        "fixture",
        async {
            let mut attempt = generation::start_current_attempt(&json!({
                "model": "fixture", "max_tokens": 100,
            }))
            .unwrap();
            attempt.response(&json!({
                "usage": {"prompt_tokens": 10, "completion_tokens": 2, "total_tokens": 12},
                "choices": [{"finish_reason": "stop", "message": {"content": "PRIVATE_OUTPUT"}}],
            }));
        },
    )
    .await;
}

fn rows(database: &Database) -> Vec<(String, String, i64, Value)> {
    database
        .with_connection(|connection| {
            let mut query = connection
                .prepare(
                    "SELECT id,status,total_tokens,payload_bytes_json FROM generation_requests
                     ORDER BY stage,attempt,id",
                )
                .map_err(CommandError::database)?;
            let rows = query
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })
                .map_err(CommandError::database)?;
            rows.map(|row| {
                let (id, status, tokens, text) = row.map_err(CommandError::database)?;
                Ok((id, status, tokens, serde_json::from_str(&text).unwrap()))
            })
            .collect()
        })
        .unwrap()
}

#[tokio::test]
async fn document_authoring_outcomes_link_repair_without_claiming_publication() {
    let (database, _operation, context) = fixture();
    let expected = vec![json!({"content": "PRIVATE_CARD"})];
    let returned = observe(
        Some(&context),
        Some("window"),
        Kind::Flashcards,
        async {
            response(&context, Kind::Flashcards, "window").await;
            assert!(context.reserve_recovery(
                Kind::Flashcards.stage(),
                Some("window"),
                RecoveryReason::ContractRepair,
                "ai_document_output_invalid",
            ));
            response(&context, Kind::Flashcards, "window").await;
            Ok(expected.clone())
        },
        Vec::len,
    )
    .await
    .unwrap();
    assert_eq!(returned, expected);
    let rows = rows(&database);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].3["nativeTask"]["state"], "continued");
    let last = &rows[1].3;
    assert_eq!(last["nativeLineage"]["previousRequestId"], rows[0].0);
    assert_eq!(last["nativeLineage"]["recovery"]["reason"], "contract_repair");
    assert_eq!(last["nativeAuthoring"]["returnedItems"], 1);
    assert_eq!(last["nativeAuthoring"]["itemKind"], "cards");
    assert_eq!(last["nativeAuthoring"]["persistence"], "not_observed");
    assert_eq!(last["nativeTask"]["state"], "returned");
    assert!(last.get("nativeCheckpoint").is_none());
    assert!(rows.iter().all(|row| row.1 == "response" && row.2 == 12));
    assert!(!serde_json::to_string(&rows).unwrap().contains("PRIVATE_"));
}

#[tokio::test]
async fn document_authoring_outcomes_do_not_mix_parallel_artifact_results() {
    let (database, _operation, context) = fixture();
    let (cards, guide) = tokio::join!(
        observe(
            Some(&context),
            Some("cards"),
            Kind::Flashcards,
            async {
                response(&context, Kind::Flashcards, "cards").await;
                tokio::task::yield_now().await;
                Ok(vec![1, 2, 3])
            },
            Vec::len,
        ),
        observe(
            Some(&context),
            Some("guide"),
            Kind::GuideSection,
            async {
                response(&context, Kind::GuideSection, "guide").await;
                tokio::task::yield_now().await;
                Err::<Vec<usize>, _>(CommandError::new(
                    "invalid_guide_support",
                    "PRIVATE_VALIDATION_DETAIL",
                ))
            },
            Vec::len,
        ),
    );
    assert_eq!(cards.unwrap(), vec![1, 2, 3]);
    assert_eq!(guide.unwrap_err().code, "invalid_guide_support");
    let rows = rows(&database);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].3["nativeAuthoring"]["returnedItems"], 3);
    assert_eq!(rows[1].3["nativeAuthoring"]["state"], "failed");
    assert!(rows[1].3["nativeAuthoring"]["returnedItems"].is_null());
    assert_eq!(rows[1].3["nativeTask"]["errorCode"], "invalid_guide_support");
    assert!(rows.iter().all(|row| {
        row.1 == "response" && row.3["nativeLineage"]["previousRequestId"].is_null()
    }));
    assert!(!serde_json::to_string(&rows).unwrap().contains("PRIVATE_"));
}

#[tokio::test]
async fn document_authoring_outcomes_keep_curation_batches_and_empty_selection_distinct() {
    let (database, _operation, context) = fixture();
    let (empty, failed) = tokio::join!(
        observe(
            Some(&context),
            Some("batch_a"),
            Kind::CurationSelection,
            async {
                response(&context, Kind::CurationSelection, "batch_a").await;
                tokio::task::yield_now().await;
                Ok(Vec::<usize>::new())
            },
            Vec::len,
        ),
        observe(
            Some(&context),
            Some("batch_b"),
            Kind::CurationSelection,
            async {
                response(&context, Kind::CurationSelection, "batch_b").await;
                Err::<Vec<usize>, _>(CommandError::new("invalid_indices", "PRIVATE_ERROR"))
            },
            Vec::len,
        ),
    );
    assert!(empty.unwrap().is_empty());
    assert_eq!(failed.unwrap_err().code, "invalid_indices");
    let rows = rows(&database);
    assert_eq!(rows.len(), 2);
    let empty = rows
        .iter()
        .find(|row| row.3["nativeAuthoring"]["state"] == "returned")
        .unwrap();
    assert_eq!(empty.3["nativeAuthoring"]["returnedItems"], 0);
    assert_eq!(empty.3["nativeAuthoring"]["itemKind"], "curation_selections");
    let failed = rows
        .iter()
        .find(|row| row.3["nativeAuthoring"]["state"] == "failed")
        .unwrap();
    assert!(failed.3["nativeAuthoring"]["returnedItems"].is_null());
    assert!(rows.iter().all(|row| row.3.get("nativeCheckpoint").is_none()));
}

#[tokio::test]
async fn document_authoring_outcomes_do_not_invent_usage_before_any_request() {
    let (database, _operation, context) = fixture();
    let empty = observe(
        Some(&context),
        Some("empty"),
        Kind::Flashcards,
        async { Ok(Vec::<usize>::new()) },
        Vec::len,
    )
    .await
    .unwrap();
    assert!(empty.is_empty());
    let failure = observe(
        Some(&context),
        Some("no_evidence"),
        Kind::GuideSection,
        async { Err::<Vec<usize>, _>(CommandError::new("no_evidence", "No evidence")) },
        Vec::len,
    )
    .await
    .unwrap_err();
    assert_eq!(failure.code, "no_evidence");
    assert!(rows(&database).is_empty());
}

#[tokio::test]
async fn document_authoring_outcomes_mark_dropped_native_work_interrupted() {
    let (database, _operation, context) = fixture();
    let mut future = Box::pin(observe(
        Some(&context),
        Some("interrupted"),
        Kind::GuideSection,
        async {
            response(&context, Kind::GuideSection, "interrupted").await;
            std::future::pending::<Result<Vec<usize>, CommandError>>().await
        },
        Vec::len,
    ));
    std::future::poll_fn(|cx| {
        assert!(future.as_mut().poll(cx).is_pending());
        Poll::Ready(())
    })
    .await;
    drop(future);
    let rows = rows(&database);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1, "response");
    assert_eq!(rows[0].3["nativeTask"]["state"], "interrupted");
    assert!(rows[0].3.get("nativeAuthoring").is_none());
}

#[tokio::test]
async fn document_authoring_outcomes_write_failure_preserves_successful_result() {
    let (database, _operation, context) = fixture();
    let returned = observe(
        Some(&context),
        Some("accounting_failure"),
        Kind::Flashcards,
        async {
            response(&context, Kind::Flashcards, "accounting_failure").await;
            database.with_connection(|connection| {
                connection.execute_batch(
                    "CREATE TRIGGER reject_diagnostic_update BEFORE UPDATE OF payload_bytes_json
                     ON generation_requests BEGIN SELECT RAISE(FAIL, 'diagnostic fixture'); END;",
                ).map_err(CommandError::database)
            })?;
            Ok(vec![1, 2])
        },
        Vec::len,
    )
    .await
    .unwrap();
    assert_eq!(returned, vec![1, 2]);
    let rows = rows(&database);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1, "response");
    assert_eq!(rows[0].2, 12);
    assert!(rows[0].3.get("nativeAuthoring").is_none());
}

#[test]
fn document_authoring_observer_boxes_large_child_before_outer_state_machine() {
    let payload = [1_u8; 256 * 1024];
    let child = async move {
        std::future::pending::<()>().await;
        Ok::<_, CommandError>(usize::from(std::hint::black_box(payload)[0]))
    };
    assert!(std::mem::size_of_val(&child) >= 256 * 1024);
    let observed = observe(None, None, Kind::Flashcards, child, |n| *n);
    assert!(std::mem::size_of_val(&observed) < 4096);
}

#[test]
fn document_authoring_observer_stage_matches_original_request_owners() {
    for (file, kind) in [
        ("flashcards.rs", Kind::Flashcards),
        ("study_guide.rs", Kind::GuideSection),
        ("curation.rs", Kind::CurationSelection),
    ] {
        assert_eq!(generation::stage(file), kind.stage());
    }
}
