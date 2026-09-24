//! Exercise production sharing, decoding, recovery and SQLite checkpoints.
use super::*;
use crate::ai::generation::{self, Operation};

fn unit(title: &str) -> Value {
    json!({"title":title,"kind":"fact","statement":"A triangle has three sides.",
        "qualifications":[],"topic":"Geometry","importance":"core","evidenceIds":["E0"]})
}

fn requests(database: &Database) -> Vec<(String, Value)> {
    database
        .with_connection(|c| {
            let mut query = c
                .prepare(
                    "SELECT id,payload_bytes_json FROM generation_requests ORDER BY attempt,id",
                )
                .map_err(CommandError::database)?;
            let rows = query
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .map_err(CommandError::database)?;
            rows.map(|row| {
                let (id, text) = row.map_err(CommandError::database)?;
                Ok((id, serde_json::from_str(&text).unwrap()))
            })
            .collect()
        })
        .unwrap()
}

async fn scenario(improve: bool) {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("outcome production regression");
    let authorization = hash("fixture authorization");
    let operation = Operation::begin(&database, None, None, None, "extraction_fixture").unwrap();
    let context = operation.context();
    let key = analysis_store::window_id(&sources, &contract).unwrap();
    let input = Window {
        database: &database,
        sources: &sources,
        contract: &contract,
        catalog: &catalog,
        authorization: &authorization,
        retry_partial: false,
        diagnostics: Some(&context),
        logical_key: Some(&key),
    };
    let calls = std::cell::Cell::new(0);
    let result = run(input,&CancellationSignal::new(),|call| {
        let n = calls.get();
        calls.set(n+1);
        let value = if n==0 {
            let mut bad = unit("Other property");
            bad["title"] = json!("");
            json!({"units":[unit("Sides"),bad],"noLearningSourceIds":[]})
        } else {
            assert!(call.user.contains("invalid_unit_content"));
            json!({"repairs":[{"slot":1,"unit":if improve { unit("Other property") } else { Value::Null }}],
                "additionalUnits":[],"noLearningSourceIds":[]})
        };
        let context = context.clone();
        let key = key.clone();
        async move {
            generation::scoped_request(Some(context),"knowledge_extraction",Some(&key),"fixture",async move {
                let mut attempt = generation::start_current_attempt(&json!({"model":"fixture","max_tokens":6000})).unwrap();
                attempt.response(&json!({"usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12},
                    "choices":[{"finish_reason":"stop","message":{"content":"PRIVATE_CONTENT"}}]}));
                Ok(value)
            }).await
        }
    }).await.unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(result[&sources[0].id].len(), if improve { 2 } else { 1 });
    let rows = requests(&database);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        rows[0].1["nativeCheckpoint"]["after"]["rejectionRules"]["invalid_unit_content"],
        1
    );
    assert_eq!(
        rows[1].1["nativeLineage"]["recovery"]["triggerRequestId"],
        rows[0].0
    );
    assert_eq!(
        rows[1].1["nativeCheckpoint"]["gain"]["hasNativeProgress"],
        improve
    );
    assert_eq!(
        rows[1].1["nativeCheckpoint"]["after"]["protocolComplete"],
        improve
    );
    assert_eq!(rows[1].1["nativeTask"]["state"], "returned");
    assert!(!serde_json::to_string(&rows)
        .unwrap()
        .contains("PRIVATE_CONTENT"));
    assert!(!serde_json::to_string(&rows)
        .unwrap()
        .contains(&sources[0].text));
    run(input, &CancellationSignal::new(), |_| async {
        panic!("reuse must not buy another request")
    })
    .await
    .unwrap();
    assert_eq!(requests(&database), rows);
}

#[tokio::test]
async fn document_outcomes_real_repair_commits_measured_progress_and_reuses() {
    scenario(true).await;
}

#[tokio::test]
async fn document_outcomes_real_no_gain_repair_remains_explicitly_partial() {
    scenario(false).await;
}

#[tokio::test]
async fn document_outcomes_real_failed_checkpoint_never_claims_committed_knowledge() {
    let (database, source, _, _) = crate::knowledge::authoring::tests::fixture();
    let sources = [source];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let contract = hash("failed outcome checkpoint");
    let authorization = hash("fixture authorization");
    let operation = Operation::begin(&database, None, None, None, "failure_fixture").unwrap();
    let context = operation.context();
    let key = analysis_store::window_id(&sources, &contract).unwrap();
    let input = Window {
        database: &database,
        sources: &sources,
        contract: &contract,
        catalog: &catalog,
        authorization: &authorization,
        retry_partial: false,
        diagnostics: Some(&context),
        logical_key: Some(&key),
    };
    let error=run(input,&CancellationSignal::new(),|_| {
        let context=context.clone();let key=key.clone();let database=&database;let sources=&sources;
        async move {
            generation::scoped_request(Some(context),"knowledge_extraction",Some(&key),"fixture",async move {
                let mut attempt=generation::start_current_attempt(&json!({"model":"fixture","max_tokens":6000})).unwrap();
                attempt.response(&json!({"usage":{"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}}));
                let mut changed=sources[0].clone();changed.text.push_str(" changed");changed.refresh_identity().unwrap();
                database.with_connection(|c|crate::knowledge::repository::publish_source(c,&changed,"0",3)).unwrap();
                Ok(json!({"units":[unit("Sides")],"noLearningSourceIds":[]}))
            }).await
        }
    }).await.unwrap_err();
    let rows = requests(&database);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].1.get("nativeCheckpoint").is_none());
    assert_eq!(rows[0].1["nativeTask"]["state"], "failed");
    assert_eq!(rows[0].1["nativeTask"]["errorCode"], error.code);
    database
        .with_connection(|c| {
            let count: i64 = c
                .query_row("SELECT COUNT(*) FROM knowledge_analysis_windows", [], |r| {
                    r.get(0)
                })
                .map_err(CommandError::database)?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
}
