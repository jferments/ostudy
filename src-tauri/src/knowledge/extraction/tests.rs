use std::{cell::RefCell, collections::VecDeque};

use serde_json::json;

use super::*;
use crate::knowledge::authoring::tests::fixture;

fn proposal(evidence: &str) -> Value {
    json!({"title":"Triangle","kind":"definition","statement":"A triangle has three sides.",
        "qualifications":[],"topic":"Geometry","importance":"core","evidenceIds":[evidence]})
}

#[tokio::test]
async fn mixed_window_preserves_good_sibling_then_resumes_only_rejected_slot() {
    let (db, source, _, _) = fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("native-analysis-test");
    let signal = CancellationSignal::new();
    let responses = RefCell::new(VecDeque::from([
        Ok(json!({"units":[proposal("E0"),proposal("invented")],"noLearningSourceIds":[]})),
        Ok(
            json!({"repairs":[{"slot":1,"unit":null}],"additionalUnits":[],"noLearningSourceIds":[]}),
        ),
    ]));
    let calls = RefCell::new(Vec::new());
    let result = run_window(&db, &sources, &contract, &catalog, &signal, false, |call| {
        calls.borrow_mut().push(call.user);
        std::future::ready(responses.borrow_mut().pop_front().unwrap())
    })
    .await
    .unwrap();
    assert_eq!(result[&sources[0].id].len(), 1);
    assert_eq!(calls.borrow().len(), 2);
    assert!(calls.borrow()[1].contains("rejectedProposals"));
    assert!(!analysis_store::load(&db, &sources, &contract)
        .unwrap()
        .unwrap()
        .is_complete());
    let mut fixed = proposal("E0");
    fixed["title"] = json!("Three sides");
    let retried=run_window(&db,&sources,&contract,&catalog,&signal,true,|call| {
        assert!(call.user.contains("rejectedProposals"));
        std::future::ready(Ok(json!({"repairs":[{"slot":1,"unit":fixed.clone()}],"additionalUnits":[],"noLearningSourceIds":[]})))
    }).await.unwrap();
    assert_eq!(retried[&sources[0].id].len(), 2);
    assert!(analysis_store::load(&db, &sources, &contract)
        .unwrap()
        .unwrap()
        .is_complete());
    // A third invocation uses the durable receipt without provider traffic.
    run_window(&db, &sources, &contract, &catalog, &signal, false, |_| {
        std::future::ready(Err(invalid("completed analysis must not call a provider")))
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn transport_failure_during_repair_does_not_erase_admitted_knowledge() {
    let (db, source, _, _) = fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("transport-test");
    let responses = RefCell::new(VecDeque::from([
        Ok(json!({"units":[proposal("E0"),proposal("E999")],"noLearningSourceIds":[]})),
        Err(CommandError::new("ai_provider_unavailable", "unavailable")),
    ]));
    let error = run_window(
        &db,
        &sources,
        &contract,
        &catalog,
        &CancellationSignal::new(),
        false,
        |_| std::future::ready(responses.borrow_mut().pop_front().unwrap()),
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "ai_provider_unavailable");
    db.with_connection(|connection| {
        assert_eq!(repository::load_units(connection, &sources[0].id)?.len(), 1);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        analysis_store::load(&db, &sources, &contract)
            .unwrap()
            .unwrap()
            .rejected_count(),
        1
    );
}

#[test]
fn batch_boundaries_do_not_mix_unrelated_document_owners() {
    let (_, first, _, _) = fixture();
    let mut second = first.clone();
    second.owner_id = "a".repeat(32);
    second.document_id = Some(second.owner_id.clone());
    second.refresh_identity().unwrap();
    assert_eq!(batch_ranges(&[first, second]), vec![0..1, 1..2]);
}

#[test]
fn schema_selects_evidence_ids_instead_of_model_generated_quotations() {
    let (_, source, _, _) = fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let value = schema::initial(&catalog, &sources);
    assert!(value.to_string().contains("evidenceIds"));
    assert!(!value.to_string().contains("supportingExcerpt"));
    assert_eq!(
        value
            .pointer("/properties/units/items/properties/evidenceIds/items/enum")
            .unwrap(),
        &json!(["E0"])
    );
}

#[tokio::test]
async fn cancellation_during_repair_preserves_the_checkpoint_without_publishing_new_work() {
    let (db, source, _, _) = fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("cancel-test");
    let signal = CancellationSignal::new();
    let mut count = 0;
    let error = run_window(&db, &sources, &contract, &catalog, &signal, false, |_| {
        count += 1;
        if count == 1 {
            std::future::ready(Ok(
                json!({"units":[proposal("E0"),proposal("E999")],"noLearningSourceIds":[]}),
            ))
        } else {
            signal.cancel();
            std::future::ready(Ok(
                json!({"repairs":[],"additionalUnits":[],"noLearningSourceIds":[]}),
            ))
        }
    })
    .await
    .unwrap_err();
    assert_eq!(error.code, "ai_request_canceled");
    assert_eq!(
        analysis_store::load(&db, &sources, &contract)
            .unwrap()
            .unwrap()
            .accepted
            .len(),
        1
    );
}

#[tokio::test]
async fn source_mutation_during_provider_work_never_publishes_stale_evidence() {
    let (db, source, _, _) = fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("stale-request-test");
    let error = run_window(
        &db,
        &sources,
        &contract,
        &catalog,
        &CancellationSignal::new(),
        false,
        |_| {
            let mut changed = sources[0].clone();
            changed.text = "Changed source content".into();
            changed.refresh_identity().unwrap();
            db.with_connection(|connection| {
                repository::publish_source(connection, &changed, "0", 3)
            })
            .unwrap();
            std::future::ready(Ok(
                json!({"units":[proposal("E0")],"noLearningSourceIds":[]}),
            ))
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.code, "semantic_source_changed");
    db.with_connection(|connection| {
        let count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM knowledge_analysis_windows",
                [],
                |row| row.get(0),
            )
            .map_err(CommandError::database)?;
        assert_eq!(count, 0);
        Ok(())
    })
    .unwrap();
}

#[test]
fn native_evidence_and_display_metadata_have_separate_versioned_identities() {
    let (db, mut source, _, _) = fixture();
    let legacy_json = serde_json::to_value(&source).unwrap();
    assert!(legacy_json.get("identityVersion").is_none());
    let legacy_hash = source.content_hash.clone();
    source.title = "Display rename".into();
    source.refresh_identity().unwrap();
    assert_ne!(legacy_hash, source.content_hash);
    source.identity_version = 2;
    source.refresh_identity().unwrap();
    let evidence_hash = source.content_hash.clone();
    db.with_connection(|connection| repository::publish_source(connection, &source, "0", 4))
        .unwrap();
    source.title = "Another display name".into();
    source.refresh_identity().unwrap();
    assert_eq!(evidence_hash, source.content_hash);
    db.with_connection(|connection| {
        connection
            .execute(
                "UPDATE documents SET title='Automatic title' WHERE id=?1",
                [&source.owner_id],
            )
            .map_err(CommandError::database)?;
        assert!(repository::source_is_current(connection, &source)?);
        repository::publish_source(connection, &source, "0", 5)?;
        assert_eq!(
            repository::load_source(connection, &source.id)?
                .unwrap()
                .title,
            source.title
        );
        Ok(())
    })
    .unwrap();
}

#[tokio::test]
async fn explicit_empty_disposition_reuses_without_inventing_knowledge() {
    let (db, source, _, _) = fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("empty-test");
    let result = run_window(
        &db,
        &sources,
        &contract,
        &catalog,
        &CancellationSignal::new(),
        false,
        |_| {
            std::future::ready(Ok(
                json!({"units":[],"noLearningSourceIds":[sources[0].id]}),
            ))
        },
    )
    .await
    .unwrap();
    assert!(result[&sources[0].id].is_empty());
    assert!(analysis_store::load(&db, &sources, &contract)
        .unwrap()
        .unwrap()
        .is_complete());
}
