use super::*;
use crate::ai::generation::Operation;
use crate::knowledge::annotation_demand;
use std::{cell::Cell, task::Poll};

fn policy(family: QuestionFamily) -> GenerationPolicy {
    GenerationPolicy {
        allowed_types: vec![family],
        ..GenerationPolicy::default()
    }
}

fn details() -> Value {
    json!({"kind": "quantity", "subject": {"text": "Sides", "occurrence": 0},
        "bounds": [{"label": {"text": "=", "occurrence": 0},
                    "value": {"text": "3", "occurrence": 0}}], "unit": null})
}

fn response() -> Value {
    let mut bad = details();
    bad["unit"] = json!({"text": "meters", "occurrence": 0});
    json!({"units": [
        {"title": "Sides", "kind": "fact", "statement": "Sides = 3",
         "qualifications": ["For triangles."], "topic": "Geometry", "importance": "core",
         "evidenceIds": ["E0"], "details": bad},
        {"title": "Triangle", "kind": "definition", "statement": "A triangle has three sides.",
         "qualifications": [], "topic": "Geometry", "importance": "core", "evidenceIds": ["E0"]}
    ], "noLearningSourceIds": []})
}

fn reply() -> Value {
    json!({"annotationRepairs": [{"slot": 0, "details": details()}]})
}

fn input<'a>(
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
        logical_key: Some(contract),
    }
}

fn load(input: Window<'_>) -> decode::AnalysisReport {
    analysis_store::Checkpoint::open(input.database, input.sources, input.contract)
        .unwrap()
        .load()
        .unwrap()
        .unwrap()
}

fn retain(input: Window<'_>) -> decode::AnalysisReport {
    let report = decode::initial(response(), input.catalog, input.sources).unwrap();
    assert_eq!(report.deferred_annotations().len(), 1);
    save(input, &report);
    report
}

fn save(input: Window<'_>, report: &decode::AnalysisReport) {
    analysis_store::Checkpoint::open(input.database, input.sources, input.contract)
        .unwrap()
        .save(report)
        .unwrap();
}

async fn demanded_run<F, Fut>(
    input: Window<'_>,
    policy: &GenerationPolicy,
    call: F,
) -> Result<(), CommandError>
where
    F: FnMut(WindowCall) -> Fut,
    Fut: Future<Output = Result<Value, CommandError>>,
{
    let signal = CancellationSignal::new();
    annotation_demand::scope(policy, super::super::run(input, &signal, call))
        .await
        .map(|_| ())
}

#[tokio::test]
async fn document_annotation_demand_is_absent_for_recall_explanations_and_soft_mixes() {
    for selected in [
        None,
        Some(GenerationPolicy::default()),
        Some(policy(QuestionFamily::Recall)),
    ] {
        let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        let contract = hash("no structural requirement");
        let input = input(&database, &sources, &catalog, &contract);
        let calls = Cell::new(0);
        let signal = CancellationSignal::new();
        let future = super::super::run(input, &signal, |_| {
            calls.set(calls.get() + 1);
            async { Ok(response()) }
        });
        match selected {
            Some(policy) => annotation_demand::scope(&policy, future).await.unwrap(),
            None => future.await.unwrap(),
        };
        assert_eq!(calls.get(), 1);
        assert_eq!(load(input).deferred_annotations().len(), 1);
        assert!(annotation_demand::current().is_none());
    }
}

#[tokio::test]
async fn document_annotation_demand_repairs_only_the_named_structure_and_reuses_it() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("required structured support");
    let input = input(&database, &sources, &catalog, &contract);
    let before = retain(input);
    let original = serde_json::to_value(&before.accepted[1]).unwrap();
    let policy = policy(QuestionFamily::SingleChoice);
    let calls = Cell::new(0);
    for _ in 0..2 {
        demanded_run(input, &policy, |call| {
            calls.set(calls.get() + 1);
            assert_eq!(
                call.schema["properties"]["annotationRepairs"]["items"]["properties"]["slot"]
                    ["enum"],
                json!([0])
            );
            assert!(call.user.contains("MUST NOT be rewritten"));
            assert!(call.user.contains("For triangles."));
            assert!(call.user.contains("failureRule"));
            async { Ok(reply()) }
        })
        .await
        .unwrap();
    }
    assert_eq!(calls.get(), 1);
    let saved = load(input);
    assert!(saved.deferred_annotations().is_empty());
    assert_eq!(saved.accepted[0].content.statement, "Sides = 3");
    assert_eq!(saved.accepted[0].content.qualifications, ["For triangles."]);
    assert_eq!(serde_json::to_value(&saved.accepted[1]).unwrap(), original);
    assert_eq!(
        serde_json::to_value(&saved.accepted[0].content.grounding).unwrap(),
        serde_json::to_value(&before.accepted[0].content.grounding).unwrap()
    );
}

#[tokio::test]
async fn document_annotation_ordering_demand_does_not_invent_a_process_for_numeric_facts() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("ordering cannot change fact kind");
    let input = input(&database, &sources, &catalog, &contract);
    retain(input);
    demanded_run(input, &policy(QuestionFamily::Ordering), |_| async {
        panic!("an ordering preference does not authorize changing a fact into a process")
    })
    .await
    .unwrap();
    assert_eq!(load(input).deferred_annotations().len(), 1);
}

#[tokio::test]
async fn document_annotation_demand_respects_spent_parent_recovery_budget() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("demand shares original recovery budget");
    let mut input = input(&database, &sources, &catalog, &contract);
    retain(input);
    let operation = Operation::begin(&database, None, None, None, "annotation_test").unwrap();
    let context = operation.context();
    input.diagnostics = Some(&context);
    assert!(context.reserve_recovery(
        "knowledge_extraction",
        Some(&contract),
        crate::ai::generation::RecoveryReason::ContractRepair,
        "initial_failure"
    ));
    demanded_run(input, &policy(QuestionFamily::SingleChoice), |_| async {
        panic!("a demanded annotation must not acquire a fresh parent allowance")
    })
    .await
    .unwrap();
    assert_eq!(load(input).deferred_annotations().len(), 1);
}

#[test]
fn document_annotation_repair_cannot_rewrite_core_guess_slots_or_admit_invalid_structure() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("strict annotation patch");
    let before = retain(input(&database, &sources, &catalog, &contract));
    let mut invalid = details();
    invalid["bounds"][0]["value"]["text"] = json!("4");
    for value in [
        json!({"annotationRepairs": [{"slot": 0, "details": null}]}),
        json!({"annotationRepairs": [{"slot": 0, "details": invalid}]}),
        json!({"annotationRepairs": [{"slot": 1, "details": details()}]}),
        json!({"annotationRepairs": [{"slot": 0, "details": details(), "statement": "changed"}]}),
        json!({"annotationRepairs": [{"slot": 0, "details": details()}], "units": []}),
        json!({"annotationRepairs": [{"slot": 0, "details": details()}, {"slot": 0, "details": details()}]}),
    ] {
        assert!(replacements(&before, &[0], value).is_empty());
    }
    let patches = replacements(&before, &[0], reply());
    assert_eq!(patches.len(), 1);
    let after = apply(&before, &before, &patches).unwrap();
    assert!(after.deferred_annotations().is_empty());
}

#[tokio::test]
async fn document_annotation_requests_are_bounded_while_all_demanded_targets_are_preserved() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("bounded annotation targets");
    let input = input(&database, &sources, &catalog, &contract);
    let mut before = retain(input);
    for index in 1..=MAX_TARGETS {
        let mut unit = before.accepted[0].clone();
        unit.content.title = format!("Independent target {index}");
        before.accepted.push(unit);
    }
    save(input, &before);
    let policy = policy(QuestionFamily::SingleChoice);
    let selected = targets(&before, &policy);
    assert_eq!(selected.len(), MAX_TARGETS + 1);
    let calls = Cell::new(0);
    demanded_run(input, &policy, |call| {
        calls.set(calls.get() + 1);
        let batch = call.schema["properties"]["annotationRepairs"]["items"]["properties"]
            ["slot"]["enum"].as_array().unwrap();
        assert!(batch.len() <= MAX_TARGETS);
        let value = json!({"annotationRepairs": batch.iter().map(|slot| json!({"slot": slot, "details": details()})).collect::<Vec<_>>()});
        async move { Ok(value) }
    })
    .await
    .unwrap();
    assert_eq!(calls.get(), 2);
    let saved = load(input);
    assert_eq!(saved.accepted.len(), before.accepted.len());
    assert!(saved.deferred_annotations().is_empty());
    assert!(saved.is_complete());
    for (old, new) in before.accepted.iter().zip(&saved.accepted) {
        assert_eq!(old.content.statement, new.content.statement);
        assert_eq!(old.content.qualifications, new.content.qualifications);
    }
}

#[tokio::test]
async fn document_annotation_concurrent_demand_shares_one_model_response() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("shared annotation proposal");
    let input = input(&database, &sources, &catalog, &contract);
    retain(input);
    let signal = CancellationSignal::new();
    let policy = policy(QuestionFamily::SingleChoice);
    let release = tokio::sync::Notify::new();
    let calls = Cell::new(0);
    let mut first = Box::pin(annotation_demand::scope(
        &policy,
        super::super::run(input, &signal, |_| {
            calls.set(calls.get() + 1);
            let release = &release;
            async move {
                release.notified().await;
                Ok(reply())
            }
        }),
    ));
    std::future::poll_fn(|context| {
        assert!(first.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    let mut second = Box::pin(annotation_demand::scope(
        &policy,
        super::super::run(input, &signal, |_| async {
            panic!("the second demand must join the exact annotation producer")
        }),
    ));
    std::future::poll_fn(|context| {
        assert!(second.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    release.notify_one();
    let (first, second) = tokio::join!(first, second);
    first.unwrap();
    second.unwrap();
    assert_eq!(calls.get(), 1);
    assert!(load(input).deferred_annotations().is_empty());
}

#[tokio::test]
async fn document_annotation_publication_reconciles_new_siblings_but_not_changed_targets() {
    for changed_slot in [0, 1] {
        let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        let contract = hash("retained annotation proposal versus newer receipt");
        let input = input(&database, &sources, &catalog, &contract);
        let mut newer = retain(input);
        newer.accepted[changed_slot].content.title = "Concurrent update".into();
        let calls = Cell::new(0);
        let result = demanded_run(input, &policy(QuestionFamily::SingleChoice), |_| {
            calls.set(calls.get() + 1);
            save(input, &newer);
            async { Ok(reply()) }
        })
        .await;
        assert_eq!(calls.get(), 1);
        let saved = load(input);
        assert_eq!(
            saved.accepted[changed_slot].content.title,
            "Concurrent update"
        );
        if changed_slot == 0 {
            assert_eq!(result.unwrap_err().code, repository::stale_source().code);
            assert_eq!(saved.deferred_annotations().len(), 1);
        } else {
            result.unwrap();
            assert!(saved.deferred_annotations().is_empty());
        }
    }
}

#[test]
fn document_annotation_resolution_coalesces_only_exact_duplicate_representations() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("annotation duplicate convergence");
    let before = retain(input(&database, &sources, &catalog, &contract));
    let patches = replacements(&before, &[0], reply());
    let repaired = apply(&before, &before, &patches).unwrap();
    let mut latest = before.clone();
    latest.accepted.insert(0, repaired.accepted[0].clone());
    let result = apply(&latest, &before, &patches).unwrap();
    assert_eq!(result.accepted.len(), before.accepted.len());
    assert!(result.deferred_annotations().is_empty());
    let mut different = repaired.accepted[0].clone();
    different
        .content
        .qualifications
        .push("Additional condition.".into());
    latest.accepted.push(different);
    let result = apply(&latest, &before, &patches).unwrap();
    assert_eq!(result.accepted.len(), before.accepted.len() + 1);
}

#[tokio::test]
async fn document_annotation_ordering_repairs_only_the_process_graph() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("ordering support repairs its process graph");
    let input = input(&database, &sources, &catalog, &contract);
    let mut before = retain(input);
    let content = &mut before.accepted[0].content;
    content.statement = "Prepare, then heat.".into();
    content.kind = KnowledgeKind::Process;
    content.qualifications = vec!["Only after preparation.".into()];
    let fixed = json!({"kind": "process", "steps": [
        {"text": "Prepare", "occurrence": 0}, {"text": "heat", "occurrence": 0}
    ], "dependencies": [{"before": 0, "after": 1}]});
    let mut cyclic = fixed.clone();
    cyclic["dependencies"] = json!([{"before": 0, "after": 1}, {"before": 1, "after": 0}]);
    content.details =
        Some(TypedPayload::admit_or_defer(cyclic, &content.statement, content.kind).unwrap());
    save(input, &before);
    demanded_run(input, &policy(QuestionFamily::Ordering), |_| {
        let value = json!({"annotationRepairs": [{"slot": 0, "details": fixed}]});
        async move { Ok(value) }
    })
    .await
    .unwrap();
    let saved = load(input);
    let content = &saved.accepted[0].content;
    assert_eq!(content.statement, "Prepare, then heat.");
    assert_eq!(content.qualifications, ["Only after preparation."]);
    assert_eq!(
        content
            .details
            .as_ref()
            .unwrap()
            .prompt(&content.statement)
            .unwrap()["dependencies"],
        json!([{"before": 0, "after": 1}])
    );
}

#[tokio::test]
async fn document_annotation_failed_local_save_does_not_retry_the_model_or_claim_publication() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("annotation local transaction failure");
    let input = input(&database, &sources, &catalog, &contract);
    let before = retain(input);
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
    let calls = Cell::new(0);
    let result = demanded_run(input, &policy(QuestionFamily::SingleChoice), |_| {
        calls.set(calls.get() + 1);
        async { Ok(reply()) }
    })
    .await;
    assert!(result.is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(
        serde_json::to_value(load(input)).unwrap(),
        serde_json::to_value(before).unwrap()
    );
}

#[tokio::test]
async fn document_annotation_identical_no_gain_batches_stop_without_discarding_pending_debt() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("annotation no gain strategy");
    let input = input(&database, &sources, &catalog, &contract);
    let mut before = retain(input);
    for index in 1..=MAX_TARGETS * 2 {
        let mut unit = before.accepted[0].clone();
        unit.content.title = format!("Independent unresolved target {index}");
        before.accepted.push(unit);
    }
    save(input, &before);
    let calls = Cell::new(0);
    demanded_run(input, &policy(QuestionFamily::SingleChoice), |_| {
        calls.set(calls.get() + 1);
        async { Ok(json!({"annotationRepairs": []})) }
    })
    .await
    .unwrap();
    assert_eq!(calls.get(), 2);
    let saved = load(input);
    assert!(!saved.is_complete());
    assert_eq!(
        serde_json::to_value(saved).unwrap(),
        serde_json::to_value(before).unwrap()
    );
}

#[tokio::test]
async fn document_annotation_reconciled_revision_keeps_later_batch_outcomes_observable() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("continue after local publication reconciliation");
    let mut input = input(&database, &sources, &catalog, &contract);
    let mut before = retain(input);
    for index in 1..=MAX_TARGETS {
        let mut unit = before.accepted[0].clone();
        unit.content.title = format!("Independent target {index}");
        before.accepted.push(unit);
    }
    save(input, &before);
    let operation = Operation::begin(&database, None, None, None, "annotation_test").unwrap();
    let context = operation.context();
    input.diagnostics = Some(&context);
    let calls = Cell::new(0);
    demanded_run(input, &policy(QuestionFamily::SingleChoice), |call| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            let mut newer = before.clone();
            newer.accepted[1].content.title = "Concurrent sibling".into();
            save(input, &newer);
        }
        let mut attempt = context
            .start_attempt(
                "knowledge_extraction",
                &json!({"model": "fixture", "messages": []}),
                Some(&contract),
                "fixture",
            )
            .unwrap();
        attempt.response(&json!({"usage": {"prompt_tokens": 10, "completion_tokens": 5}}));
        let slots = call.schema["properties"]["annotationRepairs"]["items"]["properties"]["slot"]
            ["enum"]
            .as_array()
            .unwrap();
        let value = json!({"annotationRepairs": slots.iter().map(|slot| {
            json!({"slot": slot, "details": details()})
        }).collect::<Vec<_>>()});
        async move { Ok(value) }
    })
    .await
    .unwrap();
    assert_eq!(calls.get(), 2);
    let saved = load(input);
    assert!(saved.deferred_annotations().is_empty());
    assert_eq!(saved.accepted[1].content.title, "Concurrent sibling");
    database
        .with_connection(|connection| {
            let payload: String = connection
                .query_row(
                    "SELECT payload_bytes_json FROM generation_requests
                     WHERE operation_id=?1 ORDER BY attempt DESC LIMIT 1",
                    [&context.operation_id],
                    |row| row.get(0),
                )
                .map_err(CommandError::database)?;
            let payload: Value = serde_json::from_str(&payload).unwrap();
            let checkpoint = &payload["nativeCheckpoint"];
            assert_eq!(checkpoint["before"]["deferredAnnotations"]["count"], 1);
            assert_eq!(checkpoint["after"]["deferredAnnotations"]["count"], 0);
            assert_eq!(checkpoint["gain"]["deferredAnnotationsDelta"], -1);
            Ok(())
        })
        .unwrap();
}
