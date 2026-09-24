//! Real candidate admission plus the physical-request ledger. No provider or
//! replacement validator is involved; fixtures establish contracts, not truth.
use super::*;
use crate::{
    ai::generation::{
        self,
        outcomes::authoring::{observe, Kind},
        Context, Operation, RecoveryReason,
    },
    database::Database,
};

fn recall(id: &str, front: &str) -> Value {
    json!({"card":{"type":"basic","front":front,"back":"Three sides."},
        "knowledgeUnitIds":[id]})
}

fn fixture() -> (Database, Operation, Context) {
    let database = Database::open_in_memory().unwrap();
    let operation = Operation::begin(&database, None, None, None, "candidate_fixture").unwrap();
    let context = operation.context();
    (database, operation, context)
}

async fn response(context: &Context, key: &str, kind: Kind) {
    generation::scoped_request(
        Some(context.clone()),
        kind.stage(),
        Some(key),
        "fixture",
        async {
            let mut attempt = generation::start_current_attempt(&json!({
                "model":"fixture", "max_tokens":100,
            }))
            .unwrap();
            attempt.response(&json!({
                "usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12},
                "choices":[{"finish_reason":"stop","message":{"content":"PRIVATE_OUTPUT"}}],
            }));
        },
    )
    .await;
}

fn rows(database: &Database) -> Vec<Value> {
    database
        .with_connection(|connection| {
            let mut query = connection
                .prepare(
                    "SELECT id,status,total_tokens,payload_bytes_json FROM generation_requests
                     ORDER BY attempt,id",
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
                let payload: Value = serde_json::from_str(&text).unwrap();
                Ok(json!({"id":id,"status":status,"totalTokens":tokens,"payload":payload}))
            })
            .collect()
        })
        .unwrap()
}

async fn run(value: Value, maximum: usize) -> (Result<Vec<Value>, CommandError>, Value) {
    let (database, _operation, context) = fixture();
    let catalog = authoring::tests::catalog_fixture();
    let result = observe(
        Some(&context),
        Some("window"),
        Kind::Flashcards,
        async {
            response(&context, "window", Kind::Flashcards).await;
            parse_knowledge_cards(value, maximum, &catalog)
        },
        Vec::len,
    )
    .await;
    let records = rows(&database);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["status"], "response");
    assert_eq!(records[0]["totalTokens"], 12);
    assert!(!records[0].to_string().contains("PRIVATE_"));
    (result, records[0]["payload"].clone())
}

#[tokio::test]
async fn document_candidate_outcomes_preserve_siblings_and_specific_boundaries() {
    let good = recall("K0", "Retained question?");
    let (result, payload) = run(
        json!({"cards":[
            null,
            recall("K0", "Which of the following: one, two, three, four?"),
            recall("K999", "PRIVATE_INVENTED_EVIDENCE?"),
            recall("K0", "```PRIVATE_UNCLOSED"),
            {"card":{"type":"interactive"}},
            good
        ]}),
        8,
    )
    .await;
    let cards = result.unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0]["front"], "Retained question?");
    assert_eq!(cards[0]["back"], "Three sides.");
    assert_eq!(
        authoring::card_support(&cards[0]).unwrap()[0].unit_id,
        "a".repeat(32)
    );
    let record = &payload["nativeAdmission"];
    assert_eq!(record["state"], "returned");
    assert_eq!(record["declaredCandidates"], 6);
    assert_eq!(record["examinedCandidates"], 6);
    assert_eq!(record["admittedCandidates"], 1);
    assert_eq!(record["rejectedCandidates"], 5);
    assert_eq!(record["selectedCandidates"], 1);
    assert_eq!(record["unexaminedCandidates"], 0);
    assert!(record["failureBoundary"].is_null());
    for (index, rule) in [
        "candidate_envelope",
        "flattened_choice_recall",
        "knowledge_selection",
        "recall_content",
        "assessment_missing",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(record["rejections"][index]["candidateIndex"], index);
        assert_eq!(record["rejections"][index]["rule"], *rule);
        assert_eq!(record["rejections"][index]["retryable"], true);
    }
    assert_eq!(record["persistence"], "not_observed");
    assert!(payload.get("nativeCheckpoint").is_none());
}

#[tokio::test]
async fn document_candidate_outcomes_empty_malformed_and_oversized_are_not_conflated() {
    let (empty, payload) = run(json!({"cards":[]}), 2).await;
    assert!(empty.unwrap().is_empty());
    assert_eq!(payload["nativeAdmission"]["declaredCandidates"], 0);
    assert_eq!(payload["nativeAdmission"]["selectedCandidates"], 0);
    for malformed in [Value::Null, json!({}), json!({"cards":null})] {
        let (result, payload) = run(malformed, 2).await;
        assert!(result.is_err());
        let record = &payload["nativeAdmission"];
        assert_eq!(record["failureBoundary"], "response_envelope");
        assert!(record["declaredCandidates"].is_null());
        assert!(record["selectedCandidates"].is_null());
        assert_eq!(record["examinedCandidates"], 0);
    }
    let (result, payload) = run(json!({"cards":vec![Value::Null;129]}), 2).await;
    assert!(result.is_err());
    let record = &payload["nativeAdmission"];
    assert_eq!(record["failureBoundary"], "response_bounds");
    assert_eq!(record["declaredCandidates"], 129);
    assert_eq!(record["unexaminedCandidates"], 129);
    assert_eq!(record["rejectedCandidates"], 0);
    assert_eq!(record["rejections"], json!([]));
}

#[tokio::test]
async fn document_candidate_outcomes_all_rejected_preserve_the_original_error() {
    let raw = json!({"cards":[recall("K999", "PRIVATE_BAD?"),null]});
    let expected =
        parse_knowledge_cards(raw.clone(), 2, &authoring::tests::catalog_fixture()).unwrap_err();
    let (result, payload) = run(raw, 2).await;
    let error = result.unwrap_err();
    assert_eq!(error.code, expected.code);
    assert_eq!(error.message, expected.message);
    assert!(retryable_contract_error(&error));
    assert_eq!(
        payload["nativeAdmission"]["failureBoundary"],
        "all_candidates_rejected"
    );
    assert_eq!(payload["nativeAdmission"]["rejectedCandidates"], 2);
    assert!(payload["nativeAdmission"]["selectedCandidates"].is_null());
    assert_eq!(payload["nativeTask"]["state"], "failed");
    assert_eq!(payload["nativeTask"]["errorCode"], error.code);
}

#[tokio::test]
async fn document_candidate_outcomes_selection_does_not_invent_rejections() {
    let (result, payload) = run(
        json!({"cards":[recall("K0", "First?"),recall("K0", "Second?")]}),
        1,
    )
    .await;
    assert_eq!(result.unwrap().len(), 1);
    let record = &payload["nativeAdmission"];
    assert_eq!(record["admittedCandidates"], 2);
    assert_eq!(record["selectedCandidates"], 1);
    assert_eq!(record["unselectedAdmittedCandidates"], 1);
    assert_eq!(record["rejectedCandidates"], 0);
}

#[tokio::test]
async fn document_candidate_outcomes_recovery_keeps_each_response_separate() {
    let (database, _operation, context) = fixture();
    let catalog = authoring::tests::catalog_fixture();
    let result = observe(
        Some(&context),
        Some("window"),
        Kind::Flashcards,
        async {
            response(&context, "window", Kind::Flashcards).await;
            let error = parse_knowledge_cards(json!({"cards":[null]}), 2, &catalog).unwrap_err();
            assert!(context.reserve_recovery(
                "flashcard_authoring",
                Some("window"),
                RecoveryReason::ContractRepair,
                &error.code,
            ));
            response(&context, "window", Kind::Flashcards).await;
            parse_knowledge_cards(json!({"cards":[recall("K0", "Repaired?")]}), 2, &catalog)
        },
        Vec::len,
    )
    .await;
    assert_eq!(result.unwrap().len(), 1);
    let records = rows(&database);
    assert_eq!(records.len(), 2);
    assert_eq!(
        records[0]["payload"]["nativeAdmission"]["rejectedCandidates"],
        1
    );
    assert_eq!(records[0]["payload"]["nativeAdmission"]["state"], "failed");
    assert_eq!(records[0]["payload"]["nativeTask"]["state"], "continued");
    assert_eq!(
        records[1]["payload"]["nativeAdmission"]["rejectedCandidates"],
        0
    );
    assert_eq!(
        records[1]["payload"]["nativeLineage"]["previousRequestId"],
        records[0]["id"]
    );
    assert!(records
        .iter()
        .all(|r| r["status"] == "response" && r["totalTokens"] == 12));
}

#[tokio::test]
async fn document_candidate_outcomes_diagnostic_failure_never_changes_admission() {
    let (database, _operation, context) = fixture();
    let catalog = authoring::tests::catalog_fixture();
    let result = observe(
        Some(&context),
        Some("window"),
        Kind::Flashcards,
        async {
            response(&context, "window", Kind::Flashcards).await;
            database
                .with_connection(|c| {
                    c.execute_batch(
                        "CREATE TRIGGER deny_diagnostics BEFORE UPDATE ON generation_requests
                         BEGIN SELECT RAISE(FAIL,'fixture'); END;",
                    )
                    .map_err(CommandError::database)
                })
                .unwrap();
            parse_knowledge_cards(json!({"cards":[recall("K0", "Retained?")]}), 2, &catalog)
        },
        Vec::len,
    )
    .await;
    assert_eq!(result.unwrap()[0]["front"], "Retained?");
    let records = rows(&database);
    assert_eq!(records.len(), 1);
    assert!(records[0]["payload"].get("nativeAdmission").is_none());
    assert_eq!(records[0]["totalTokens"], 12);
}

#[tokio::test]
async fn document_candidate_outcomes_no_request_and_wrong_stage_cannot_claim_admission() {
    let (database, _operation, context) = fixture();
    let catalog = authoring::tests::catalog_fixture();
    let no_request = observe(
        Some(&context),
        Some("local"),
        Kind::Flashcards,
        async { parse_knowledge_cards(json!({"cards":[]}), 2, &catalog) },
        Vec::len,
    )
    .await;
    assert!(no_request.unwrap().is_empty());
    assert!(rows(&database).is_empty());
    let wrong_stage = observe(
        Some(&context),
        Some("guide"),
        Kind::GuideSection,
        async {
            response(&context, "guide", Kind::GuideSection).await;
            parse_knowledge_cards(json!({"cards":[]}), 2, &catalog)
        },
        Vec::len,
    )
    .await;
    assert!(wrong_stage.unwrap().is_empty());
    assert!(rows(&database)[0]["payload"]
        .get("nativeAdmission")
        .is_none());
}

#[tokio::test]
async fn document_candidate_outcomes_invalid_policy_remains_a_pre_admission_failure() {
    let (database, _operation, context) = fixture();
    let catalog = authoring::tests::catalog_fixture();
    let policy = GenerationPolicy {
        allowed_types: vec![],
        ..GenerationPolicy::default()
    };
    let result = observe(
        Some(&context),
        Some("window"),
        Kind::Flashcards,
        async {
            response(&context, "window", Kind::Flashcards).await;
            parse_knowledge_cards_with_policy(json!({"cards":[]}), 2, &catalog, &policy)
        },
        Vec::len,
    )
    .await;
    assert!(result.is_err());
    let record = &rows(&database)[0]["payload"]["nativeAdmission"];
    assert_eq!(record["failureBoundary"], "policy");
    assert!(record["declaredCandidates"].is_null());
    assert!(record["selectedCandidates"].is_null());
}
