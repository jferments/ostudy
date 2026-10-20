use std::{cell::RefCell, collections::VecDeque};

use serde_json::json;

use super::*;
use crate::knowledge::{authoring::tests::fixture, contracts::EvidenceImage};

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

#[test]
fn dense_windows_use_larger_first_pass_only_when_model_context_can_support_it() {
    let (_, mut first, _, _) = fixture();
    first.text = "Dense cardiovascular evidence with qualifications. ".repeat(140);
    let mut second = first.clone();
    second.id = "d".repeat(32);
    second.text = "Additional dense pharmacology evidence with conditions. ".repeat(140);
    let sources = [first, second];

    assert_eq!(
        initial_extraction_completion_tokens(Some(128_000), &sources),
        DENSE_EXTRACTION_COMPLETION_TOKENS
    );
    assert_eq!(
        initial_extraction_completion_tokens(Some(32_000), &sources),
        EXTRACTION_COMPLETION_TOKENS
    );
    assert_eq!(
        initial_extraction_completion_tokens(None, &sources),
        EXTRACTION_COMPLETION_TOKENS
    );
    assert_eq!(
        extraction_recovery_completion_tokens(DENSE_EXTRACTION_COMPLETION_TOKENS),
        MAX_EXTRACTION_RECOVERY_TOKENS
    );
}

#[tokio::test]
async fn truncated_window_retries_with_real_output_headroom_instead_of_contract_replay() {
    let (db, source, _, _) = fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("truncation-headroom-test");
    let signal = CancellationSignal::new();
    let responses = RefCell::new(VecDeque::from([
        Err(CommandError::new(
            "ai_document_output_truncated",
            "completion limit",
        )),
        Ok(json!({"units":[proposal("E0")],"noLearningSourceIds":[]})),
    ]));
    let budgets = RefCell::new(Vec::new());
    let prompts = RefCell::new(Vec::new());
    let result = run_window(&db, &sources, &contract, &catalog, &signal, false, |call| {
        budgets.borrow_mut().push(call.visible_completion_tokens);
        prompts.borrow_mut().push(call.user);
        std::future::ready(responses.borrow_mut().pop_front().unwrap())
    })
    .await
    .unwrap();

    assert_eq!(result[&sources[0].id].len(), 1);
    assert_eq!(
        *budgets.borrow(),
        vec![
            EXTRACTION_COMPLETION_TOKENS,
            EXTRACTION_COMPLETION_TOKENS * 2
        ]
    );
    assert!(prompts.borrow()[1].contains("completion ceiling"));
}

#[test]
fn targeted_repair_keeps_original_aliases_and_excludes_unrelated_evidence() {
    let (_, mut source, _, _) = fixture();
    source.text = (0..24)
        .map(|index| {
            format!(
                "Distinct repair region {index}: physiology marker {index} {}. ",
                "context ".repeat(20)
            )
        })
        .collect::<String>();
    source.refresh_identity().unwrap();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    assert!(catalog.entries.len() > 8);

    let target = catalog.entries.len() / 2;
    let alias = EvidenceCatalog::alias(target);
    let report = decode::AnalysisReport {
        rejected: vec![decode::RejectedUnit {
            slot: 7,
            proposal: proposal(&alias),
            code: "invalid_unit_content".to_string(),
        }],
        ..decode::AnalysisReport::default()
    };
    let scope = repair_scope(&report, &catalog, &sources);
    assert!(scope.positions.contains(&target));
    assert!(scope.positions.len() <= 3);
    assert!(!scope.positions.contains(&0));

    let (user, schema) = build_repair_request(&report, &catalog, &sources);
    assert!(user.contains(&format!("\"id\":\"{alias}\"")));
    assert!(!user.contains("\"id\":\"E0\""));
    let allowed = schema
        .pointer(
            "/properties/repairs/items/properties/unit/anyOf/0/properties/evidenceIds/items/enum",
        )
        .and_then(Value::as_array)
        .unwrap();
    assert!(allowed.iter().any(|value| value == &json!(alias)));
    assert!(!allowed.iter().any(|value| value == &json!("E0")));
    assert!(scope.positions.len() < catalog.entries.len());
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

#[path = "reuse_tests.rs"]
mod reuse_tests;

#[test]
fn complexity_aware_batching_splits_fragmented_text_without_losing_sources() {
    let (_, source, _, _) = fixture();
    let mut sources = Vec::new();
    for index in 0..4 {
        let mut source = source.clone();
        source.segment_key = format!("complexity-{index}");
        source.text = "concept ".repeat(375);
        sources.push(source);
    }
    assert_eq!(
        sources
            .iter()
            .map(|source| source.text.chars().count())
            .sum::<usize>(),
        EXTRACTION_BATCH_CHARS
    );
    let ranges = batch_ranges(&sources);
    assert_eq!(ranges, vec![0..3, 3..4]);
    assert_eq!(
        ranges.iter().map(|range| range.len()).sum::<usize>(),
        sources.len()
    );
}

#[test]
fn neighbouring_sources_share_a_window_and_its_page_images() {
    let (_, source, _, _) = fixture();
    let on_pages = |segment: &str, assets: &[&str]| {
        let mut source = source.clone();
        source.segment_key = segment.to_string();
        source.images = assets
            .iter()
            .map(|asset| EvidenceImage::DocumentAsset {
                document_id: source.owner_id.clone(),
                asset_id: asset.to_string(),
            })
            .collect();
        source.refresh_identity().unwrap();
        source
    };
    let sources = [
        on_pages("table-head", &["unit-3"]),
        on_pages("table-rows", &["unit-3", "unit-4"]),
        on_pages("page-four", &["unit-4"]),
        on_pages("page-five", &["unit-5"]),
        on_pages("prose", &[]),
    ];
    assert_eq!(batch_ranges(&sources), vec![0..3, 3..5]);
    assert_eq!(window_page_images(&sources[..3]).len(), 2);
    assert_eq!(window_page_images(&sources).len(), 3);
}

#[test]
fn picture_only_sources_wait_for_vision_without_failing_their_window() {
    let (_, source, _, _) = fixture();
    let picture = |segment: &str, text: &str| {
        let mut source = source.clone();
        source.segment_key = segment.to_string();
        source.text = text.to_string();
        source.images = vec![EvidenceImage::DocumentAsset {
            document_id: source.owner_id.clone(),
            asset_id: "unit-1".to_string(),
        }];
        source.refresh_identity().unwrap();
        source
    };
    let cover = picture("cover", "[image]");
    let figure = picture("figure", "Figure 2 shows the pressure curve.");
    let kept =
        defer_picture_only_sources(vec![cover.clone(), source.clone(), figure.clone()]).unwrap();
    assert_eq!(
        kept.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        vec![source.id.as_str(), figure.id.as_str()]
    );
    let error = defer_picture_only_sources(vec![cover]).unwrap_err();
    assert_eq!(error.code, IMAGES_REQUIRE_VISION);
}
