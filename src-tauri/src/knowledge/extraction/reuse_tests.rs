//! Request-counted reuse baselines over real admission and SQLite receipts.
use std::cell::Cell;

use super::*;
use crate::knowledge::authoring::KnowledgeCatalog;

#[tokio::test]
async fn completed_interpretation_is_read_only_across_consumer_contracts() {
    let (db, source, _, _) = fixture();
    let sources = [source];
    let evidence = EvidenceCatalog::build(&sources, false).unwrap();
    let producer = hash("captured interpretation producer");
    let signal = CancellationSignal::new();
    let calls = Cell::new(0);
    let first = run_window(&db, &sources, &producer, &evidence, &signal, false, |_| {
        calls.set(calls.get() + 1);
        std::future::ready(Ok(
            json!({"units":[proposal("E0")],"noLearningSourceIds":[]}),
        ))
    })
    .await
    .unwrap();
    assert_eq!(calls.get(), 1);
    let initial_units = serde_json::to_value(&first).unwrap();
    let changes = db.with_connection(|c| Ok(c.total_changes())).unwrap();
    for consumer in ["initial cards", "additional cards", "guide", "tutor"] {
        let compatible = analysis_store::compatible_contract(&db, &sources)
            .unwrap()
            .unwrap();
        assert_eq!(compatible, producer);
        assert_ne!(compatible, hash(consumer));
        let reused = run_window(
            &db,
            &sources,
            &compatible,
            &evidence,
            &signal,
            false,
            |_| {
                calls.set(calls.get() + 1);
                std::future::ready(Err(invalid("unexpected interpretation request")))
            },
        )
        .await
        .unwrap();
        assert_eq!(serde_json::to_value(&reused).unwrap(), initial_units);
        let catalog = KnowledgeCatalog {
            units: reused[&sources[0].id].clone(),
            document_core_units: Default::default(),
            preparation: Default::default(),
        };
        let prompt: Value = serde_json::from_str(&catalog.prompt().unwrap()).unwrap();
        assert_eq!(prompt["units"].as_array().unwrap().len(), 1);
        assert_eq!(prompt["evidence"].as_array().unwrap().len(), 1);
        assert_eq!(calls.get(), 1);
        assert_eq!(
            db.with_connection(|c| Ok(c.total_changes())).unwrap(),
            changes
        );
    }
}

#[tokio::test]
async fn partial_read_does_not_retry_and_explicit_repair_retains_accepted_identity() {
    let (db, source, _, _) = fixture();
    let sources = [source];
    let evidence = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("partial request accounting");
    let signal = CancellationSignal::new();
    let calls = Cell::new(0);
    let first = run_window(&db, &sources, &contract, &evidence, &signal, false, |_| {
        calls.set(calls.get() + 1);
        std::future::ready(Ok(if calls.get() == 1 {
            json!({"units":[proposal("E0"),proposal("not-supplied")],"noLearningSourceIds":[]})
        } else {
            json!({"repairs":[{"slot":1,"unit":null}],"additionalUnits":[],"noLearningSourceIds":[]})
        }))
    })
    .await
    .unwrap();
    assert_eq!(calls.get(), 2);
    let accepted_id = first[&sources[0].id][0].id.clone();
    let changes = db.with_connection(|c| Ok(c.total_changes())).unwrap();
    run_window(&db, &sources, &contract, &evidence, &signal, false, |_| {
        calls.set(calls.get() + 1);
        std::future::ready(Err(invalid("a partial read must not authorize repair")))
    })
    .await
    .unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(
        db.with_connection(|c| Ok(c.total_changes())).unwrap(),
        changes
    );
    let repaired = run_window(&db, &sources, &contract, &evidence, &signal, true, |call| {
        calls.set(calls.get() + 1);
        let mut fixed = proposal("E0");
        fixed["title"] = json!("Three-sided polygon");
        assert!(call.user.contains("rejectedProposals"));
        std::future::ready(Ok(json!({
            "repairs":[{"slot":1,"unit":fixed}],
            "additionalUnits":[],"noLearningSourceIds":[]
        })))
    })
    .await
    .unwrap();
    assert_eq!(calls.get(), 3);
    assert_eq!(repaired[&sources[0].id].len(), 2);
    assert!(repaired[&sources[0].id]
        .iter()
        .any(|unit| unit.id == accepted_id));
    assert!(analysis_store::load(&db, &sources, &contract)
        .unwrap()
        .unwrap()
        .is_complete());
}
