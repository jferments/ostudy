use std::cell::Cell;
use std::collections::BTreeMap;

use super::*;
use crate::ai::generation::{outcomes, payload, Operation};
use crate::database::Database;

const STAGE: &str = "knowledge_extraction";

fn key(index: usize) -> String {
    format!("{index:064x}")
}

fn body(index: usize) -> Value {
    json!({
        "model": "fixture", "max_tokens": 6000,
        "messages": [
            {"role": "system", "content": "Return the fixed native contract."},
            {"role": "user", "content": format!("PRIVATE_SOURCE_{index}")}
        ],
        "response_format": {"type": "json_object"}
    })
}

fn snapshot() -> outcomes::Snapshot {
    outcomes::Snapshot {
        accepted_units: 1,
        rejected_proposals: 1,
        unaccounted_sources: 0,
        unresolved_regions: Some(1),
        protocol_complete: false,
        rejection_rules: BTreeMap::from([("invalid_unit_content".into(), 1)]),
        deferred_annotations: Some(outcomes::DeferredAnnotations::default()),
    }
}

fn response(context: &Context, stage: &str, key: &str, body: &Value) {
    let mut attempt = context
        .start_attempt(stage, body, Some(key), "fixture")
        .unwrap();
    attempt.response(&json!({
        "usage": {"prompt_tokens": 10, "completion_tokens": 2, "total_tokens": 12}
    }));
}

async fn run(
    context: &Context,
    index: usize,
    before: outcomes::Snapshot,
    after: outcomes::Snapshot,
    body: Value,
) -> bool {
    let key = key(index);
    let mut task = outcomes::Task::new(Some(context), STAGE, Some(&key));
    let result = task
        .scope(async {
            response(context, STAGE, &key, &body);
            outcomes::checkpoint(before);
            let allowed = context.reserve_recovery(
                STAGE,
                Some(&key),
                RecoveryReason::ContractRepair,
                "partial_coverage",
            );
            if allowed {
                response(context, STAGE, &key, &body);
                outcomes::checkpoint(after.clone());
                // A duplicate final checkpoint must not become a second sample.
                outcomes::checkpoint(after);
            }
            Ok::<_, CommandError>(allowed)
        })
        .await;
    task.finish(&result);
    result.unwrap()
}

fn operation() -> (Database, Operation, Context) {
    let database = Database::open_in_memory().unwrap();
    let operation = Operation::begin(&database, None, None, None, "benefit_test").unwrap();
    let context = operation.context();
    (database, operation, context)
}

#[tokio::test]
async fn document_recovery_stops_identical_no_gain_siblings_without_a_third_paid_repair() {
    let (database, _operation, context) = operation();
    assert!(run(&context, 1, snapshot(), snapshot(), body(1)).await);
    assert!(run(&context, 2, snapshot(), snapshot(), body(2)).await);
    assert!(!run(&context, 3, snapshot(), snapshot(), body(3)).await);
    database
        .with_connection(|connection| {
            let counts: (i64, i64, i64) = connection
                .query_row(
                    "SELECT COUNT(*),SUM(input_tokens),SUM(output_tokens) FROM generation_requests",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(counts, (5, 50, 10));
            let encoded: String = connection
                .query_row(
                    "SELECT payload_bytes_json FROM generation_requests WHERE request_key=?1",
                    [key(3)],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let value: Value = serde_json::from_str(&encoded).unwrap();
            assert_eq!(
                value["nativeRecoveryPolicy"]["decision"],
                "stop_same_strategy"
            );
            assert_eq!(value["nativeRecoveryPolicy"]["noGainSiblingRoots"], 2);
            assert_eq!(
                value["nativeRecoveryDecisions"]["contract_repair_denied"]["allowed"],
                false
            );
            assert!(!encoded.contains("PRIVATE_SOURCE"));
            Ok(())
        })
        .unwrap();
    assert!(!context
        .recovery
        .lock()
        .unwrap()
        .tasks
        .contains_key(&format!("{STAGE}:{}", key(3))));
    assert!(context.reserve_recovery(
        STAGE,
        Some(&key(3)),
        RecoveryReason::TransientTransport,
        "ai_timeout"
    ));
}

#[tokio::test]
async fn document_recovery_does_not_count_unknown_or_partial_diagnostics_as_zero_gain() {
    let (_, _operation, context) = operation();
    let mut unknown = snapshot();
    unknown.deferred_annotations = None;
    for index in 1..=4 {
        assert!(run(&context, index, snapshot(), unknown.clone(), body(index)).await);
    }
    let mut malformed = serde_json::to_value(snapshot()).unwrap();
    malformed["acceptedUnits"] = json!(true);
    assert!(super::snapshot(&malformed).is_none());
    malformed["acceptedUnits"] = json!(1);
    malformed["rejectionRules"] = json!({});
    assert!(super::snapshot(&malformed).is_none());
}

#[tokio::test]
async fn document_recovery_recent_measured_progress_keeps_the_strategy_available() {
    let (_, _operation, context) = operation();
    let mut improved = snapshot();
    improved.accepted_units = 2;
    improved.rejected_proposals = 0;
    improved.rejection_rules.clear();
    improved.unresolved_regions = Some(0);
    improved.protocol_complete = true;
    assert!(run(&context, 1, snapshot(), snapshot(), body(1)).await);
    assert!(run(&context, 2, snapshot(), improved, body(2)).await);
    assert!(run(&context, 3, snapshot(), snapshot(), body(3)).await);
    assert!(run(&context, 4, snapshot(), snapshot(), body(4)).await);
    assert!(!run(&context, 5, snapshot(), snapshot(), body(5)).await);
}

#[tokio::test]
async fn document_recovery_compares_only_the_same_provider_contract_and_failure_fingerprint() {
    let (_, _operation, context) = operation();
    for index in 1..=2 {
        assert!(run(&context, index, snapshot(), snapshot(), body(index)).await);
    }
    let mut different_model = body(3);
    different_model["model"] = json!("another-model");
    assert!(run(&context, 3, snapshot(), snapshot(), different_model).await);
    let mut different_schema = body(4);
    different_schema["response_format"] = json!({"type": "new_contract"});
    assert!(run(&context, 4, snapshot(), snapshot(), different_schema).await);
    let mut different_failure = snapshot();
    different_failure.rejection_rules = BTreeMap::from([("invalid_evidence_selection".into(), 1)]);
    assert!(
        run(
            &context,
            5,
            different_failure.clone(),
            different_failure,
            body(5)
        )
        .await
    );
    let (_, _other_operation, other) = operation();
    assert!(run(&other, 3, snapshot(), snapshot(), body(3)).await);
}

#[tokio::test]
async fn document_recovery_duplicate_task_history_does_not_multiply_sibling_evidence() {
    let (_, _operation, context) = operation();
    assert!(run(&context, 1, snapshot(), snapshot(), body(1)).await);
    assert!(!run(&context, 1, snapshot(), snapshot(), body(1)).await);
    assert!(run(&context, 2, snapshot(), snapshot(), body(2)).await);
    assert!(!run(&context, 3, snapshot(), snapshot(), body(3)).await);
}

#[tokio::test]
async fn document_recovery_accounting_gap_does_not_borrow_an_earlier_failure() {
    let (_, _operation, context) = operation();
    for index in 1..=2 {
        assert!(run(&context, index, snapshot(), snapshot(), body(index)).await);
    }
    let key = key(3);
    let mut task = outcomes::Task::new(Some(&context), STAGE, Some(&key));
    let result = task
        .scope(async {
            response(&context, STAGE, &key, &body(3));
            outcomes::checkpoint(snapshot());
            outcomes::accounting_gap();
            assert!(context.reserve_recovery(
                STAGE,
                Some(&key),
                RecoveryReason::ContractRepair,
                "partial_coverage"
            ));
            Ok::<_, CommandError>(())
        })
        .await;
    task.finish(&result);
    result.unwrap();
}

fn rejected_card() -> Result<Vec<Value>, CommandError> {
    let error = CommandError::new("ai_document_output_invalid", "Fixture rejection");
    let mut report = outcomes::authoring::candidates::Report::default();
    report.rejected(0, "source_question", &error, true);
    report.declared = Some(1);
    report.boundary = "all_candidates_rejected";
    let result = Err(error);
    report.finish(&result);
    result
}

async fn cards(context: &Context, index: usize) -> bool {
    let key = key(index);
    let body = body(index);
    let allowed = Cell::new(false);
    let result = outcomes::authoring::observe(
        Some(context),
        Some(&key),
        outcomes::authoring::Kind::Flashcards,
        async {
            response(context, "flashcard_authoring", &key, &body);
            let first = rejected_card();
            let retry = context.reserve_recovery(
                "flashcard_authoring",
                Some(&key),
                RecoveryReason::ContractRepair,
                "ai_document_output_invalid",
            );
            allowed.set(retry);
            if retry {
                response(context, "flashcard_authoring", &key, &body);
                rejected_card()
            } else {
                first
            }
        },
        Vec::len,
    )
    .await;
    assert!(result.is_err());
    allowed.get()
}

#[tokio::test]
async fn document_recovery_all_rejected_card_repairs_use_real_admission_and_task_outcomes() {
    let (_, _operation, context) = operation();
    assert!(cards(&context, 1).await);
    assert!(cards(&context, 2).await);
    assert!(!cards(&context, 3).await);
}

#[test]
fn document_recovery_contract_fingerprint_excludes_sources_but_preserves_all_wire_controls() {
    let original = body(1);
    let measured = payload::measure(&original).unwrap();
    let fingerprint = &measured["requestContractFingerprint"];
    assert_eq!(
        fingerprint,
        &payload::measure(&body(2)).unwrap()["requestContractFingerprint"]
    );
    assert!(!measured.to_string().contains("PRIVATE_SOURCE"));
    for (field, replacement) in [
        ("model", json!("changed")),
        ("max_tokens", json!(3200)),
        ("temperature", json!(0.25)),
        ("response_format", json!({"type": "changed"})),
    ] {
        let mut changed = original.clone();
        changed[field] = replacement;
        assert_ne!(
            fingerprint,
            &payload::measure(&changed).unwrap()["requestContractFingerprint"]
        );
    }
    let mut changed = original;
    changed["messages"][0]["content"] = json!("Different system contract");
    assert_ne!(
        fingerprint,
        &payload::measure(&changed).unwrap()["requestContractFingerprint"]
    );
}

#[test]
fn document_recovery_contract_matches_real_extraction_schemas_without_equating_semantic_enums() {
    use crate::knowledge::{evidence::EvidenceCatalog, extraction::response_schema_for_test};

    let (_, first, _, _) = crate::knowledge::authoring::tests::fixture();
    let mut second = first.clone();
    second.segment_key = "1:0:0".into();
    second.refresh_identity().unwrap();
    assert_ne!(first.id, second.id);
    let request = |sources: &[crate::knowledge::contracts::SourceRecord]| {
        let catalog = EvidenceCatalog::build(sources, false).unwrap();
        let schema = response_schema_for_test(&catalog, sources);
        let mut request = body(1);
        request["response_format"] = json!({"schema": schema});
        request["messages"][0]["content"] = json!(format!(
            "Return JSON. <ostudy-output-schema>{schema}</ostudy-output-schema>"
        ));
        request
    };
    let first_request = request(&[first.clone()]);
    let second_request = request(&[second.clone()]);
    let fingerprint =
        |request: &Value| payload::measure(request).unwrap()["requestContractFingerprint"].clone();
    assert_eq!(fingerprint(&first_request), fingerprint(&second_request));
    assert_ne!(
        fingerprint(&first_request),
        fingerprint(&request(&[first, second]))
    );
    let mut semantic = first_request.clone();
    semantic["response_format"] = json!({"enum": ["fact", "process"]});
    let original = fingerprint(&semantic);
    semantic["response_format"] = json!({"enum": ["fact", "comparison"]});
    assert_ne!(original, fingerprint(&semantic));
    // The measurement function did not normalize or rewrite the wire requests.
    assert_ne!(first_request, second_request);
}
