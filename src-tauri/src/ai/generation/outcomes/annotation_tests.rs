use super::*;
use crate::{
    ai::generation::{self, Operation},
    database::Database,
    knowledge::{
        analysis_store::Checkpoint, contracts::hash, evidence::EvidenceCatalog, extraction::decode,
    },
};

fn snapshot(deferred: Option<u32>) -> Snapshot {
    Snapshot {
        accepted_units: 2,
        unresolved_regions: Some(0),
        deferred_annotations: deferred.map(|count| DeferredAnnotations {
            count,
            rules: if count == 0 {
                BTreeMap::new()
            } else {
                BTreeMap::from([("knowledge_annotation_malformed".to_string(), count)])
            },
        }),
        ..Snapshot::default()
    }
}

#[test]
fn document_annotation_outcomes_measure_resolution_without_new_core_units() {
    let before = snapshot(Some(1));
    let after = snapshot(Some(0));
    let gain = after.gain(&before);
    assert_eq!(gain["acceptedUnitsDelta"], 0);
    assert_eq!(gain["deferredAnnotationsDelta"], -1);
    assert_eq!(gain["hasNativeProgress"], true);
    assert_eq!(gain["hasNativeRegression"], false);
    let regression = before.gain(&after);
    assert_eq!(regression["deferredAnnotationsDelta"], 1);
    assert_eq!(regression["hasNativeRegression"], true);
    let unchanged = before.gain(&before);
    assert_eq!(unchanged["hasNativeProgress"], false);
    assert_eq!(unchanged["hasNativeRegression"], false);
    let mut mixed = before.clone();
    mixed.accepted_units += 1;
    let gain = mixed.gain(&after);
    assert_eq!(gain["hasNativeProgress"], true);
    assert_eq!(gain["hasNativeRegression"], true);
}

#[test]
fn document_annotation_outcomes_do_not_invent_a_historical_baseline() {
    let unknown = snapshot(None);
    let measured = snapshot(Some(1));
    for gain in [measured.gain(&unknown), unknown.gain(&measured)] {
        assert!(gain["deferredAnnotationsDelta"].is_null());
        assert_eq!(gain["hasNativeProgress"], false);
        assert_eq!(gain["hasNativeRegression"], false);
    }
    assert!(serde_json::to_value(&unknown)
        .unwrap()
        .get("deferredAnnotations")
        .is_none());
    assert_eq!(
        serde_json::to_value(snapshot(Some(0))).unwrap()["deferredAnnotations"]["count"],
        0
    );
}

fn ledger(database: &Database) -> Value {
    database
        .with_connection(|connection| {
            let (status, tokens, encoded): (String, i64, String) = connection
                .query_row(
                    "SELECT status,total_tokens,payload_bytes_json FROM generation_requests",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .map_err(CommandError::database)?;
            assert_eq!(status, "response");
            assert_eq!(tokens, 12);
            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM generation_requests", [], |row| {
                    row.get(0)
                })
                .map_err(CommandError::database)?;
            assert_eq!(count, 1);
            Ok(serde_json::from_str(&encoded).unwrap())
        })
        .unwrap()
}

async fn committed_observation(fail_save: bool) {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let mut report = decode::initial(
        json!({"units":[{"title":"Sides","kind":"fact",
            "statement":"A triangle has three sides.","qualifications":[],
            "topic":"Geometry","importance":"core","evidenceIds":["E0"],
            "details":{"privateDraft":"PRIVATE ANNOTATION DRAFT"}}]}),
        &catalog,
        &sources,
    )
    .unwrap();
    let contract = hash("committed deferred annotation diagnostic");
    let key = crate::knowledge::analysis_store::window_id(&sources, &contract).unwrap();
    let operation = Operation::begin(&database, None, None, None, "annotation_fixture").unwrap();
    let context = operation.context();
    let mut task = Task::new(Some(&context), "knowledge_extraction", Some(&key));
    if fail_save {
        report.accepted[0].content.statement.push_str(" changed");
    }
    let result: Result<(), CommandError> = task
        .scope(generation::scoped_request(
            Some(context),
            "knowledge_extraction",
            Some(&key),
            "fixture",
            async {
                let mut attempt = generation::start_current_attempt(
                    &json!({"model":"fixture","max_tokens":6000}),
                )
                .unwrap();
                attempt.response(&json!({"usage":{
                    "prompt_tokens":10,"completion_tokens":2,"total_tokens":12}}));
                Checkpoint::open(&database, &sources, &contract)?.save(&report)?;
                Ok(())
            },
        ))
        .await;
    task.finish(&result);
    assert_eq!(result.is_err(), fail_save);
    let record = ledger(&database);
    assert!(!record.to_string().contains("PRIVATE"));
    assert!(!record.to_string().contains("A triangle has three sides."));
    assert!(record.get("nativeRecoveryDecisions").is_none());
    if fail_save {
        assert!(record.get("nativeCheckpoint").is_none());
        assert_eq!(record["nativeTask"]["state"], "failed");
    } else {
        assert_eq!(record["nativeTask"]["state"], "returned");
        let saved = &record["nativeCheckpoint"]["after"];
        assert_eq!(saved["acceptedUnits"], 1);
        assert_eq!(saved["rejectedProposals"], 0);
        assert_eq!(saved["rejectionRules"], json!({}));
        assert_eq!(saved["protocolComplete"], false);
        assert_eq!(saved["deferredAnnotations"]["count"], 1);
        assert_eq!(
            saved["deferredAnnotations"]["rules"],
            json!({"knowledge_annotation_malformed":1})
        );
        assert_eq!(record["nativeTask"]["lastCheckpoint"], *saved);
        let restored = Checkpoint::open(&database, &sources, &contract)
            .unwrap()
            .load()
            .unwrap()
            .unwrap();
        assert!(!restored.requires_paid_repair(&sources));
        assert!(!restored.is_complete());
        assert_eq!(ledger(&database), record);
    }
}

#[tokio::test]
async fn document_annotation_outcomes_retain_committed_debt_not_rejected_core() {
    committed_observation(false).await;
}

#[tokio::test]
async fn document_annotation_outcomes_failed_save_never_claims_committed_debt() {
    committed_observation(true).await;
}
