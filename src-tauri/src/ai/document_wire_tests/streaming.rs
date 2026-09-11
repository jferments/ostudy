//! Real SSE -> provider decoder -> semantic admission -> SQLite regression.
//! Parallel tool invocations are records, never concatenated JSON fragments.
use serde_json::{json, Value};

use crate::ai::gateway::SseChatAccumulator;
use crate::{
    ai::{
        catalog::{AuthMode, ProviderKind},
        profiles::ProfileSnapshot,
        providers,
    },
    knowledge::{
        analysis_store, authoring::tests::fixture, contracts::hash, evidence::EvidenceCatalog,
        extraction,
    },
};

#[test]
fn interleaved_tool_results_preserve_all_knowledge_and_native_provenance() {
    let (database, first, _, _) = fixture();
    let mut second = first.clone();
    second.segment_key = "1:0:0".into();
    second.text = "Triangles have three angles.".into();
    second.refresh_identity().unwrap();
    database
        .with_connection(|c| crate::knowledge::repository::publish_source(c, &second, "1", 2))
        .unwrap();
    let sources = [first, second];
    let catalog = EvidenceCatalog::build(&sources, false).unwrap();
    let profile = ProfileSnapshot {
        id: "a".repeat(32),
        provider_kind: ProviderKind::Deepseek,
        base_url: "https://api.deepseek.com/".into(),
        model_id: "fixture".into(),
        reasoning_effort: "none".into(),
        auth_mode: AuthMode::None,
        enabled: true,
        allow_insecure_http: false,
        require_zero_data_retention: false,
        request_timeout_ms: 1000,
        credential_epoch: 0,
        revision: 1,
    };
    let request = providers::prepare_document_request(
        &profile,
        &extraction::extraction_prompt(),
        Value::String(catalog.prompt(&sources).to_string()),
        &extraction::response_schema_for_test(&catalog, &sources),
        6000,
        "none",
    )
    .unwrap();
    let name = request.body["tools"][0]["function"]["name"].clone();
    let outputs = [
        ("Sides", "A triangle has three sides.", "E0"),
        ("Angles", "A triangle has three angles.", "E1"),
    ]
    .map(|(title, statement, evidence)| {
        json!({"units":[{"title":title,"kind":"fact","statement":statement,
            "qualifications":[],"topic":"Geometry","importance":"core","evidenceIds":[evidence]}],
            "noLearningSourceIds":[]})
        .to_string()
    });
    let mut stream = SseChatAccumulator::new(32000, 128000);
    // Split both documents, interleave the tails, and confirm final snapshots
    // cannot duplicate the accumulated argument strings.
    for calls in [
        json!([{"index":0,"id":"left","function":{"name":name,"arguments":&outputs[0][..10]}},
            {"index":1,"id":"right","function":{"name":name,"arguments":&outputs[1][..10]}}]),
        json!([{"index":1,"function":{"arguments":&outputs[1][10..]}}]),
        json!([{"index":0,"function":{"arguments":&outputs[0][10..]}}]),
    ] {
        let frame = format!(
            "data: {}\n\n",
            json!({"choices":[{"delta":{"tool_calls":calls}}]})
        );
        for chunk in frame.as_bytes().chunks(7) {
            stream.push_bytes(chunk).unwrap();
        }
    }
    let snapshot = format!(
        "data: {}\ndata: [DONE]\n",
        json!({"choices":[{"finish_reason":"tool_calls", "message":{"tool_calls":[
            {"index":0,"id":"left","function":{"name":name,"arguments":outputs[0]}},
            {"index":1,"id":"right","function":{"name":name,"arguments":outputs[1]}}
        ]}}]})
    );
    stream.push_bytes(snapshot.as_bytes()).unwrap();
    stream.finish().unwrap();
    assert_eq!(
        stream.tool_arguments_len(),
        outputs.iter().map(String::len).sum::<usize>()
    );
    let response = json!({"choices":[{"message":{"tool_calls":stream.tool_calls()}}]});
    let decoded = providers::decode_document_output(&request, &response).unwrap();
    assert_eq!(decoded.as_array().unwrap().len(), 2);
    let report = extraction::decode::initial(decoded, &catalog, &sources).unwrap();
    assert_eq!(report.accepted.len(), 2);
    assert!(report.is_complete());
    let contract = hash("indexed-tools-wire-regression");
    let mut checkpoint = analysis_store::Checkpoint::open(&database, &sources, &contract).unwrap();
    let saved = checkpoint.save(&report).unwrap();
    for source in &sources {
        assert_eq!(saved[&source.id].len(), 1);
        let unit = &saved[&source.id][0];
        assert_eq!(unit.source_hash, source.content_hash);
        assert_eq!(unit.content.supporting_excerpt, source.text);
        unit.content.grounding.as_ref().unwrap().evidence[0]
            .validate(source)
            .unwrap();
    }
    assert!(checkpoint.load().unwrap().unwrap().is_complete());
}
