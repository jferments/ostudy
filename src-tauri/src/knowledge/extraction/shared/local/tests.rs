use super::*;
use crate::knowledge::{contracts::KnowledgeKind, typed_payload::TypedPayload};

fn draft() -> Value {
    json!({
        "kind": "quantity", "subject": {"text": "Sides", "occurrence": 1},
        "bounds": [{"label": {"text": "=", "occurrence": 0},
                    "value": {"text": "3", "occurrence": 0}}],
        "unit": null
    })
}

fn response() -> Value {
    json!({"units": [
        {"title": "Side count", "kind": "fact", "statement": "Sides = 3",
         "qualifications": ["For a triangle."], "topic": "Geometry",
         "importance": "core", "evidenceIds": ["E0"], "details": draft()},
        {"title": "Triangle", "kind": "definition",
         "statement": "A triangle has three sides.", "qualifications": [],
         "topic": "Geometry", "importance": "core", "evidenceIds": ["E0"]}
    ], "noLearningSourceIds": []})
}

fn historical(draft: Value, statement: &str) -> TypedPayload {
    let error = TypedPayload::admit(draft.clone(), statement, KnowledgeKind::Fact).unwrap_err();
    let encoded = draft.to_string();
    serde_json::from_value(json!({
        "deferredVersion": 1, "kind": "fact", "statementHash": hash(statement),
        "rule": error.code, "draftHash": hash(&encoded),
        "draftBytes": encoded.len(), "draft": draft
    }))
    .unwrap()
}

fn retained(input: Window<'_>) -> decode::AnalysisReport {
    let mut report = decode::initial(response(), input.catalog, input.sources).unwrap();
    report.accepted[0].content.details = Some(historical(draft(), "Sides = 3"));
    analysis_store::validate_report(input.sources, &report).unwrap();
    analysis_store::Checkpoint::open(input.database, input.sources, input.contract)
        .unwrap()
        .save(&report)
        .unwrap();
    report
}

fn window<'a>(
    database: &'a Database,
    sources: &'a [SourceRecord],
    catalog: &'a EvidenceCatalog,
    contract: &'a str,
) -> Window<'a> {
    Window {
        database,
        sources,
        catalog,
        contract,
        authorization: contract,
        retry_partial: false,
        diagnostics: None,
        logical_key: None,
    }
}

async fn resume(input: Window<'_>) {
    super::super::run(input, &CancellationSignal::new(), |_| async {
        panic!("retained annotation recovery must not purchase another interpretation")
    })
    .await
    .unwrap();
}

fn load(input: Window<'_>) -> decode::AnalysisReport {
    analysis_store::Checkpoint::open(input.database, input.sources, input.contract)
        .unwrap()
        .load()
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn document_shared_first_pass_normalizes_annotation_with_one_provider_call() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("shared first pass normalization");
    let input = window(&database, &sources, &catalog, &contract);
    let mut calls = 0;
    super::super::run(input, &CancellationSignal::new(), |_| {
        calls += 1;
        async { Ok(response()) }
    })
    .await
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(load(input).accepted.len(), 2);
    assert!(load(input).deferred_annotations().is_empty());
}

#[tokio::test]
async fn document_shared_annotation_only_repair_persists_without_changing_core_or_siblings() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("retained annotation normalization");
    let mut input = window(&database, &sources, &catalog, &contract);
    let original = retained(input);
    let sibling = serde_json::to_value(&original.accepted[1]).unwrap();
    let grounding = serde_json::to_value(&original.accepted[0].content.grounding).unwrap();
    for retry in [false, true] {
        input.retry_partial = retry;
        resume(input).await;
        let saved = load(input);
        assert_eq!(saved.accepted.len(), original.accepted.len());
        assert_eq!(saved.rejected_count(), original.rejected_count());
        assert!(saved.deferred_annotations().is_empty());
        assert_eq!(serde_json::to_value(&saved.accepted[1]).unwrap(), sibling);
        assert_eq!(
            serde_json::to_value(&saved.accepted[0].content.grounding).unwrap(),
            grounding
        );
        assert_eq!(saved.accepted[0].content.statement, "Sides = 3");
        assert_eq!(
            saved.accepted[0].content.qualifications,
            ["For a triangle."]
        );
    }
}

#[tokio::test]
async fn document_shared_ambiguous_annotation_stays_deferred_without_paid_recovery() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("ambiguous annotation receipt");
    let mut input = window(&database, &sources, &catalog, &contract);
    input.retry_partial = true;
    let mut report = retained(input);
    let statement = "Sides = 3; Sides = 3";
    let mut ambiguous = draft();
    ambiguous["subject"]["occurrence"] = json!(2);
    report.accepted[0].content.statement = statement.into();
    report.accepted[0].content.details = Some(historical(ambiguous, statement));
    analysis_store::Checkpoint::open(&database, &sources, &contract)
        .unwrap()
        .save(&report)
        .unwrap();
    resume(input).await;
    assert_eq!(load(input).deferred_annotations().len(), 1);
    assert_eq!(
        serde_json::to_value(load(input)).unwrap(),
        serde_json::to_value(report).unwrap()
    );
}

#[test]
fn document_shared_local_repair_rejects_invalid_core_and_tampered_debt() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("invalid local repair inputs");
    let input = window(&database, &sources, &catalog, &contract);
    let original = retained(input);
    let mut invalid = original.clone();
    invalid.accepted[0].content.title.clear();
    assert!(repair(&invalid, &sources).is_err());
    let mut invalid = original;
    invalid.accepted[0].content.statement = "Sides = 4".into();
    assert!(repair(&invalid, &sources).is_err());
    assert_eq!(load(input).deferred_annotations().len(), 1);
}

#[tokio::test]
async fn document_shared_local_repair_reopens_after_conflict_and_preserves_newer_sibling() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("local repair publication conflict");
    let input = window(&database, &sources, &catalog, &contract);
    let original = retained(input);
    let repaired = repair(&original, &sources).unwrap().unwrap();
    let mut stale = analysis_store::Checkpoint::open(&database, &sources, &contract).unwrap();
    let mut current = analysis_store::Checkpoint::open(&database, &sources, &contract).unwrap();
    let mut newer = original;
    newer.accepted[1].content.title = "Newer triangle definition".into();
    current.save(&newer).unwrap();
    assert!(stale.save(&repaired).is_err());
    assert_eq!(load(input).deferred_annotations().len(), 1);
    resume(input).await;
    let saved = load(input);
    assert!(saved.deferred_annotations().is_empty());
    assert_eq!(saved.accepted[1].content.title, "Newer triangle definition");
}

#[tokio::test]
async fn document_shared_failed_annotation_save_rolls_back_and_retries_without_provider() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("local annotation transaction failure");
    let input = window(&database, &sources, &catalog, &contract);
    let original = retained(input);
    database
        .with_connection(|connection| {
            connection
                .execute_batch(
                    "CREATE TEMP TRIGGER reject_annotation BEFORE INSERT ON knowledge_units
                     BEGIN SELECT RAISE(ABORT, 'annotation test failure'); END;",
                )
                .map_err(CommandError::database)
        })
        .unwrap();
    let failed = super::super::run(input, &CancellationSignal::new(), |_| async {
        panic!("a failed local transaction must not trigger another provider call")
    })
    .await;
    assert!(failed.is_err());
    assert_eq!(
        serde_json::to_value(load(input)).unwrap(),
        serde_json::to_value(original).unwrap()
    );
    database
        .with_connection(|connection| {
            connection
                .execute_batch("DROP TRIGGER reject_annotation")
                .map_err(CommandError::database)
        })
        .unwrap();
    resume(input).await;
    assert!(load(input).deferred_annotations().is_empty());
}

#[tokio::test]
async fn document_local_repair_does_not_attribute_its_save_to_an_enclosing_provider_request() {
    use crate::ai::generation::{outcomes, Operation};

    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("isolated local annotation repair");
    let input = window(&database, &sources, &catalog, &contract);
    retained(input);
    let operation = Operation::begin(&database, None, None, None, "local_repair_test").unwrap();
    let context = operation.context();
    let key = hash("enclosing guide request");
    let mut parent = outcomes::Task::new(Some(&context), "guide_authoring", Some(&key));
    let result = parent
        .scope(async {
            let mut attempt = context.start_attempt(
                "guide_authoring",
                &json!({"model": "fixture", "max_tokens": 3200, "messages": []}),
                Some(&key),
                "fixture",
            )?;
            attempt.response(&json!({"usage": {"prompt_tokens": 10, "completion_tokens": 2}}));
            super::super::run(input, &CancellationSignal::new(), |_| async {
                panic!("local repair must not request a model")
            })
            .await
        })
        .await;
    parent.finish(&result);
    result.unwrap();
    database
        .with_connection(|connection| {
            let measured: Option<String> = connection
                .query_row(
                    "SELECT json_type(payload_bytes_json, '$.nativeCheckpoint')
                     FROM generation_requests WHERE stage='guide_authoring'",
                    [],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            assert!(measured.is_none());
            Ok(())
        })
        .unwrap();
    assert!(load(input).deferred_annotations().is_empty());
}
