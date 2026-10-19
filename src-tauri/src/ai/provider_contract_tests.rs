use std::{
    collections::VecDeque,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use serde_json::{json, Value};

use super::{
    catalog::{AuthMode, ProviderKind},
    gateway::AiGateway,
    profiles::ProfileSnapshot,
    providers,
};

struct ExpectedExchange {
    method: &'static str,
    path: &'static str,
    assert_body: fn(&str),
    status: u16,
    response_body: String,
    extra_headers: Vec<(&'static str, &'static str)>,
    /// Lowercase `name: value` request header lines that must be present
    /// (`!name` means the header must be absent).
    request_headers: Vec<&'static str>,
}

struct MockRequest {
    method: String,
    path: String,
    headers: String,
    body: String,
}

#[test]
fn llama_cpp_adapter_discovers_models_and_verifies_server_structured_output() {
    let echo = "llama-contract-echo";
    let (base_url, server) = spawn_mock_server(vec![
        ExpectedExchange {
            method: "GET",
            path: "/v1/models",
            assert_body: assert_empty_body,
            status: 200,
            response_body: json!({
                "object": "list",
                "data": [{ "id": "nursing-qwen", "owned_by": "local" }]
            })
            .to_string(),
            extra_headers: Vec::new(),
            request_headers: Vec::new(),
        },
        ExpectedExchange {
            method: "GET",
            path: "/props?model=nursing-qwen",
            assert_body: assert_empty_body,
            status: 200,
            response_body: json!({
                "chat_template": "{% for message in messages %}{{ message.content }}{% endfor %}",
                "modalities": ["text", "image"],
                "n_ctx": 16384
            })
            .to_string(),
            extra_headers: Vec::new(),
            request_headers: Vec::new(),
        },
        ExpectedExchange {
            method: "POST",
            path: "/v1/chat/completions",
            assert_body: assert_llama_text_completion_request,
            status: 200,
            response_body: completion_response("OSTUDY_OK"),
            extra_headers: Vec::new(),
            request_headers: Vec::new(),
        },
        ExpectedExchange {
            method: "POST",
            path: "/v1/chat/completions",
            assert_body: assert_llama_schema_request,
            status: 200,
            response_body: completion_response(&json!({ "ok": true, "echo": echo }).to_string()),
            extra_headers: Vec::new(),
            request_headers: Vec::new(),
        },
    ]);
    let profile = profile(
        ProviderKind::LlamaCpp,
        base_url,
        "nursing-qwen",
        AuthMode::None,
    );
    let gateway = AiGateway::new().expect("gateway should initialize");

    tauri::async_runtime::block_on(async {
        let models = providers::list_models(&gateway, &profile, None)
            .await
            .expect("models should load");
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "nursing-qwen");

        let properties = providers::load_properties(&gateway, &profile, None)
            .await
            .expect("properties should load");
        assert_eq!(properties.server_chat_template_present, Some(true));
        assert_eq!(properties.image_input_declared, Some(true));
        assert_eq!(properties.context_window_tokens, Some(16384));
        assert_eq!(
            properties
                .server_chat_template_hash
                .as_deref()
                .map(str::len),
            Some(64)
        );

        providers::run_text_probe(&gateway, &profile, None)
            .await
            .expect("text probe should pass");
        providers::run_structured_probe(&gateway, &profile, None, echo)
            .await
            .expect("schema probe should pass");
    });
    server.join().expect("mock server should finish");
}

#[test]
fn llama_cpp_optional_props_tolerates_non_json_metadata_response() {
    let (base_url, server) = spawn_mock_server(vec![ExpectedExchange {
        method: "GET",
        path: "/props?model=nursing-qwen",
        assert_body: assert_empty_body,
        status: 200,
        response_body: "optional metadata unavailable".to_string(),
        extra_headers: Vec::new(),
        request_headers: Vec::new(),
    }]);
    let profile = profile(
        ProviderKind::LlamaCpp,
        base_url,
        "nursing-qwen",
        AuthMode::None,
    );
    let gateway = AiGateway::new().expect("gateway should initialize");

    let properties = tauri::async_runtime::block_on(async {
        providers::load_properties(&gateway, &profile, None)
            .await
            .expect("non-JSON optional metadata must not fail provider connectivity")
    });
    assert_eq!(properties.server_chat_template_present, None);
    assert_eq!(properties.image_input_declared, None);
    assert_eq!(properties.context_window_tokens, None);
    server.join().expect("mock server should finish");
}

#[test]
fn deepseek_v4_adapter_disables_thinking_for_bounded_connectivity_probes() {
    let echo = "deepseek-contract-echo";
    let (base_url, server) = spawn_mock_server(vec![
        ExpectedExchange {
            method: "GET",
            path: "/models",
            assert_body: assert_empty_body,
            status: 200,
            response_body: json!({
                "object": "list",
                "data": [{ "id": "deepseek-v4-flash", "owned_by": "deepseek" }]
            })
            .to_string(),
            extra_headers: Vec::new(),
            request_headers: Vec::new(),
        },
        ExpectedExchange {
            method: "POST",
            path: "/chat/completions",
            assert_body: assert_deepseek_text_completion_request,
            status: 200,
            response_body: completion_response_with_finish("OSTUDY_OK", "length"),
            extra_headers: Vec::new(),
            request_headers: Vec::new(),
        },
        ExpectedExchange {
            method: "POST",
            path: "/chat/completions",
            assert_body: assert_deepseek_json_object_request,
            status: 200,
            response_body: completion_response(&json!({ "ok": true, "echo": echo }).to_string()),
            extra_headers: Vec::new(),
            request_headers: Vec::new(),
        },
    ]);
    let profile = profile(
        ProviderKind::Deepseek,
        base_url,
        "deepseek-v4-flash",
        AuthMode::Bearer,
    );
    let gateway = AiGateway::new().expect("gateway should initialize");

    tauri::async_runtime::block_on(async {
        let models = providers::list_models(&gateway, &profile, Some("sentinel-token"))
            .await
            .expect("models should load");
        assert_eq!(models[0].id, "deepseek-v4-flash");
        let text = providers::run_text_probe(&gateway, &profile, Some("sentinel-token"))
            .await
            .expect("valid final content should pass even when finish reason is length");
        assert_eq!(text.finish_reason.as_deref(), Some("length"));
        providers::run_structured_probe(&gateway, &profile, Some("sentinel-token"), echo)
            .await
            .expect("JSON object should pass local validation");
    });
    server.join().expect("mock server should finish");
}

#[test]
fn native_gateway_rejects_redirects_instead_of_forwarding_credentials() {
    let (base_url, server) = spawn_mock_server(vec![ExpectedExchange {
        method: "GET",
        path: "/redirect",
        assert_body: assert_empty_body,
        status: 302,
        response_body: String::new(),
        extra_headers: vec![("Location", "http://127.0.0.1:9/untrusted")],
        request_headers: Vec::new(),
    }]);
    let gateway = AiGateway::new().expect("gateway should initialize");
    let url = url::Url::parse(&format!("{base_url}redirect")).expect("URL should parse");

    let failure = tauri::async_runtime::block_on(async {
        gateway
            .get_json(url, Some("redirect-sentinel"), 5_000, 1024)
            .await
            .expect_err("redirect should be rejected")
    });
    assert_eq!(failure.safe.code, "ai_redirect_rejected");
    assert!(!failure.safe.message.contains("redirect-sentinel"));
    server.join().expect("mock server should finish");
}

#[test]
fn deepseek_structured_probe_rejects_valid_json_with_the_wrong_semantics() {
    let echo = "expected-echo";
    let (base_url, server) = spawn_mock_server(vec![ExpectedExchange {
        method: "POST",
        path: "/chat/completions",
        assert_body: assert_deepseek_json_object_request,
        status: 200,
        response_body: completion_response(
            &json!({ "ok": true, "echo": "wrong-echo" }).to_string(),
        ),
        extra_headers: Vec::new(),
        request_headers: Vec::new(),
    }]);
    let profile = profile(
        ProviderKind::Deepseek,
        base_url,
        "deepseek-v4-flash",
        AuthMode::Bearer,
    );
    let gateway = AiGateway::new().expect("gateway should initialize");

    let failure = tauri::async_runtime::block_on(async {
        providers::run_structured_probe(&gateway, &profile, Some("sentinel-token"), echo)
            .await
            .expect_err("wrong semantic echo should fail")
    });
    assert_eq!(failure.safe.code, "ai_structured_output_invalid");
    server.join().expect("mock server should finish");
}

#[test]
fn claude_adapter_uses_the_messages_api_for_models_and_probes() {
    let echo = "claude-contract-echo";
    let claude_headers = vec![
        "x-api-key: sentinel-key",
        "anthropic-version: 2023-06-01",
        "!authorization",
    ];
    let (base_url, server) = spawn_mock_server(vec![
        ExpectedExchange {
            method: "GET",
            path: "/v1/models?limit=1000",
            assert_body: assert_empty_body,
            status: 200,
            response_body: json!({
                "data": [
                    claude_model("claude-haiku-5-5", "Claude Haiku 5.5", true, true),
                    claude_model("claude-legacy", "Claude Legacy", false, true)
                ],
                "has_more": false,
                "first_id": "claude-haiku-5-5",
                "last_id": "claude-legacy"
            })
            .to_string(),
            extra_headers: Vec::new(),
            request_headers: claude_headers.clone(),
        },
        ExpectedExchange {
            method: "GET",
            path: "/v1/models/claude-haiku-5-5",
            assert_body: assert_empty_body,
            status: 200,
            response_body: claude_model("claude-haiku-5-5", "Claude Haiku 5.5", true, true)
                .to_string(),
            extra_headers: Vec::new(),
            request_headers: claude_headers.clone(),
        },
        ExpectedExchange {
            method: "POST",
            path: "/v1/messages",
            assert_body: assert_claude_text_request,
            status: 200,
            response_body: claude_message("OSTUDY_OK", "end_turn"),
            extra_headers: Vec::new(),
            request_headers: claude_headers.clone(),
        },
        ExpectedExchange {
            method: "POST",
            path: "/v1/messages",
            assert_body: assert_claude_structured_request,
            status: 200,
            response_body: claude_message(
                &json!({ "ok": true, "echo": echo }).to_string(),
                "end_turn",
            ),
            extra_headers: Vec::new(),
            request_headers: claude_headers,
        },
    ]);
    let mut profile = profile(
        ProviderKind::Claude,
        format!("{base_url}v1/"),
        "claude-haiku-5-5",
        AuthMode::Bearer,
    );
    profile.reasoning_effort = "none".to_string();
    let gateway = AiGateway::new().expect("gateway should initialize");

    tauri::async_runtime::block_on(async {
        let models = providers::list_models(&gateway, &profile, Some("sentinel-key"))
            .await
            .expect("models should load");
        assert_eq!(
            models.len(),
            1,
            "models without structured outputs are hidden"
        );
        assert_eq!(models[0].display_name, "Claude Haiku 5.5");
        assert_eq!(models[0].context_window_tokens, Some(1_000_000));
        let reasoning = models[0].reasoning.as_ref().expect("reasoning facts");
        assert!(!reasoning.mandatory);
        let properties = providers::load_properties(&gateway, &profile, Some("sentinel-key"))
            .await
            .expect("model details should load");
        assert_eq!(properties.image_input_declared, Some(true));
        let text = providers::run_text_probe(&gateway, &profile, Some("sentinel-key"))
            .await
            .expect("text probe should pass");
        assert_eq!(text.prompt_tokens, Some(130));
        assert_eq!(text.completion_tokens, Some(3));
        providers::run_structured_probe(&gateway, &profile, Some("sentinel-key"), echo)
            .await
            .expect("schema-constrained JSON should pass local validation");
    });
    server.join().expect("mock server should finish");
}

#[test]
fn claude_document_probe_enforces_the_contract_with_output_config() {
    let (base_url, server) = spawn_mock_server(vec![ExpectedExchange {
        method: "POST",
        path: "/v1/messages",
        assert_body: assert_claude_document_request,
        status: 200,
        response_body: claude_message(
            &json!({"cards": [
                {"type":"basic","front":"Probe question","back":"Probe answer","tags":["probe"]},
                {"type":"cloze","text":"The probe value is stable.","blanks":["stable"],
                    "mode":"one_at_a_time","tags":["probe"]}
            ]})
            .to_string(),
            "end_turn",
        ),
        extra_headers: Vec::new(),
        request_headers: vec!["x-api-key: sentinel-key"],
    }]);
    let profile = profile(
        ProviderKind::Claude,
        format!("{base_url}v1/"),
        "claude-opus-5-5",
        AuthMode::Bearer,
    );
    let gateway = AiGateway::new().expect("gateway should initialize");
    tauri::async_runtime::block_on(async {
        providers::run_document_schema_probe(&gateway, &profile, Some("sentinel-key"), 1_280, None)
            .await
            .expect("document contract should pass");
    });
    server.join().expect("mock server should finish");
}

#[test]
fn openai_adapter_lists_chat_models_and_probes_with_reasoning_effort() {
    let echo = "openai-contract-echo";
    let bearer = vec!["authorization: bearer sentinel-key", "!x-api-key"];
    let (base_url, server) = spawn_mock_server(vec![
        ExpectedExchange {
            method: "GET",
            path: "/v1/models",
            assert_body: assert_empty_body,
            status: 200,
            response_body: json!({"object": "list", "data": [
                {"id": "gpt-5.4-mini", "object": "model", "created": 1, "owned_by": "system"},
                {"id": "text-embedding-3-small", "object": "model", "created": 1, "owned_by": "system"},
                {"id": "gpt-4o-mini-tts", "object": "model", "created": 1, "owned_by": "system"}
            ]})
            .to_string(),
            extra_headers: Vec::new(),
            request_headers: bearer.clone(),
        },
        ExpectedExchange {
            method: "POST",
            path: "/v1/chat/completions",
            assert_body: assert_openai_text_request,
            status: 200,
            response_body: completion_response("OSTUDY_OK"),
            extra_headers: Vec::new(),
            request_headers: bearer.clone(),
        },
        ExpectedExchange {
            method: "POST",
            path: "/v1/chat/completions",
            assert_body: assert_openai_structured_request,
            status: 200,
            response_body: completion_response(&json!({ "ok": true, "echo": echo }).to_string()),
            extra_headers: Vec::new(),
            request_headers: bearer,
        },
    ]);
    let mut profile = profile(
        ProviderKind::Openai,
        format!("{base_url}v1/"),
        "gpt-5.4-mini",
        AuthMode::Bearer,
    );
    profile.reasoning_effort = "none".to_string();
    let gateway = AiGateway::new().expect("gateway should initialize");

    tauri::async_runtime::block_on(async {
        let models = providers::list_models(&gateway, &profile, Some("sentinel-key"))
            .await
            .expect("models should load");
        assert_eq!(models.len(), 1, "only chat models are listed");
        assert_eq!(models[0].owned_by.as_deref(), Some("OpenAI"));
        assert!(models[0]
            .reasoning
            .as_ref()
            .is_some_and(|value| !value.mandatory));
        let properties = providers::load_properties(&gateway, &profile, Some("sentinel-key"))
            .await
            .expect("model facts need no request");
        assert_eq!(properties.image_input_declared, Some(true));
        let text = providers::run_text_probe(&gateway, &profile, Some("sentinel-key"))
            .await
            .expect("text probe should pass");
        assert_eq!(text.prompt_tokens, Some(5));
        providers::run_structured_probe(&gateway, &profile, Some("sentinel-key"), echo)
            .await
            .expect("strict JSON schema output should pass local validation");
    });
    server.join().expect("mock server should finish");
}

#[test]
fn openai_document_probe_uses_a_strict_schema_on_non_reasoning_models() {
    let (base_url, server) = spawn_mock_server(vec![ExpectedExchange {
        method: "POST",
        path: "/v1/chat/completions",
        assert_body: assert_openai_document_request,
        status: 200,
        response_body: completion_response(
            &json!({"cards": [
                {"type":"basic","front":"Probe question","back":"Probe answer","tags":["probe"]},
                {"type":"cloze","text":"The probe value is stable.","blanks":["stable"],
                    "mode":"one_at_a_time","tags":["probe"]}
            ]})
            .to_string(),
        ),
        extra_headers: Vec::new(),
        request_headers: vec!["authorization: bearer sentinel-key"],
    }]);
    let profile = profile(
        ProviderKind::Openai,
        format!("{base_url}v1/"),
        "gpt-4.1-mini",
        AuthMode::Bearer,
    );
    let gateway = AiGateway::new().expect("gateway should initialize");
    tauri::async_runtime::block_on(async {
        providers::run_document_schema_probe(&gateway, &profile, Some("sentinel-key"), 1_280, None)
            .await
            .expect("document contract should pass");
    });
    server.join().expect("mock server should finish");
}

fn assert_openai_text_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    assert_eq!(value["model"], "gpt-5.4-mini");
    assert_eq!(value["max_completion_tokens"], 64);
    assert!(value.get("max_tokens").is_none());
    assert!(value.get("temperature").is_none());
    assert_eq!(value["reasoning_effort"], "none");
    assert_eq!(value["messages"][0]["role"], "system");
}

fn assert_openai_structured_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    assert_eq!(value["response_format"]["type"], "json_schema");
    assert_eq!(value["response_format"]["json_schema"]["strict"], true);
    assert_eq!(
        value["response_format"]["json_schema"]["schema"]["additionalProperties"],
        false
    );
    assert_eq!(value["reasoning_effort"], "none");
}

fn assert_openai_document_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    assert_eq!(value["model"], "gpt-4.1-mini");
    assert!(value.get("reasoning_effort").is_none());
    assert!(value.get("temperature").is_some());
    assert!(value.get("max_completion_tokens").is_some());
    assert!(value.get("max_tokens").is_none());
    assert_eq!(value["response_format"]["json_schema"]["strict"], true);
}

#[test]
fn gemini_adapter_uses_generate_content_for_models_and_probes() {
    let echo = "gemini-contract-echo";
    let gemini_headers = vec!["x-goog-api-key: sentinel-key", "!authorization"];
    let (base_url, server) = spawn_mock_server(vec![
        ExpectedExchange {
            method: "GET",
            path: "/v1beta/models?pageSize=1000",
            assert_body: assert_empty_body,
            status: 200,
            response_body: json!({"models": [
                gemini_model("gemini-2.5-flash", "Gemini 2.5 Flash"),
                {"name": "models/gemini-embedding-001", "displayName": "Gemini Embedding",
                    "supportedGenerationMethods": ["embedContent"]}
            ]})
            .to_string(),
            extra_headers: Vec::new(),
            request_headers: gemini_headers.clone(),
        },
        ExpectedExchange {
            method: "GET",
            path: "/v1beta/models/gemini-2.5-flash",
            assert_body: assert_empty_body,
            status: 200,
            response_body: gemini_model("gemini-2.5-flash", "Gemini 2.5 Flash").to_string(),
            extra_headers: Vec::new(),
            request_headers: gemini_headers.clone(),
        },
        ExpectedExchange {
            method: "POST",
            path: "/v1beta/models/gemini-2.5-flash:generateContent",
            assert_body: assert_gemini_text_request,
            status: 200,
            response_body: gemini_response("OSTUDY_OK"),
            extra_headers: Vec::new(),
            request_headers: gemini_headers.clone(),
        },
        ExpectedExchange {
            method: "POST",
            path: "/v1beta/models/gemini-2.5-flash:generateContent",
            assert_body: assert_gemini_structured_request,
            status: 200,
            response_body: gemini_response(&json!({ "ok": true, "echo": echo }).to_string()),
            extra_headers: Vec::new(),
            request_headers: gemini_headers,
        },
    ]);
    let mut profile = profile(
        ProviderKind::Gemini,
        format!("{base_url}v1beta/"),
        "gemini-2.5-flash",
        AuthMode::Bearer,
    );
    profile.reasoning_effort = "none".to_string();
    let gateway = AiGateway::new().expect("gateway should initialize");

    tauri::async_runtime::block_on(async {
        let models = providers::list_models(&gateway, &profile, Some("sentinel-key"))
            .await
            .expect("models should load");
        assert_eq!(
            models.len(),
            1,
            "only content-generating chat models are listed"
        );
        assert_eq!(models[0].id, "gemini-2.5-flash");
        assert_eq!(models[0].display_name, "Gemini 2.5 Flash");
        assert_eq!(models[0].context_window_tokens, Some(1_048_576));
        assert!(models[0]
            .reasoning
            .as_ref()
            .is_some_and(|value| !value.mandatory));
        let properties = providers::load_properties(&gateway, &profile, Some("sentinel-key"))
            .await
            .expect("model details should load");
        assert_eq!(properties.image_input_declared, Some(true));
        let text = providers::run_text_probe(&gateway, &profile, Some("sentinel-key"))
            .await
            .expect("text probe should pass");
        assert_eq!(text.prompt_tokens, Some(21));
        assert_eq!(text.completion_tokens, Some(4));
        providers::run_structured_probe(&gateway, &profile, Some("sentinel-key"), echo)
            .await
            .expect("schema-constrained JSON should pass local validation");
    });
    server.join().expect("mock server should finish");
}

#[test]
fn gemini_document_probe_enforces_the_contract_with_a_json_schema() {
    let (base_url, server) = spawn_mock_server(vec![ExpectedExchange {
        method: "POST",
        path: "/v1beta/models/gemini-3-flash:generateContent",
        assert_body: assert_gemini_document_request,
        status: 200,
        response_body: gemini_response(
            &json!({"cards": [
                {"type":"basic","front":"Probe question","back":"Probe answer","tags":["probe"]},
                {"type":"cloze","text":"The probe value is stable.","blanks":["stable"],
                    "mode":"one_at_a_time","tags":["probe"]}
            ]})
            .to_string(),
        ),
        extra_headers: Vec::new(),
        request_headers: vec!["x-goog-api-key: sentinel-key"],
    }]);
    let profile = profile(
        ProviderKind::Gemini,
        format!("{base_url}v1beta/"),
        "gemini-3-flash",
        AuthMode::Bearer,
    );
    let gateway = AiGateway::new().expect("gateway should initialize");
    tauri::async_runtime::block_on(async {
        providers::run_document_schema_probe(&gateway, &profile, Some("sentinel-key"), 1_280, None)
            .await
            .expect("document contract should pass");
    });
    server.join().expect("mock server should finish");
}

fn gemini_model(id: &str, name: &str) -> Value {
    json!({
        "name": format!("models/{id}"),
        "baseModelId": id,
        "version": "001",
        "displayName": name,
        "inputTokenLimit": 1_048_576,
        "outputTokenLimit": 65_536,
        "supportedGenerationMethods": ["generateContent", "countTokens", "createCachedContent"],
        "temperature": 1.0,
        "maxTemperature": 2.0,
        "thinking": true
    })
}

fn gemini_response(text: &str) -> String {
    json!({
        "candidates": [{
            "content": {"role": "model", "parts": [{"text": text}]},
            "finishReason": "STOP",
            "index": 0
        }],
        "usageMetadata": {"promptTokenCount": 21, "candidatesTokenCount": 4, "totalTokenCount": 25},
        "modelVersion": "gemini-2.5-flash",
        "responseId": "contract"
    })
    .to_string()
}

fn assert_gemini_text_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    assert!(value.get("model").is_none());
    assert!(value.get("messages").is_none());
    assert!(value["systemInstruction"]["parts"][0]["text"]
        .as_str()
        .unwrap()
        .contains("connectivity test"));
    assert_eq!(value["contents"][0]["role"], "user");
    assert_eq!(value["generationConfig"]["maxOutputTokens"], 64);
    assert_eq!(
        value["generationConfig"]["thinkingConfig"]["thinkingBudget"],
        0
    );
}

fn assert_gemini_structured_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    assert_eq!(
        value["generationConfig"]["responseMimeType"],
        "application/json"
    );
    assert_eq!(
        value["generationConfig"]["responseJsonSchema"]["additionalProperties"],
        false
    );
}

fn assert_gemini_document_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    let config = &value["generationConfig"];
    // Gemini 3 cannot stop thinking; "low" is a thinking level.
    assert_eq!(config["thinkingConfig"]["thinkingLevel"], "LOW");
    assert!(config.get("temperature").is_none());
    assert!(config
        .pointer("/responseJsonSchema/properties/cards/items/anyOf")
        .is_some());
}

fn claude_model(id: &str, name: &str, structured_outputs: bool, image_input: bool) -> Value {
    json!({
        "type": "model",
        "id": id,
        "display_name": name,
        "created_at": "2026-09-01T00:00:00Z",
        "max_input_tokens": 1_000_000,
        "max_tokens": 128_000,
        "capabilities": {
            "image_input": {"supported": image_input},
            "structured_outputs": {"supported": structured_outputs},
            "thinking": {"supported": true, "types": {
                "adaptive": {"supported": true},
                "disabled": {"supported": true},
                "enabled": {"supported": false}
            }},
            "effort": {
                "supported": true,
                "low": {"supported": true}, "medium": {"supported": true},
                "high": {"supported": true}, "xhigh": {"supported": true},
                "max": {"supported": true}
            }
        }
    })
}

fn claude_message(text: &str, stop_reason: &str) -> String {
    json!({
        "id": "msg_contract",
        "type": "message",
        "role": "assistant",
        "model": "claude-haiku-5-5",
        "content": [{"type": "text", "text": text}],
        "stop_reason": stop_reason,
        "usage": {
            "input_tokens": 10,
            "cache_read_input_tokens": 120,
            "cache_creation_input_tokens": 0,
            "output_tokens": 3
        }
    })
    .to_string()
}

fn assert_claude_text_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    assert_eq!(value["model"], "claude-haiku-5-5");
    assert_eq!(value["max_tokens"], 64);
    assert!(value.get("temperature").is_none());
    assert!(value.get("stream_options").is_none());
    assert_eq!(value["thinking"]["type"], "disabled");
    assert!(value["system"][0]["text"]
        .as_str()
        .unwrap()
        .contains("connectivity test"));
    assert_eq!(value["messages"][0]["role"], "user");
    assert_eq!(value["messages"].as_array().unwrap().len(), 1);
}

fn assert_claude_structured_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    assert_eq!(value["output_config"]["format"]["type"], "json_schema");
    assert_eq!(
        value["output_config"]["format"]["schema"]["additionalProperties"],
        false
    );
    assert!(value.get("response_format").is_none());
}

fn assert_claude_document_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    // Opus at "low" effort reasons adaptively; the probe budget has headroom.
    assert_eq!(value["thinking"]["type"], "adaptive");
    assert_eq!(value["output_config"]["effort"], "low");
    let schema = &value["output_config"]["format"]["schema"];
    assert!(schema.pointer("/properties/cards/items/anyOf").is_some());
    assert!(schema.pointer("/properties/cards/minItems").is_none());
}

fn profile(
    provider_kind: ProviderKind,
    base_url: String,
    model_id: &str,
    auth_mode: AuthMode,
) -> ProfileSnapshot {
    ProfileSnapshot {
        id: "0123456789abcdef0123456789abcdef".to_string(),
        provider_kind,
        base_url,
        model_id: model_id.to_string(),
        reasoning_effort: "low".to_string(),
        auth_mode,
        enabled: true,
        allow_insecure_http: false,
        require_zero_data_retention: false,
        request_timeout_ms: 5_000,
        credential_epoch: 0,
        revision: 1,
    }
}

fn completion_response(content: &str) -> String {
    completion_response_with_finish(content, "stop")
}

fn completion_response_with_finish(content: &str, finish_reason: &str) -> String {
    json!({
        "id": "completion-id",
        "model": "mock-model",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": content },
            "finish_reason": finish_reason
        }],
        "usage": { "prompt_tokens": 5, "completion_tokens": 3 }
    })
    .to_string()
}

fn assert_empty_body(body: &str) {
    assert!(body.is_empty());
}

fn assert_llama_text_completion_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    assert_eq!(value.get("temperature").and_then(Value::as_i64), Some(0));
    assert_eq!(value.get("stream").and_then(Value::as_bool), Some(false));
    assert_eq!(
        value.get("reasoning_effort").and_then(Value::as_str),
        Some("none")
    );
    assert_eq!(
        value
            .pointer("/chat_template_kwargs/enable_thinking")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(value.get("response_format").is_none());
    assert!(value.get("messages").and_then(Value::as_array).is_some());
}

fn assert_deepseek_text_completion_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    assert_eq!(
        value.get("model").and_then(Value::as_str),
        Some("deepseek-v4-flash")
    );
    assert_eq!(value.get("temperature").and_then(Value::as_i64), Some(0));
    assert_eq!(value.get("stream").and_then(Value::as_bool), Some(false));
    assert_eq!(
        value.pointer("/thinking/type").and_then(Value::as_str),
        Some("disabled")
    );
    assert!(value.get("response_format").is_none());
    assert!(value.get("messages").and_then(Value::as_array).is_some());
}

fn assert_llama_schema_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    assert_eq!(value.get("stream").and_then(Value::as_bool), Some(false));
    assert_eq!(
        value.get("reasoning_effort").and_then(Value::as_str),
        Some("none")
    );
    assert_eq!(
        value
            .pointer("/chat_template_kwargs/enable_thinking")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        value
            .pointer("/response_format/type")
            .and_then(Value::as_str),
        Some("json_schema")
    );
    assert_eq!(
        value
            .pointer("/response_format/json_schema/strict")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(value
        .pointer("/response_format/json_schema/schema")
        .is_some());
}

fn assert_deepseek_json_object_request(body: &str) {
    let value: Value = serde_json::from_str(body).expect("request should be JSON");
    assert_eq!(
        value.get("model").and_then(Value::as_str),
        Some("deepseek-v4-flash")
    );
    assert_eq!(value.get("stream").and_then(Value::as_bool), Some(false));
    assert_eq!(
        value.pointer("/thinking/type").and_then(Value::as_str),
        Some("disabled")
    );
    assert_eq!(
        value
            .pointer("/response_format/type")
            .and_then(Value::as_str),
        Some("json_object")
    );
    assert!(value.pointer("/response_format/json_schema").is_none());
    let system = value
        .pointer("/messages/0/content")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(system.contains("valid JSON"));
}

fn spawn_mock_server(exchanges: Vec<ExpectedExchange>) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener should bind");
    let address = listener.local_addr().expect("mock address should resolve");
    let queue = Arc::new(Mutex::new(VecDeque::from(exchanges)));
    let server_queue = Arc::clone(&queue);
    let handle = thread::spawn(move || {
        listener
            .set_nonblocking(false)
            .expect("listener should remain blocking");
        loop {
            let expected = {
                let mut guard = server_queue.lock().expect("exchange queue should lock");
                guard.pop_front()
            };
            let Some(expected) = expected else {
                break;
            };
            let (mut stream, _) = listener.accept().expect("mock request should arrive");
            let request = read_request(&mut stream);
            assert_eq!(request.method, expected.method);
            assert_eq!(request.path, expected.path);
            (expected.assert_body)(&request.body);
            for header in &expected.request_headers {
                match header.strip_prefix('!') {
                    Some(name) => assert!(
                        !request.headers.contains(&format!("\n{name}:")),
                        "request header {name} should be absent"
                    ),
                    None => assert!(
                        request.headers.contains(&format!("\n{header}\r")),
                        "request header {header} should be present"
                    ),
                }
            }
            write_response(&mut stream, expected);
        }
        assert!(server_queue
            .lock()
            .expect("exchange queue should lock")
            .is_empty());
    });
    (format!("http://{address}/"), handle)
}

fn read_request(stream: &mut TcpStream) -> MockRequest {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("read timeout should set");
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    let header_end = loop {
        let count = stream.read(&mut buffer).expect("request should read");
        assert!(count > 0, "request closed before headers completed");
        bytes.extend_from_slice(&buffer[..count]);
        if let Some(index) = find_subsequence(&bytes, b"\r\n\r\n") {
            break index + 4;
        }
        assert!(
            bytes.len() < 64 * 1024,
            "request headers exceeded test bound"
        );
    };
    let header_text =
        String::from_utf8(bytes[..header_end].to_vec()).expect("request headers should be UTF-8");
    let content_length = header_text
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length").then(|| {
                value
                    .trim()
                    .parse::<usize>()
                    .expect("content length should parse")
            })
        })
        .unwrap_or(0);
    while bytes.len() < header_end + content_length {
        let count = stream.read(&mut buffer).expect("request body should read");
        assert!(count > 0, "request closed before body completed");
        bytes.extend_from_slice(&buffer[..count]);
    }
    let request_line = header_text
        .lines()
        .next()
        .expect("request line should exist");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().expect("method should exist").to_string();
    let path = parts.next().expect("path should exist").to_string();
    let body = String::from_utf8(bytes[header_end..header_end + content_length].to_vec())
        .expect("request body should be UTF-8");
    MockRequest {
        method,
        path,
        headers: header_text.to_ascii_lowercase(),
        body,
    }
}

fn write_response(stream: &mut TcpStream, expected: ExpectedExchange) {
    let status_text = match expected.status {
        200 => "OK",
        302 => "Found",
        401 => "Unauthorized",
        429 => "Too Many Requests",
        _ => "Error",
    };
    let mut headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        expected.status,
        status_text,
        expected.response_body.len()
    );
    for (name, value) in expected.extra_headers {
        headers.push_str(name);
        headers.push_str(": ");
        headers.push_str(value);
        headers.push_str("\r\n");
    }
    headers.push_str("\r\n");
    stream
        .write_all(headers.as_bytes())
        .expect("response headers should write");
    stream
        .write_all(expected.response_body.as_bytes())
        .expect("response body should write");
    stream.flush().expect("response should flush");
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
