use super::*;
use crate::{ai::generation::{self, Operation}, database::Database};

fn fixture() -> (Database, Operation, Context) {
    let database = Database::open_in_memory().unwrap();
    let operation = Operation::begin(&database, None, None, None, "outcome_fixture").unwrap();
    let context = operation.context();
    (database, operation, context)
}

fn snapshot(accepted: u32, rejected: u32) -> Snapshot {
    Snapshot {
        accepted_units: accepted,
        rejected_proposals: rejected,
        unresolved_regions: Some(rejected),
        protocol_complete: rejected == 0,
        ..Default::default()
    }
}

fn request(context: &Context, key: &str) -> super::super::Attempt {
    context.start_attempt(
        "knowledge_extraction", &json!({"model":"fixture","max_tokens":100}),
        Some(&context.logical_request_key("knowledge_extraction", Some(key))), "fixture",
    ).unwrap()
}

fn respond(attempt: &mut super::super::Attempt) {
    attempt.response(&json!({"usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12},
        "choices":[{"finish_reason":"stop","message":{"content":"PRIVATE_PROVIDER_TEXT"}}]}));
}

fn payload(database: &Database, id: &str) -> Value {
    database.with_connection(|c| {
        let text: String = c.query_row("SELECT payload_bytes_json FROM generation_requests WHERE id=?1", [id], |r| r.get(0))
            .map_err(CommandError::database)?;
        Ok(serde_json::from_str(&text).unwrap())
    }).unwrap()
}

#[tokio::test]
async fn document_outcomes_link_recovery_and_preserve_gains_across_duplicate_save() {
    let (database, _operation, context) = fixture();
    let mut task = Task::new(Some(&context), "knowledge_extraction", Some("window"));
    let result: Result<_, CommandError> = task.scope(async {
        let mut first = request(&context, "window");
        respond(&mut first);
        checkpoint(snapshot(1, 1));
        assert!(context.reserve_recovery("knowledge_extraction", Some("window"), RecoveryReason::ContractRepair, "partial_coverage"));
        assert!(!context.reserve_recovery("knowledge_extraction", Some("window"), RecoveryReason::ContractRepair, "partial_coverage"));
        let mut second = request(&context, "window");
        respond(&mut second);
        checkpoint(snapshot(2, 0));
        checkpoint(snapshot(2, 0));
        Ok((first.id.clone(), second.id.clone()))
    }).await;
    task.finish(&result);
    let (first, second) = result.unwrap();
    let old = payload(&database, &first);
    let new = payload(&database, &second);
    assert_eq!(old["nativeTask"]["state"], "continued");
    assert_eq!(old["nativeRecoveryDecisions"]["contract_repair_allowed"]["allowed"], true);
    assert_eq!(old["nativeRecoveryDecisions"]["contract_repair_denied"]["allowed"], false);
    assert_eq!(new["nativeLineage"]["previousRequestId"], first);
    assert_eq!(new["nativeLineage"]["recovery"]["triggerRequestId"], first);
    assert_eq!(new["nativeCheckpoint"]["before"]["acceptedUnits"], 1);
    assert_eq!(new["nativeCheckpoint"]["gain"]["acceptedUnitsDelta"], 1);
    assert_eq!(new["nativeCheckpoint"]["gain"]["rejectedProposalsDelta"], -1);
    assert_eq!(new["nativeCheckpoint"]["gain"]["hasNativeProgress"], true);
    assert_eq!(new["nativeTask"]["state"], "returned");
    assert!(!new.to_string().contains("PRIVATE_PROVIDER_TEXT"));
    database.with_connection(|c| {
        let totals: (i64, i64, i64) = c.query_row("SELECT COUNT(*),SUM(total_tokens),SUM(status='response') FROM generation_requests", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?)))
            .map_err(CommandError::database)?;
        assert_eq!(totals, (2,24,2));
        Ok(())
    }).unwrap();
}

#[tokio::test]
async fn document_outcomes_keep_resume_zero_gain_and_failure_separate() {
    let (database, _operation, context) = fixture();
    let mut task = Task::new(Some(&context), "knowledge_extraction", Some("resume"));
    let mut id = String::new();
    let result: Result<(), CommandError> = task.scope(async {
        baseline(snapshot(1, 1));
        assert!(context.reserve_recovery("knowledge_extraction", Some("resume"), RecoveryReason::ContractRepair, "partial_coverage"));
        let mut attempt = request(&context, "resume");
        id = attempt.id.clone();
        respond(&mut attempt);
        checkpoint(snapshot(1, 1));
        Err(CommandError::new("stale_source", "PRIVATE_NATIVE_ERROR"))
    }).await;
    task.finish(&result);
    let value = payload(&database, &id);
    assert_eq!(value["nativeCheckpoint"]["gain"]["hasNativeProgress"], false);
    assert_eq!(value["nativeLineage"]["recovery"]["reason"], "contract_repair");
    assert!(value["nativeLineage"]["previousRequestId"].is_null());
    assert_eq!(value["nativeTask"]["errorCode"], "stale_source");
    assert_eq!(value["nativeTask"]["state"], "failed");
    assert!(!value.to_string().contains("PRIVATE_NATIVE_ERROR"));
}

#[tokio::test]
async fn document_outcomes_do_not_cross_interleaved_task_scopes() {
    let (database, _operation, context) = fixture();
    let mut a = Task::new(Some(&context), "knowledge_extraction", Some("a"));
    let mut b = Task::new(Some(&context), "knowledge_extraction", Some("b"));
    let (ra, rb): (Result<_, CommandError>, Result<_, CommandError>) = tokio::join!(
        a.scope(async {
            let mut attempt = request(&context, "a");
            tokio::task::yield_now().await;
            respond(&mut attempt);
            checkpoint(snapshot(1,0));
            Ok(attempt.id.clone())
        }),
        b.scope(async {
            let mut attempt = request(&context, "b");
            tokio::task::yield_now().await;
            respond(&mut attempt);
            checkpoint(snapshot(2,0));
            Ok(attempt.id.clone())
        }),
    );
    a.finish(&ra);
    b.finish(&rb);
    for (id, count) in [(ra.unwrap(),1), (rb.unwrap(),2)] {
        let value = payload(&database, &id);
        assert_eq!(value["nativeCheckpoint"]["after"]["acceptedUnits"], count);
        assert!(value["nativeLineage"]["previousRequestId"].is_null());
    }
}

#[tokio::test]
async fn document_outcomes_do_not_fabricate_requests_for_reuse_or_attach_after_accounting_gap() {
    let (database, _operation, context) = fixture();
    let mut task = Task::new(Some(&context), "knowledge_extraction", Some("gap"));
    let result: Result<_, CommandError> = task.scope(async {
        baseline(snapshot(1,0));
        let mut first = request(&context, "gap");
        respond(&mut first);
        checkpoint(snapshot(1,0));
        accounting_gap();
        checkpoint(snapshot(9,0));
        Ok(first.id.clone())
    }).await;
    task.finish(&result);
    let value = payload(&database, &result.unwrap());
    assert_eq!(value["nativeTask"]["state"], "linkage_lost");
    assert_eq!(value["nativeCheckpoint"]["after"]["acceptedUnits"], 1);
    let mut reused = Task::new(Some(&context), "knowledge_extraction", Some("reuse"));
    let result: Result<(), CommandError> = reused.scope(async { baseline(snapshot(2,0)); Ok(()) }).await;
    reused.finish(&result);
    database.with_connection(|c| {
        let count: i64 = c.query_row("SELECT COUNT(*) FROM generation_requests", [], |r|r.get(0)).map_err(CommandError::database)?;
        assert_eq!(count,1);
        Ok(())
    }).unwrap();
}

#[tokio::test]
async fn document_outcomes_interrupt_and_startup_reconcile_without_changing_usage() {
    let (database, _operation, context) = fixture();
    let task = Task::new(Some(&context), "knowledge_extraction", Some("drop"));
    let id = task.scope(async {
        let mut attempt = request(&context, "drop");
        respond(&mut attempt);
        attempt.id.clone()
    }).await;
    drop(task);
    assert_eq!(payload(&database, &id)["nativeTask"]["state"], "interrupted");
    let running = Task::new(Some(&context), "knowledge_extraction", Some("startup"));
    let id = running.scope(async {
        let mut attempt = request(&context, "startup");
        respond(&mut attempt);
        attempt.id.clone()
    }).await;
    generation::reconcile_startup(&database).unwrap();
    let value = payload(&database,&id);
    assert_eq!(value["nativeTask"]["state"], "interrupted");
    assert_eq!(value["nativeTask"]["errorCode"], "native_process_interrupted");
    database.with_connection(|c| {
        let values: (String,i64) = c.query_row("SELECT status,total_tokens FROM generation_requests WHERE id=?1",[&id],|r|Ok((r.get(0)?,r.get(1)?))).map_err(CommandError::database)?;
        assert_eq!(values, ("response".into(),12));
        Ok(())
    }).unwrap();
}

#[test]
fn document_outcomes_report_native_regression_and_unknown_legacy_regions() {
    let mut before = snapshot(2,0);
    before.unresolved_regions = None;
    let value = snapshot(1,1).gain(&before);
    assert_eq!(value["hasNativeRegression"],true);
    assert_eq!(value["acceptedUnitsDelta"],-1);
    assert!(value["unresolvedRegionsDelta"].is_null());
}
