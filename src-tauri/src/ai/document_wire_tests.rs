//! Regression across actual request lowering, provider response decoding,
//! native evidence admission and durable publication. No live account is used.
use super::{
    catalog::{AuthMode, ProviderKind},
    profiles::ProfileSnapshot,
    providers,
};
use crate::knowledge::{
    analysis_store, authoring::tests::fixture, contracts::hash, evidence::EvidenceCatalog,
    extraction,
};
use serde_json::{json, Value};

#[test]
fn all_provider_dialects_deliver_the_semantic_contract_and_preserve_native_evidence() {
    for kind in [
        ProviderKind::Openrouter,
        ProviderKind::Deepseek,
        ProviderKind::LlamaCpp,
    ] {
        let (database, source, _, _) = fixture();
        let sources = [source];
        let catalog = EvidenceCatalog::build(&sources, false).unwrap();
        let schema = extraction::response_schema_for_test(&catalog, &sources);
        let profile = ProfileSnapshot {
            id: "a".repeat(32),
            provider_kind: kind,
            base_url: match kind {
                ProviderKind::LlamaCpp => "http://127.0.0.1:8080/",
                ProviderKind::Deepseek => "https://api.deepseek.com/",
                _ => "https://openrouter.ai/api/v1/",
            }
            .into(),
            model_id: "test-model".into(),
            reasoning_effort: "none".into(),
            auth_mode: AuthMode::None,
            enabled: true,
            allow_insecure_http: false,
            require_zero_data_retention: false,
            request_timeout_ms: 60000,
            credential_epoch: 0,
            revision: 1,
        };
        let request = providers::prepare_document_request(
            &profile,
            &extraction::extraction_prompt(),
            Value::String(catalog.prompt(&sources).to_string()),
            &schema,
            6000,
            "none",
        )
        .unwrap();
        let system = request.body["messages"][0]["content"].as_str().unwrap();
        assert!(system.contains("noLearningSourceIds"));
        assert!(system.contains("evidenceIds"));
        assert!(system.contains("<ostudy-output-schema>"));
        let output=json!({"units":[{"title":"Triangle","kind":"fact","statement":"A triangle has three sides.",
            "qualifications":[],"topic":"Geometry","importance":"core","evidenceIds":["E0"]}],"noLearningSourceIds":null}).to_string();
        let message = if kind == ProviderKind::Deepseek {
            json!({"tool_calls":[{"function":{"name":request.body["tools"][0]["function"]["name"],"arguments":output}}]})
        } else {
            json!({"content":output})
        };
        let response = json!({"choices":[{"message":message,"finish_reason":"stop"}]});
        let value = providers::decode_document_output(&request, &response).unwrap();
        let report = extraction::decode::initial(value, &catalog, &sources).unwrap();
        assert!(report.is_complete());
        let contract = hash("wire-round-trip");
        let mut checkpoint =
            analysis_store::Checkpoint::open(&database, &sources, &contract).unwrap();
        let published = checkpoint.save(&report).unwrap();
        assert_eq!(
            published[&sources[0].id][0].content.supporting_excerpt,
            sources[0].text
        );
    }
}

mod streaming;
