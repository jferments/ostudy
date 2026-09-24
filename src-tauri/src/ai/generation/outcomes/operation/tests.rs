use super::*;
use crate::ai::generation;
use serde_json::Value;

fn operation(database: &Database) -> Operation {
    Operation::begin(database, None, None, None, "outcome_fixture").unwrap()
}

fn stored(database: &Database, id: &str) -> (String, Option<Value>) {
    database
        .with_connection(|connection| {
            let (state, value): (String, Option<String>) = connection
                .query_row(
                    "SELECT state,outcome_json FROM generation_operations WHERE id=?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(CommandError::database)?;
            Ok((
                state,
                value.map(|text| serde_json::from_str(&text).unwrap()),
            ))
        })
        .unwrap()
}

fn request_count(database: &Database) -> i64 {
    database
        .with_connection(|connection| {
            connection
                .query_row("SELECT COUNT(*) FROM generation_requests", [], |row| {
                    row.get(0)
                })
                .map_err(CommandError::database)
        })
        .unwrap()
}

#[test]
fn document_operation_records_pre_request_failure_without_message_or_fabricated_usage() {
    let database = Database::open_in_memory().unwrap();
    let mut operation = operation(&database);
    let id = operation.context().operation_id;
    let result: Result<(), CommandError> = Err(CommandError::new(
        "document_source_missing",
        "PRIVATE SOURCE PATH AND PROVIDER CONTENT",
    ));
    operation.finish_result(&result);
    let (state, outcome) = stored(&database, &id);
    let outcome = outcome.unwrap();
    assert_eq!(state, "failed");
    assert_eq!(outcome["errorCode"], "document_source_missing");
    assert_eq!(outcome["resultObserved"], true);
    assert_eq!(outcome["artifactPublication"], "not_observed");
    assert!(!outcome.to_string().contains("PRIVATE"));
    assert_eq!(request_count(&database), 0);
    operation.finish_result(&Ok::<_, CommandError>(()));
    drop(operation);
    assert_eq!(stored(&database, &id), (state, Some(outcome)));
}

#[test]
fn document_operation_success_is_not_artifact_publication_and_legacy_cause_stays_unknown() {
    let database = Database::open_in_memory().unwrap();
    let mut complete = operation(&database);
    let id = complete.context().operation_id;
    complete.finish_result(&Ok::<_, CommandError>("returned only"));
    let outcome = stored(&database, &id).1.unwrap();
    assert_eq!(outcome["state"], "complete");
    assert!(outcome["errorCode"].is_null());
    assert_eq!(outcome["artifactPublication"], "not_observed");
    let mut legacy = operation(&database);
    let legacy_id = legacy.context().operation_id;
    legacy.finish(false);
    assert_eq!(stored(&database, &legacy_id), ("failed".into(), None));
    assert_eq!(request_count(&database), 0);
}

#[test]
fn document_operation_diagnostic_failure_does_not_change_the_original_result() {
    let database = Database::open_in_memory().unwrap();
    let mut operation = operation(&database);
    let id = operation.context().operation_id;
    database
        .with_connection(|connection| {
            connection
                .execute_batch(
                    "CREATE TRIGGER reject_outcome BEFORE UPDATE OF outcome_json
                     ON generation_operations BEGIN SELECT RAISE(ABORT,'fixture failure'); END;",
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    let result: Result<_, CommandError> = Ok("already committed elsewhere");
    operation.finish_result(&result);
    assert_eq!(result.unwrap(), "already committed elsewhere");
    assert_eq!(stored(&database, &id), ("running".into(), None));
    drop(operation);
    assert_eq!(stored(&database, &id), ("running".into(), None));
}

#[test]
fn document_operation_startup_interruption_is_atomic_idempotent_and_not_a_return() {
    let database = Database::open_in_memory().unwrap();
    let mut running = operation(&database);
    let id = running.context().operation_id;
    generation::reconcile_startup(&database).unwrap();
    let first = stored(&database, &id);
    assert_eq!(first.0, "interrupted");
    let outcome = first.1.as_ref().unwrap();
    assert_eq!(outcome["errorCode"], "native_process_interrupted");
    assert_eq!(outcome["resultObserved"], false);
    generation::reconcile_startup(&database).unwrap();
    running.finish_result(&Ok::<_, CommandError>(()));
    assert_eq!(stored(&database, &id), first);
    assert_eq!(request_count(&database), 0);
}

#[test]
fn document_operation_unsafe_error_codes_and_device_transfer_do_not_leak_content() {
    let database = Database::open_in_memory().unwrap();
    let mut operation = operation(&database);
    let id = operation.context().operation_id;
    let result: Result<(), CommandError> = Err(CommandError::new("PRIVATE secret", "PRIVATE"));
    operation.finish_result(&result);
    let outcome = stored(&database, &id).1.unwrap();
    assert_eq!(outcome["errorCode"], "unclassified_native_error");
    assert!(!outcome.to_string().contains("PRIVATE"));
    database
        .with_connection(|connection| {
            generation::sanitize_for_device_transfer(connection)?;
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM generation_operations", [], |row| {
                    row.get(0)
                })
                .map_err(CommandError::database)?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
}

#[tokio::test]
async fn document_operation_real_task_owner_captures_failure_before_any_provider_request() {
    let database = Database::open_in_memory().unwrap();
    let document_id = crate::util::generate_id().unwrap();
    database
        .with_connection(|connection| {
            connection
                .execute(
                    "INSERT INTO documents(id,title,original_filename,mime_type,byte_size,sha256,
                     source_relative_path,lifecycle_state,created_at,updated_at)
                     VALUES (?1,'Fixture','fixture.pdf','application/pdf',1,?2,
                             'documents/fixture.pdf','ready',1,1)",
                    params![document_id, "2".repeat(64)],
                )
                .map_err(CommandError::database)?;
            Ok(())
        })
        .unwrap();
    let mut id = None;
    let result = crate::task_cancellation::run_document_task(&document_id, async {
        bind_document(&database);
        id = crate::task_cancellation::generation_context(&database)?
            .map(|context| context.operation_id);
        Err::<(), _>(CommandError::new(
            "document_source_missing",
            "PRIVATE SOURCE PATH",
        ))
    })
    .await;
    assert_eq!(result.unwrap_err().code, "document_source_missing");
    let (state, outcome) = stored(&database, &id.unwrap());
    assert_eq!(state, "failed");
    assert_eq!(outcome.unwrap()["errorCode"], "document_source_missing");
    assert_eq!(request_count(&database), 0);
    assert!(!crate::task_cancellation::is_document_task_active(
        &document_id
    ));
}
