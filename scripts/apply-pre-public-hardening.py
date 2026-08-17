#!/usr/bin/env python3
"""Apply the large, deterministic source and schema transformations for PR #54.

This helper is intentionally branch-scoped and temporary. It exists because the
provider/document and database composition modules are large enough that exact
in-repository transformations are safer than reconstructing whole files through
remote editor payloads. The applying workflow removes this helper before the PR
is marked ready.
"""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import sqlite3
import subprocess
import tempfile
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]


def read(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8")


def write(relative: str, content: str, *, executable: bool = False) -> None:
    path = ROOT / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")
    if executable:
        path.chmod(0o755)


def replace_once(relative: str, old: str, new: str) -> None:
    content = read(relative)
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"{relative}: expected one exact replacement, found {count}")
    write(relative, content.replace(old, new, 1))


def regex_once(relative: str, pattern: str, replacement: str, *, flags: int = 0) -> None:
    content = read(relative)
    updated, count = re.subn(pattern, replacement, content, count=1, flags=flags)
    if count != 1:
        raise RuntimeError(f"{relative}: expected one regex replacement, found {count}: {pattern}")
    write(relative, updated)


def append_once(relative: str, marker: str, addition: str) -> None:
    content = read(relative)
    if marker in content:
        return
    write(relative, content.rstrip() + "\n\n" + addition.strip() + "\n")


def patch_provider_runtime() -> None:
    providers = "src-tauri/src/ai/providers.rs"
    replace_once(
        providers,
        "const DEFAULT_MAX_MODEL_LIST_BYTES: usize = 512 * 1024;",
        "pub(crate) fn openrouter_provider_preferences(require_parameters: bool) -> Value {\n"
        "    openrouter::provider_preferences(require_parameters)\n"
        "}\n\n"
        "const DEFAULT_MAX_MODEL_LIST_BYTES: usize = 512 * 1024;",
    )
    replace_once(providers, '"modelId": profile.model_id,', '"modelId": profile.model_id.as_str(),')
    replace_once(
        providers,
        '"finishReason": outcome.finish_reason,',
        '"finishReason": outcome.finish_reason.as_deref(),',
    )
    replace_once(
        providers,
        """    let value = gateway
        .get_json(url, bearer, profile.request_timeout_ms, maximum_bytes)
        .await?;
    let models = parse_model_list(&value, adapter, maximum_models)?;
""",
        """    let value = match gateway
        .get_json(url, bearer, profile.request_timeout_ms, maximum_bytes)
        .await
    {
        Ok(value) => value,
        Err(error_value)
            if adapter == ProviderAdapter::Openrouter
                && error_value.safe.code == "ai_http_not_found" =>
        {
            crate::logging::warn_target(
                "ai.provider.openrouter",
                "openrouter_user_model_catalog_unavailable",
                "The account-filtered OpenRouter model catalog was unavailable; ostudy is using the public catalog for this refresh.",
                json!({}),
            );
            let public_url =
                join_endpoint(&profile.base_url, "models").map_err(command_to_gateway)?;
            gateway
                .get_json(
                    public_url,
                    bearer,
                    profile.request_timeout_ms,
                    maximum_bytes,
                )
                .await?
        }
        Err(error_value) => return Err(error_value),
    };
    let models = parse_model_list(&value, adapter, maximum_models)?;
""",
    )

    document = "src-tauri/src/ai/document_intelligence.rs"
    replace_once(
        document,
        "    profiles::{load_profile_snapshot, ProfileSnapshot},\n    secrets::CredentialStorage,",
        "    profiles::{load_profile_snapshot, ProfileSnapshot},\n    providers,\n    secrets::CredentialStorage,",
    )
    replace_once(
        document,
        """    match provider_kind {
        ProviderKind::Deepseek => 3_072,
        ProviderKind::LlamaCpp => 2_048,
        _ => 2_048,
    }
""",
        """    match provider_kind {
        ProviderKind::Deepseek | ProviderKind::Openrouter => 3_072,
        ProviderKind::LlamaCpp => 2_048,
        _ => 2_048,
    }
""",
    )
    helper = """
fn openrouter_document_body(profile: &ProfileSnapshot, request: &JsonTaskRequest<'_>) -> Value {
    json!({
        "model": profile.model_id.as_str(),
        "messages": [
            { "role": "system", "content": request.system_prompt },
            { "role": "user", "content": request.user_text }
        ],
        "temperature": 0.12,
        "max_tokens": request.max_tokens,
        "stream": false,
        "provider": providers::openrouter_provider_preferences(true),
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "ostudy_document_task",
                "strict": true,
                "schema": request.schema
            }
        }
    })
}

"""
    replace_once(document, "async fn request_json(\n", helper + "async fn request_json(\n")
    replace_once(
        document,
        """        ProviderKind::Deepseek => {
            let strict_schema = deepseek_strict_schema(request.schema);
            (
                deepseek_strict_completion_endpoint(&task.profile)?,
                json!({
                    "model": task.profile.model_id,
                    "messages": [
                        { "role": "system", "content": request.system_prompt },
                        { "role": "user", "content": request.user_text }
                    ],
                    "temperature": 0.12,
                    "max_tokens": request.max_tokens,
                    "stream": false,
                    "thinking": { "type": "disabled" },
                    "tools": [{
                        "type": "function",
                        "function": {
                            "name": DEEPSEEK_STRICT_TOOL_NAME,
                            "description": "Submit the exact structured ostudy document-task result.",
                            "strict": true,
                            "parameters": strict_schema
                        }
                    }],
                    "tool_choice": {
                        "type": "function",
                        "function": { "name": DEEPSEEK_STRICT_TOOL_NAME }
                    }
                }),
            )
        }
        _ => {
""",
        """        ProviderKind::Deepseek => {
            let strict_schema = deepseek_strict_schema(request.schema);
            (
                deepseek_strict_completion_endpoint(&task.profile)?,
                json!({
                    "model": task.profile.model_id,
                    "messages": [
                        { "role": "system", "content": request.system_prompt },
                        { "role": "user", "content": request.user_text }
                    ],
                    "temperature": 0.12,
                    "max_tokens": request.max_tokens,
                    "stream": false,
                    "thinking": { "type": "disabled" },
                    "tools": [{
                        "type": "function",
                        "function": {
                            "name": DEEPSEEK_STRICT_TOOL_NAME,
                            "description": "Submit the exact structured ostudy document-task result.",
                            "strict": true,
                            "parameters": strict_schema
                        }
                    }],
                    "tool_choice": {
                        "type": "function",
                        "function": { "name": DEEPSEEK_STRICT_TOOL_NAME }
                    }
                }),
            )
        }
        ProviderKind::Openrouter => (
            completion_endpoint(&task.profile)?,
            openrouter_document_body(&task.profile, &request),
        ),
        _ => {
""",
    )
    replace_once(
        document,
        """        ProviderKind::Deepseek => deepseek_strict_completion_endpoint(profile),
        _ => Err(CommandError::new(
""",
        """        ProviderKind::Deepseek => deepseek_strict_completion_endpoint(profile),
        ProviderKind::Openrouter => join_endpoint(&profile.base_url, "chat/completions"),
        _ => Err(CommandError::new(
""",
    )
    replace_once(
        document,
        """    fn deepseek_strict_schema_uses_supported_required_shapes() {
""",
        """    fn openrouter_document_requests_require_private_parameter_compatible_json_schema() {
        let profile = ProfileSnapshot {
            id: "a".repeat(32),
            provider_kind: ProviderKind::Openrouter,
            base_url: "https://openrouter.ai/api/v1/".to_string(),
            model_id: "openai/example".to_string(),
            auth_mode: super::super::catalog::AuthMode::Bearer,
            enabled: true,
            allow_insecure_http: false,
            request_timeout_ms: 60_000,
            credential_epoch: 0,
            revision: 1,
        };
        let schema = json!({ "type": "object" });
        let request = JsonTaskRequest {
            system_prompt: "system",
            user_text: "source",
            schema: &schema,
            max_tokens: 128,
            images: &[],
        };
        let body = openrouter_document_body(&profile, &request);
        assert_eq!(body.pointer("/provider/require_parameters"), Some(&json!(true)));
        assert_eq!(body.pointer("/provider/data_collection"), Some(&json!("deny")));
        assert_eq!(body.pointer("/response_format/type"), Some(&json!("json_schema")));
        assert!(body.get("plugins").is_none());
        assert!(body.get("debug").is_none());
    }

    #[test]
    fn deepseek_strict_schema_uses_supported_required_shapes() {
""",
    )

    profiles = "src-tauri/src/ai/profiles.rs"
    replace_once(
        profiles,
        """    if provider_kind == ProviderKind::Deepseek && auth_mode != AuthMode::Bearer {
        return Err(CommandError::new(
            "invalid_ai_auth_mode",
            "DeepSeek profiles require bearer-token authentication.",
        ));
    }
""",
        """    if matches!(provider_kind, ProviderKind::Deepseek | ProviderKind::Openrouter)
        && auth_mode != AuthMode::Bearer
    {
        return Err(CommandError::new(
            "invalid_ai_auth_mode",
            "Built-in cloud provider profiles require bearer-token authentication.",
        ));
    }
""",
    )

    probes = "src-tauri/src/ai/probes.rs"
    replace_once(probes, "const ADAPTER_REVISION: i64 = 2;", "const ADAPTER_REVISION: i64 = 3;")
    replace_once(
        probes,
        """        match profile.provider_kind {
            ProviderKind::LlamaCpp if structured_output => CapabilityStatus::Supported,
            ProviderKind::LlamaCpp => CapabilityStatus::Unknown,
            ProviderKind::Deepseek => CapabilityStatus::Unsupported,
            _ => CapabilityStatus::Unknown,
        },
""",
        """        match profile.provider_kind {
            ProviderKind::LlamaCpp | ProviderKind::Openrouter if structured_output => {
                CapabilityStatus::Supported
            }
            ProviderKind::LlamaCpp | ProviderKind::Openrouter => CapabilityStatus::Unknown,
            ProviderKind::Deepseek => CapabilityStatus::Unsupported,
            _ => CapabilityStatus::Unknown,
        },
""",
    )
    replace_once(
        probes,
        """            base_url: if provider_kind == ProviderKind::LlamaCpp {
                "http://127.0.0.1:8080/".to_string()
            } else {
                "https://api.deepseek.com/".to_string()
            },
            model_id: model_id.to_string(),
            auth_mode: if provider_kind == ProviderKind::Deepseek {
                AuthMode::Bearer
            } else {
                AuthMode::None
            },
""",
        """            base_url: match provider_kind {
                ProviderKind::LlamaCpp => "http://127.0.0.1:8080/".to_string(),
                ProviderKind::Openrouter => "https://openrouter.ai/api/v1/".to_string(),
                _ => "https://api.deepseek.com/".to_string(),
            },
            model_id: model_id.to_string(),
            auth_mode: if matches!(provider_kind, ProviderKind::Deepseek | ProviderKind::Openrouter)
            {
                AuthMode::Bearer
            } else {
                AuthMode::None
            },
""",
    )
    replace_once(
        probes,
        """    fn sole_llama_model_mismatch_uses_server_reported_id_for_probe() {
""",
        """    fn openrouter_probes_can_report_provider_enforced_json_schema() {
        let profile = profile(ProviderKind::Openrouter, "openai/example");
        let capabilities = build_capabilities(
            &profile,
            true,
            true,
            true,
            true,
            &ProviderProperties::default(),
        );
        let schema = capabilities
            .iter()
            .find(|capability| capability.key == "json_schema_output")
            .expect("schema capability should exist");
        assert_eq!(schema.status, CapabilityStatus::Supported);
    }

    #[test]
    fn sole_llama_model_mismatch_uses_server_reported_id_for_probe() {
""",
    )

    endpoint = "src-tauri/src/ai/endpoint_policy.rs"
    replace_once(
        endpoint,
        """    fn keeps_deepseek_on_the_official_endpoint() {
""",
        """    fn keeps_openrouter_on_the_official_endpoint() {
        assert!(validate_endpoint(
            ProviderKind::Openrouter,
            "https://openrouter.ai/api/v1/",
            false,
        )
        .is_ok());
        assert!(validate_endpoint(
            ProviderKind::Openrouter,
            "https://proxy.example.com/",
            false,
        )
        .is_err());
    }

    #[test]
    fn keeps_deepseek_on_the_official_endpoint() {
""",
    )


def patch_gateway() -> None:
    gateway = "src-tauri/src/ai/gateway.rs"
    replace_once(
        gateway,
        "const STREAM_WIRE_MULTIPLIER: usize = 16;",
        'const STREAM_WIRE_MULTIPLIER: usize = 16;\nconst OPENROUTER_HOST: &str = "openrouter.ai";',
    )
    replace_once(
        gateway,
        """        let mut request = self
            .streaming_client
            .post(url)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header(reqwest::header::CACHE_CONTROL, "no-cache")
            .json(&streaming_body);
""",
        """        let openrouter = is_openrouter_url(&url);
        let mut request = self
            .streaming_client
            .post(url)
            .header(reqwest::header::ACCEPT, "text/event-stream")
            .header(reqwest::header::CACHE_CONTROL, "no-cache")
            .json(&streaming_body);
        if openrouter {
            request = request.header("X-OpenRouter-Title", "ostudy");
        }
""",
    )
    replace_once(
        gateway,
        """        let mut request = self
            .client
            .request(method, url)
            .timeout(Duration::from_millis(timeout_ms as u64))
            .header(reqwest::header::ACCEPT, "application/json");
""",
        """        let openrouter = is_openrouter_url(&url);
        let mut request = self
            .client
            .request(method, url)
            .timeout(Duration::from_millis(timeout_ms as u64))
            .header(reqwest::header::ACCEPT, "application/json");
        if openrouter {
            request = request.header("X-OpenRouter-Title", "ostudy");
        }
""",
    )
    replace_once(
        gateway,
        """        let choice = value
            .get("choices")
""",
        """        if value.get("error").is_some() {
            return Err(classify_stream_event_error(&value));
        }
        let choice = value
            .get("choices")
""",
    )
    helper = """
fn is_openrouter_url(url: &Url) -> bool {
    url.scheme() == "https" && url.host_str() == Some(OPENROUTER_HOST)
}

fn classify_stream_event_error(value: &Value) -> AiGatewayError {
    let code = value
        .pointer("/error/code")
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .or_else(|| {
            value
                .pointer("/error/code")
                .and_then(Value::as_i64)
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());
    let normalized = code.to_ascii_lowercase();
    let (safe_code, message, retryable) = match normalized.as_str() {
        "401" | "unauthorized" => (
            "ai_authentication_failed",
            "The AI provider rejected the saved credential.",
            false,
        ),
        "402" | "insufficient_credits" => (
            "ai_insufficient_credits",
            "The AI provider account does not have enough credits for this request.",
            false,
        ),
        "429" | "rate_limit_exceeded" => (
            "ai_rate_limited",
            "The AI provider rate-limited the request.",
            true,
        ),
        "503" | "no_available_providers" => (
            "ai_no_compatible_endpoint",
            "No compatible model endpoint was available for the requested parameters and privacy policy.",
            true,
        ),
        "context_length_exceeded" => (
            "ai_context_length_exceeded",
            "The provider rejected the request because it exceeded the model context window.",
            false,
        ),
        _ => (
            "ai_stream_interrupted",
            "The AI provider reported an error after streaming began. Partial output was discarded.",
            true,
        ),
    };
    crate::logging::warn_target(
        "ai.gateway.streaming",
        "provider_stream_error",
        message,
        json!({ "providerCode": code }),
    );
    error(safe_code, message, retryable, None)
}

"""
    replace_once(gateway, "fn initial_timeout_error(", helper + "fn initial_timeout_error(")
    replace_once(
        gateway,
        """    fn classifies_rate_limits_without_exposing_provider_bodies() {
""",
        """    fn rejects_in_band_stream_errors_after_partial_output() {
        let mut stream = SseChatAccumulator::new(1_024, 16_384);
        stream
            .push_bytes(b"data: {\\\"choices\\\":[{\\\"delta\\\":{\\\"content\\\":\\\"partial\\\"}}]}\\n")
            .expect("first delta should parse");
        let failure = stream
            .push_bytes(b"data: {\\\"error\\\":{\\\"code\\\":\\\"server_error\\\",\\\"message\\\":\\\"private upstream detail\\\"}}\\n")
            .expect_err("embedded stream error should abort the response");
        assert_eq!(failure.safe.code, "ai_stream_interrupted");
        assert!(!failure.safe.message.contains("private upstream detail"));
    }

    #[test]
    fn recognizes_only_the_fixed_openrouter_https_host_for_attribution() {
        assert!(is_openrouter_url(
            &Url::parse("https://openrouter.ai/api/v1/models").unwrap()
        ));
        assert!(!is_openrouter_url(
            &Url::parse("https://openrouter.ai.evil.example/api/v1/models").unwrap()
        ));
        assert!(!is_openrouter_url(
            &Url::parse("http://openrouter.ai/api/v1/models").unwrap()
        ));
    }

    #[test]
    fn classifies_rate_limits_without_exposing_provider_bodies() {
""",
    )


def patch_frontend_ai() -> None:
    settings = "src/screens/settings/AiLlmSettings.tsx"
    replace_once(
        settings,
        """      {!isLlama ? (
        <p className={styles.cloudNotice}>
          DeepSeek is a cloud provider. ostudy uses DeepSeek's fixed official API endpoint.
        </p>
      ) : null}
""",
        """      {!isLlama ? (
        <p className={styles.cloudNotice}>{cloudProviderNotice(draft.providerKind)}</p>
      ) : null}
""",
    )
    replace_once(
        settings,
        """}) {
  const profile = controller.selectedProfile;
  const draft = controller.draft;
  if (!profile || !draft) return null;

  const credentialReady = canUseProviderCredential(profile);
""",
        """}) {
  const [modelSearch, setModelSearch] = useState("");
  const profile = controller.selectedProfile;
  const draft = controller.draft;
  if (!profile || !draft) return null;

  const credentialReady = canUseProviderCredential(profile);
""",
    )
    replace_once(
        settings,
        """  const savedModelMissing =
    controller.modelsLoaded &&
    profile.modelId !== "" &&
    !controller.models.some((model) => model.id === profile.modelId);

  return (
""",
        """  const savedModelMissing =
    controller.modelsLoaded &&
    profile.modelId !== "" &&
    !controller.models.some((model) => model.id === profile.modelId);
  const normalizedSearch = modelSearch.trim().toLocaleLowerCase();
  const matchingModels = controller.models.filter((model) => {
    if (!normalizedSearch) return true;
    return `${model.displayName} ${model.id} ${model.ownedBy ?? ""}`
      .toLocaleLowerCase()
      .includes(normalizedSearch);
  });
  const visibleModels = matchingModels.slice(0, 250);
  const selectedModel = controller.models.find((model) => model.id === modelValue);
  if (selectedModel && !visibleModels.some((model) => model.id === selectedModel.id)) {
    visibleModels.unshift(selectedModel);
  }

  return (
""",
    )
    replace_once(
        settings,
        """          <label className={styles.stackedField}>
            <span>Model</span>
            <select
""",
        """          {controller.models.length > 50 ? (
            <label className={styles.stackedField}>
              <span>Search models</span>
              <input
                type="search"
                value={modelSearch}
                placeholder="Search model name, slug, or provider"
                onChange={(event) => setModelSearch(event.currentTarget.value)}
              />
            </label>
          ) : null}
          <label className={styles.stackedField}>
            <span>Model</span>
            <select
""",
    )
    replace_once(
        settings,
        """              {controller.models.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.displayName || model.id}
                </option>
              ))}
""",
        """              {visibleModels.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.displayName || model.id}
                  {model.displayName && model.displayName !== model.id ? ` — ${model.id}` : ""}
                </option>
              ))}
""",
    )
    replace_once(
        settings,
        """          <p className={styles.note}>
            {controller.models.length} model{controller.models.length === 1 ? "" : "s"} returned by the provider.
          </p>
""",
        """          <p className={styles.note}>
            {normalizedSearch
              ? `${matchingModels.length} of ${controller.models.length} models match this search. At most 250 matching models are rendered at once.`
              : `${controller.models.length} model${controller.models.length === 1 ? "" : "s"} returned by the provider.`}
          </p>
""",
    )
    replace_once(
        settings,
        """  const modelEndpoint =
    profile.providerKind === "llama_cpp"
      ? joinLlamaOpenAiDiagnosticEndpoint(profile.baseUrl, "models")
      : joinDiagnosticEndpoint(profile.baseUrl, "models");
""",
        """  const modelEndpoint =
    profile.providerKind === "llama_cpp"
      ? joinLlamaOpenAiDiagnosticEndpoint(profile.baseUrl, "models")
      : joinDiagnosticEndpoint(
          profile.baseUrl,
          profile.providerKind === "openrouter" ? "models/user" : "models",
        );
""",
    )
    replace_once(
        settings,
        """  const structuredFormat =
    profile.providerKind === "llama_cpp"
      ? "json_schema"
      : "json_object + local_schema_validation";
""",
        """  const structuredFormat =
    profile.providerKind === "llama_cpp"
      ? "json_schema"
      : profile.providerKind === "openrouter"
        ? "json_schema + require_parameters + data_collection=deny"
        : "json_object + local_schema_validation";
""",
    )
    helper = """
function cloudProviderNotice(providerKind: AiProviderKind): string {
  if (providerKind === "openrouter") {
    return "OpenRouter is a cloud routing provider. ostudy uses its fixed official API endpoint, requests only parameter-compatible endpoints for structured generation, and denies data-collecting endpoints by default.";
  }
  if (providerKind === "deepseek") {
    return "DeepSeek is a cloud provider. ostudy uses DeepSeek's fixed official API endpoint.";
  }
  return "This cloud provider uses its fixed official API endpoint.";
}

"""
    replace_once(settings, "function scopeLabel(", helper + "function scopeLabel(")
    replace_once(
        settings,
        """      {profile.providerKind === "deepseek" ? (
""",
        """      {profile.providerKind === "openrouter" ? (
        <p className={aiStyles.capabilityNote}>
          OpenRouter profiles use strict JSON Schema requests with parameter-compatible routing. ostudy also requests endpoints that deny provider data collection and never enables OpenRouter's upstream-body debug echo.
        </p>
      ) : null}
      {profile.providerKind === "deepseek" ? (
""",
    )


def patch_diagnostics_and_composition() -> None:
    diagnostics_ts = "src/ipc/diagnostics.ts"
    replace_once(
        diagnostics_ts,
        """function normalizedDiagnostic(event: FrontendDiagnosticEvent): FrontendDiagnosticEvent {
  return {
    level: event.level,
    target: normalizedTarget(event.target),
    event: normalizedEvent(event.event),
    message: boundedText(event.message, MAX_MESSAGE_CHARS),
    stack: event.stack ? boundedText(event.stack, MAX_STACK_CHARS) : undefined,
    fields: sanitizeFields(event.fields),
  };
}
""",
        """function normalizedDiagnostic(event: FrontendDiagnosticEvent): FrontendDiagnosticEvent {
  const fields = sanitizeFields(event.fields);
  return {
    level: event.level,
    target: normalizedTarget(event.target),
    event: normalizedEvent(event.event),
    message: boundedText(event.message, MAX_MESSAGE_CHARS),
    ...(event.stack ? { stack: boundedText(event.stack, MAX_STACK_CHARS) } : {}),
    ...(fields ? { fields } : {}),
  };
}
""",
    )
    replace_once(
        diagnostics_ts,
        """      input: {
        event: normalizedEvent(event),
        message: boundedText(error.message, MAX_MESSAGE_CHARS),
        stack: stack ? boundedText(stack, MAX_STACK_CHARS) : undefined,
      },
""",
        """      input: {
        event: normalizedEvent(event),
        message: boundedText(error.message, MAX_MESSAGE_CHARS),
        ...(stack ? { stack: boundedText(stack, MAX_STACK_CHARS) } : {}),
      },
""",
    )

    lib = "src-tauri/src/lib.rs"
    replace_once(lib, "pub fn run() {\n", "pub fn run() {\n    logging::install_panic_hook();\n")
    replace_once(
        lib,
        """    builder
        .plugin(tauri_plugin_dialog::init())
""",
        """    let result = builder
        .plugin(tauri_plugin_dialog::init())
""",
    )
    replace_once(
        lib,
        """            logging::info(
                "application_started",
                format!(
                    "data_directory={} database={} diagnostic_log={} asset_reconciliation=complete document_reconciliation=complete ai_gateway=ready pdf_extraction={} pdf_renderer={}",
                    paths.app_data_dir().display(),
                    paths.database_path().display(),
                    logging::diagnostic_log_path(paths.app_data_dir()).display(),
                    pdf_backend::active_backend_id(),
                    pdf_render_backend::active_backend_id()
                ),
            );
""",
        """            logging::info_target(
                "app.startup",
                "application_started",
                "ostudy completed durable startup and reconciliation.",
                serde_json::json!({
                    "databaseFile": "library.sqlite3",
                    "assetReconciliation": "complete",
                    "documentReconciliation": "complete",
                    "aiGateway": "ready",
                    "pdfExtraction": pdf_backend::active_backend_id(),
                    "pdfRenderer": pdf_render_backend::active_backend_id(),
                }),
            );
""",
    )
    replace_once(
        lib,
        """            diagnostics::record_webview_diagnostic,
            settings::get_settings,
""",
        """            diagnostics::record_webview_diagnostic,
            diagnostics::record_webview_diagnostic_batch,
            diagnostics::get_diagnostic_configuration,
            diagnostics::set_diagnostic_filter,
            diagnostics::flush_diagnostics,
            settings::get_settings,
""",
    )
    replace_once(
        lib,
        """        .run(tauri::generate_context!())
        .expect("error while running ostudy");
}
""",
        """        .run(tauri::generate_context!());
    logging::shutdown();
    result.expect("error while running ostudy");
}
""",
    )

    android_test = "scripts/android-test.sh"
    replace_once(
        android_test,
        """source "$SCRIPT_DIR/android-env.sh"

RUN_EMULATOR=0
case "${1:-}" in
  "") ;;
  --emulator) RUN_EMULATOR=1 ;;
  -h|--help)
    printf 'Usage: ./scripts/android-test.sh [--emulator]\\n'
    exit 0
    ;;
  *) printf 'Unknown option: %s\\n' "$1" >&2; exit 2 ;;
esac
""",
        """source "$SCRIPT_DIR/android-env.sh"
# shellcheck disable=SC1091
source "$SCRIPT_DIR/lib/diagnostics.sh"
export OSTUDY_ROOT_DIR="$ROOT_DIR"
ostudy_parse_diagnostic_args "$@"

RUN_EMULATOR=0
for argument in "${OSTUDY_DIAGNOSTIC_REMAINDER[@]}"; do
  case "$argument" in
    --emulator) RUN_EMULATOR=1 ;;
    -h|--help)
      printf 'Usage: ./scripts/android-test.sh [--emulator] [diagnostic options]\\n'
      ostudy_diagnostic_usage
      exit 0
      ;;
    *) printf 'Unknown option: %s\\n' "$argument" >&2; exit 2 ;;
  esac
done
""",
    )
    replace_once(
        android_test,
        """node ./scripts/verify-android-plugin-routing.mjs
node ./scripts/verify-local-fonts.mjs
""",
        """node ./scripts/verify-android-plugin-routing.mjs
node ./scripts/verify-android-qr-scanner.mjs
node ./scripts/verify-local-fonts.mjs
""",
    )
    replace_once(
        android_test,
        """  if ! grep -Eq 'kotlin-stdlib:1\\.9\\.25' "$dependency_report"; then
    printf 'Android dependency resolution did not select the pinned Kotlin stdlib 1.9.25 line.\\n' >&2
    exit 1
  fi
  printf 'OK   Android Kotlin metadata dependency graph resolves to Kotlin 1.9.25 and coroutines 1.8.1.\\n'
""",
        """  if ! grep -Eq 'kotlin-stdlib:1\\.9\\.25' "$dependency_report"; then
    printf 'Android dependency resolution did not select the pinned Kotlin stdlib 1.9.25 line.\\n' >&2
    exit 1
  fi
  if ! grep -Eq 'play-services-code-scanner:16\\.1\\.0' "$dependency_report"; then
    printf 'Android dependency resolution did not include Google Code Scanner 16.1.0.\\n' >&2
    exit 1
  fi
  if grep -Eqi 'journeyapps|zxing-android-embedded' "$dependency_report"; then
    printf 'The removed ZXing Embedded scanner remains in the Android dependency graph.\\n' >&2
    exit 1
  fi
  printf 'OK   Android Kotlin, coroutines, and Google Code Scanner dependency contracts passed.\\n'
""",
    )

    check_fast = "scripts/check-fast.sh"
    replace_once(
        check_fast,
        """  bash "$SCRIPT_DIR/check-static.sh" ${OSTUDY_DIAGNOSTICS_ACTIVE:+--log-filter="$OSTUDY_LOG"}
""",
        """  STATIC_ARGS=()
  if [[ "${OSTUDY_DIAGNOSTICS_ACTIVE:-0}" == 1 ]]; then
    STATIC_ARGS+=("--log-filter=$OSTUDY_LOG")
  fi
  if [[ "${OSTUDY_KEEP_FAILURE_ARTIFACTS:-0}" == 1 ]]; then
    STATIC_ARGS+=(--keep-artifacts)
  fi
  bash "$SCRIPT_DIR/check-static.sh" "${STATIC_ARGS[@]}"
""",
    )


def quote_sql(value: Any) -> str:
    if value is None:
        return "NULL"
    if isinstance(value, bool):
        return "1" if value else "0"
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        return repr(value)
    if isinstance(value, bytes):
        return "X'" + value.hex() + "'"
    return "'" + str(value).replace("'", "''") + "'"


def apply_legacy_migrations(database_path: Path, migrations: list[Path]) -> sqlite3.Connection:
    connection = sqlite3.connect(database_path)
    connection.isolation_level = None
    connection.execute("PRAGMA foreign_keys = ON")
    for migration in migrations:
        version = int(migration.name[:4])
        if version in {13, 20}:
            connection.execute("PRAGMA foreign_keys = OFF")
        connection.executescript(migration.read_text(encoding="utf-8"))
        if version in {13, 20}:
            violations = connection.execute("PRAGMA foreign_key_check").fetchall()
            if violations:
                raise RuntimeError(f"legacy migration {version} produced foreign-key violations")
            connection.execute("PRAGMA foreign_keys = ON")
    return connection


def canonical_schema(connection: sqlite3.Connection) -> tuple[list[dict[str, str]], set[str]]:
    rows = connection.execute(
        "SELECT type, name, sql FROM sqlite_schema "
        "WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' "
        "ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'view' THEN 1 "
        "WHEN 'index' THEN 2 WHEN 'trigger' THEN 3 ELSE 4 END, name"
    ).fetchall()
    virtual_roots = {
        name
        for object_type, name, sql in rows
        if object_type == "table" and sql.lstrip().upper().startswith("CREATE VIRTUAL TABLE")
    }
    shadow_tables: set[str] = set()
    for root in virtual_roots:
        shadow_tables.update(
            name
            for object_type, name, _ in rows
            if object_type == "table" and name.startswith(f"{root}_")
        )
    objects = [
        {"type": object_type, "name": name, "sql": sql.strip().rstrip(";")}
        for object_type, name, sql in rows
        if name not in shadow_tables
    ]
    return objects, shadow_tables


def seed_statements(
    connection: sqlite3.Connection, objects: list[dict[str, str]], shadow_tables: set[str]
) -> list[str]:
    virtual_roots = {
        item["name"]
        for item in objects
        if item["type"] == "table"
        and item["sql"].lstrip().upper().startswith("CREATE VIRTUAL TABLE")
    }
    statements: list[str] = []
    for item in objects:
        if item["type"] != "table":
            continue
        table = item["name"]
        if table in virtual_roots or table in shadow_tables:
            continue
        columns = [
            row[1]
            for row in connection.execute(f'PRAGMA table_info("{table}")').fetchall()
        ]
        if not columns:
            continue
        rows = connection.execute(f'SELECT * FROM "{table}"').fetchall()
        for row in rows:
            column_sql = ", ".join(f'"{column}"' for column in columns)
            value_sql = ", ".join(quote_sql(value) for value in row)
            statements.append(f'INSERT INTO "{table}" ({column_sql}) VALUES ({value_sql});')
    return statements


def generate_public_baseline() -> tuple[str, str, dict[str, Any]]:
    migration_dir = ROOT / "src-tauri/migrations"
    migrations = sorted(migration_dir.glob("[0-9][0-9][0-9][0-9]_*.sql"))
    if len(migrations) < 26:
        raise RuntimeError("The complete pre-public migration chain is required to generate the baseline")
    with tempfile.TemporaryDirectory(prefix="ostudy-public-baseline-") as directory:
        legacy_path = Path(directory) / "legacy.sqlite3"
        legacy = apply_legacy_migrations(legacy_path, migrations)
        objects, shadow_tables = canonical_schema(legacy)
        seeds = seed_statements(legacy, objects, shadow_tables)
        legacy.close()

        metadata_sql = """CREATE TABLE schema_metadata (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    checksum TEXT NOT NULL,
    applied_at INTEGER NOT NULL
) STRICT;"""
        schema_sql = [
            "-- ostudy public schema baseline.\n"
            "-- Generated from the final pre-release schema and then curated as the\n"
            "-- immutable starting point for all public installations. Historical\n"
            "-- migration steps remain available through Git history and the\n"
            "-- pre-public schema evolution document.\n",
            "PRAGMA foreign_keys = ON;\n",
            metadata_sql,
        ]
        for item in objects:
            schema_sql.append(item["sql"] + ";")
        schema_sql.extend(seeds)
        schema_sql.append(
            "INSERT INTO schema_metadata (key, value) VALUES "
            "('schema_lineage', 'ostudy-public-v1');"
        )
        baseline = "\n\n".join(schema_sql).rstrip() + "\n"
        checksum = hashlib.sha256(baseline.encode("utf-8")).hexdigest()

        verify_path = Path(directory) / "verify.sqlite3"
        verify = sqlite3.connect(verify_path)
        verify.executescript(baseline)
        violations = verify.execute("PRAGMA foreign_key_check").fetchall()
        if violations:
            raise RuntimeError(f"public baseline produced foreign-key violations: {violations[:3]}")
        integrity = verify.execute("PRAGMA integrity_check").fetchone()[0]
        if integrity != "ok":
            raise RuntimeError(f"public baseline failed integrity_check: {integrity}")
        verified_objects, _ = canonical_schema(verify)
        manifest = {
            "formatVersion": 1,
            "schemaLineage": "ostudy-public-v1",
            "schemaVersion": 1,
            "baselineSha256": checksum,
            "objects": verified_objects,
        }
        verify.close()
        return baseline, checksum, manifest


def database_source(checksum: str) -> str:
    return f'''use std::{{path::Path, sync::Mutex, time::{{Duration, Instant}}}};

use rusqlite::{{params, Connection, OptionalExtension, TransactionBehavior}};
use serde::Serialize;
use serde_json::json;

use crate::{{
    data_repairs::{{apply_required_repairs, verify_repair_ledger, RepairPhase}},
    error::CommandError,
    logging::LogLevel,
    util::unix_time_millis,
}};

const PUBLIC_SCHEMA_LINEAGE: &str = "ostudy-public-v1";
const PRE_PUBLIC_RESET_PREFIX: &str = "PRE_PUBLIC_DATABASE_REQUIRES_RESET";

struct Migration {{
    version: i64,
    name: &'static str,
    checksum: &'static str,
    sql: &'static str,
}}

const MIGRATIONS: &[Migration] = &[Migration {{
    version: 1,
    name: "public_baseline",
    checksum: "{checksum}",
    sql: include_str!("../migrations/0001_public_baseline.sql"),
}}];

pub(crate) const LATEST_SCHEMA_VERSION: i64 = 1;

pub struct Database {{
    connection: Mutex<Connection>,
}}

impl Database {{
    pub fn open(path: &Path) -> Result<Self, String> {{
        crate::logging::debug_target(
            "database.connection",
            "database_open_started",
            "The local SQLite database is being opened.",
            json!({{ "fileName": path.file_name().and_then(|value| value.to_str()).unwrap_or("library.sqlite3") }}),
        );
        let mut connection = Connection::open(path)
            .map_err(|error| format!("Could not open the local SQLite database: {{error}}"))?;
        initialize_connection(&mut connection)?;
        Ok(Self {{
            connection: Mutex::new(connection),
        }})
    }}

    pub(crate) fn with_connection<T>(
        &self,
        operation: impl FnOnce(&mut Connection) -> Result<T, CommandError>,
    ) -> Result<T, CommandError> {{
        let mut connection = self.connection.lock().map_err(|_| {{
            CommandError::new(
                "database_lock_poisoned",
                "The local database lock could not be acquired. Restart ostudy.",
            )
        }})?;
        operation(&mut connection)
    }}

    #[cfg(test)]
    pub(crate) fn open_in_memory() -> Result<Self, String> {{
        let mut connection = Connection::open_in_memory()
            .map_err(|error| format!("Could not open an in-memory database: {{error}}"))?;
        initialize_connection(&mut connection)?;
        Ok(Self {{
            connection: Mutex::new(connection),
        }})
    }}
}}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DatabaseLineage {{
    Empty,
    Public,
    PrePublic,
    Unknown(String),
}}

fn initialize_connection(connection: &mut Connection) -> Result<(), String> {{
    configure_connection(connection)?;
    let lineage = inspect_lineage(connection)?;
    match lineage {{
        DatabaseLineage::Empty | DatabaseLineage::Public => {{}}
        DatabaseLineage::PrePublic => {{
            return Err(format!(
                "{{PRE_PUBLIC_RESET_PREFIX}}: This database predates ostudy's public schema baseline. Stop ostudy, preserve a backup if needed, and run scripts/reset-pre-public-data.sh with the exact application data directory. The database was not modified."
            ));
        }}
        DatabaseLineage::Unknown(value) => {{
            return Err(format!(
                "This database belongs to an unsupported schema lineage ({{value}}). The database was not modified."
            ));
        }}
    }}

    let started = Instant::now();
    let migrated = apply_migrations(connection, matches!(lineage, DatabaseLineage::Empty))?;
    apply_required_repairs(connection, RepairPhase::AfterMigration(LATEST_SCHEMA_VERSION))?;
    verify_repair_ledger(connection)?;
    if migrated || crate::logging::enabled(LogLevel::Debug, "ostudy.database.integrity") {{
        verify_integrity(connection)?;
    }}
    crate::logging::info_target(
        "database.migration",
        "database_schema_ready",
        "The public database schema is ready.",
        json!({{
            "schemaLineage": PUBLIC_SCHEMA_LINEAGE,
            "schemaVersion": LATEST_SCHEMA_VERSION,
            "migrationApplied": migrated,
            "durationMs": i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
        }}),
    );
    Ok(())
}}

fn configure_connection(connection: &Connection) -> Result<(), String> {{
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("Could not configure SQLite busy timeout: {{error}}"))?;
    connection
        .execute_batch(
            "PRAGMA foreign_keys = ON;\n\
             PRAGMA journal_mode = WAL;\n\
             PRAGMA synchronous = FULL;",
        )
        .map_err(|error| format!("Could not configure SQLite durability settings: {{error}}"))
}}

fn inspect_lineage(connection: &Connection) -> Result<DatabaseLineage, String> {{
    let metadata_exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'schema_metadata')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect the schema lineage table: {{error}}"))?;
    if metadata_exists != 0 {{
        let value: Option<String> = connection
            .query_row(
                "SELECT value FROM schema_metadata WHERE key = 'schema_lineage'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("Could not read the schema lineage: {{error}}"))?;
        return Ok(match value {{
            Some(value) if value == PUBLIC_SCHEMA_LINEAGE => DatabaseLineage::Public,
            Some(value) => DatabaseLineage::Unknown(value),
            None => DatabaseLineage::Unknown("missing".to_string()),
        }});
    }}

    let object_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect database contents: {{error}}"))?;
    if object_count == 0 {{
        Ok(DatabaseLineage::Empty)
    }} else {{
        Ok(DatabaseLineage::PrePublic)
    }}
}}

fn apply_migrations(connection: &mut Connection, empty_database: bool) -> Result<bool, String> {{
    let mut applied = false;
    if !empty_database {{
        verify_public_ledger_exists(connection)?;
    }}
    let newest_version = if empty_database {{
        0
    }} else {{
        connection
            .query_row("SELECT COALESCE(MAX(version), 0) FROM schema_migrations", [], |row| row.get(0))
            .map_err(|error| format!("Could not read the database schema version: {{error}}"))?
    }};
    if newest_version > LATEST_SCHEMA_VERSION {{
        return Err(format!(
            "This database was created by a newer ostudy version (public schema {{newest_version}})."
        ));
    }}

    for migration in MIGRATIONS {{
        let existing: Option<(String, String)> = if empty_database && migration.version == 1 {{
            None
        }} else {{
            connection
                .query_row(
                    "SELECT name, checksum FROM schema_migrations WHERE version = ?1",
                    [migration.version],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(|error| format!("Could not inspect migration {{}}: {{error}}", migration.version))?
        }};
        if let Some((name, checksum)) = existing {{
            if name != migration.name || checksum != migration.checksum {{
                return Err(format!(
                    "Public migration {{}} does not match this application build. The database was not modified.",
                    migration.version
                ));
            }}
            continue;
        }}
        apply_transactional_migration(connection, migration)?;
        applied = true;
    }}
    Ok(applied)
}}

fn verify_public_ledger_exists(connection: &Connection) -> Result<(), String> {{
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'schema_migrations')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect the public migration ledger: {{error}}"))?;
    if exists == 0 {{
        return Err("The public schema lineage is present but its migration ledger is missing.".to_string());
    }}
    Ok(())
}}

fn apply_transactional_migration(
    connection: &mut Connection,
    migration: &Migration,
) -> Result<(), String> {{
    let started = Instant::now();
    crate::logging::info_target(
        "database.migration",
        "database_migration_started",
        "A public database migration started.",
        json!({{ "version": migration.version, "name": migration.name }}),
    );
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Could not begin database migration {{}}: {{error}}", migration.version))?;
    transaction
        .execute_batch(migration.sql)
        .map_err(|error| format!("Could not apply migration {{}}: {{error}}", migration.version))?;
    let applied_at = unix_time_millis().map_err(|error| error.message)?;
    transaction
        .execute(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
            params![migration.version, migration.name, migration.checksum, applied_at],
        )
        .map_err(|error| format!("Could not record migration {{}}: {{error}}", migration.version))?;
    transaction
        .execute_batch(&format!("PRAGMA user_version = {{}};", migration.version))
        .map_err(|error| format!("Could not set SQLite user_version: {{error}}"))?;
    transaction
        .commit()
        .map_err(|error| format!("Could not commit migration {{}}: {{error}}", migration.version))?;
    crate::logging::info_target(
        "database.migration",
        "database_migration_completed",
        "A public database migration completed.",
        json!({{
            "version": migration.version,
            "name": migration.name,
            "durationMs": i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX),
        }}),
    );
    Ok(())
}}

fn verify_integrity(connection: &Connection) -> Result<(), String> {{
    let foreign_key_violation: Option<(String, i64)> = connection
        .query_row(
            "SELECT \"table\", rowid FROM pragma_foreign_key_check LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("Could not run SQLite foreign-key validation: {{error}}"))?;
    if let Some((table, rowid)) = foreign_key_violation {{
        return Err(format!(
            "The migrated database contains an invalid foreign-key reference in table {{table}} at row {{rowid}}."
        ));
    }}
    let result: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(|error| format!("Could not run SQLite integrity_check: {{error}}"))?;
    if result != "ok" {{
        return Err(format!("SQLite integrity_check failed: {{result}}"));
    }}
    Ok(())
}}

#[cfg(test)]
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct SchemaObject {{
    object_type: String,
    name: String,
    sql: String,
}}

#[cfg(test)]
fn schema_objects(connection: &Connection) -> Result<Vec<SchemaObject>, rusqlite::Error> {{
    let mut statement = connection.prepare(
        "SELECT type, name, RTRIM(sql, ';') FROM sqlite_schema
         WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'
           AND name NOT GLOB 'cards_fts_*'
         ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'view' THEN 1
                  WHEN 'index' THEN 2 WHEN 'trigger' THEN 3 ELSE 4 END,
                  name",
    )?;
    statement
        .query_map([], |row| {{
            Ok(SchemaObject {{
                object_type: row.get(0)?,
                name: row.get(1)?,
                sql: row.get::<_, String>(2)?.trim().to_string(),
            }})
        }})?
        .collect()
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn bundled_sqlite_version_is_the_audited_release() {{
        assert_eq!(rusqlite::version(), "3.53.2");
    }}

    #[test]
    fn public_baseline_is_idempotent_and_matches_manifest() {{
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {{
                let before = connection.total_changes();
                initialize_connection(connection)
                    .map_err(|error| CommandError::new("migration_error", error))?;
                assert_eq!(before, connection.total_changes());
                let version: i64 = connection
                    .query_row("PRAGMA user_version", [], |row| row.get(0))
                    .map_err(CommandError::database)?;
                assert_eq!(version, LATEST_SCHEMA_VERSION);
                let lineage: String = connection
                    .query_row(
                        "SELECT value FROM schema_metadata WHERE key = 'schema_lineage'",
                        [],
                        |row| row.get(0),
                    )
                    .map_err(CommandError::database)?;
                assert_eq!(lineage, PUBLIC_SCHEMA_LINEAGE);
                let manifest: serde_json::Value = serde_json::from_str(include_str!(
                    "../tests/fixtures/public-baseline-schema.json"
                ))
                .map_err(CommandError::serialization)?;
                let expected = manifest
                    .get("objects")
                    .cloned()
                    .ok_or_else(|| CommandError::new("fixture_error", "Missing schema objects."))?;
                let actual = serde_json::to_value(
                    schema_objects(connection).map_err(CommandError::database)?,
                )
                .map_err(CommandError::serialization)?;
                assert_eq!(actual, expected);
                Ok(())
            }})
            .expect("public schema should be stable");
    }}

    #[test]
    fn pre_public_database_requires_explicit_non_destructive_reset() {{
        let mut connection = Connection::open_in_memory().expect("database should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL) STRICT;
                 INSERT INTO schema_migrations VALUES (26, 'deck_folders', 'legacy', 0);
                 CREATE TABLE decks (id TEXT PRIMARY KEY) STRICT;",
            )
            .expect("legacy fixture should load");
        let error = initialize_connection(&mut connection).expect_err("legacy database must stop");
        assert!(error.starts_with(PRE_PUBLIC_RESET_PREFIX));
        let decks_exist: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = 'decks')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(decks_exist, 1, "legacy data must not be silently deleted");
    }}

    #[test]
    fn public_migration_checksum_and_future_version_are_enforced() {{
        let mut connection = Connection::open_in_memory().expect("database should open");
        initialize_connection(&mut connection).expect("baseline should apply");
        connection
            .execute(
                "UPDATE schema_migrations SET checksum = 'modified' WHERE version = 1",
                [],
            )
            .unwrap();
        assert!(initialize_connection(&mut connection).is_err());
        connection
            .execute(
                "UPDATE schema_migrations SET checksum = ?1 WHERE version = 1",
                [MIGRATIONS[0].checksum],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations (version, name, checksum, applied_at) VALUES (2, 'future', 'future', 0)",
                [],
            )
            .unwrap();
        assert!(initialize_connection(&mut connection).is_err());
    }}

    #[test]
    fn openrouter_is_a_valid_extensible_provider_key() {{
        let database = Database::open_in_memory().expect("database should open");
        database
            .with_connection(|connection| {{
                connection
                    .execute(
                        "INSERT INTO ai_provider_profiles (
                             id, display_name, normalized_name, provider_kind, base_url,
                             model_id, auth_mode, enabled, allow_insecure_http,
                             request_timeout_ms, credential_storage, credential_epoch,
                             revision, created_at, updated_at
                         ) VALUES (?1, 'OpenRouter', 'openrouter', 'openrouter',
                                   'https://openrouter.ai/api/v1/', 'openai/example',
                                   'bearer', 1, 0, 60000, 'session', 0, 1, 0, 0)",
                        ["a".repeat(32)],
                    )
                    .map_err(CommandError::database)?;
                Ok(())
            }})
            .expect("OpenRouter profile should satisfy the public baseline");
    }}
}}
'''


def data_repairs_source() -> str:
    return '''//! Versioned public data repairs with explicit migration prerequisites.
//!
//! The pre-release repair implementations were folded into the public baseline.
//! Future repairs remain independently versioned, checksum-protected, and
//! transactional rather than being hidden inside schema SQL.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::util::unix_time_millis;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepairPhase {
    AfterMigration(i64),
}

struct DataRepair {
    version: i64,
    name: &'static str,
    checksum: &'static str,
    phase: RepairPhase,
    apply: fn(&Transaction<'_>) -> Result<i64, String>,
}

const REPAIRS: &[DataRepair] = &[];

pub(crate) fn apply_required_repairs(
    connection: &mut Connection,
    phase: RepairPhase,
) -> Result<(), String> {
    ensure_ledger_exists(connection)?;
    for repair in REPAIRS.iter().filter(|repair| repair.phase == phase) {
        let existing = recorded_repair(connection, repair.version)?;
        if let Some((name, checksum)) = existing {
            if name != repair.name || checksum != repair.checksum {
                return Err(format!(
                    "Data repair {} does not match this application build. The database was not modified.",
                    repair.version
                ));
            }
            continue;
        }
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| format!("Could not start data repair {}: {error}", repair.version))?;
        let changed_rows = (repair.apply)(&transaction)?;
        let applied_at = unix_time_millis().map_err(|error| error.message)?;
        transaction
            .execute(
                "INSERT INTO data_repairs (version, name, checksum, applied_at, changed_rows)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![repair.version, repair.name, repair.checksum, applied_at, changed_rows.max(0)],
            )
            .map_err(|error| format!("Could not record data repair {}: {error}", repair.version))?;
        transaction
            .commit()
            .map_err(|error| format!("Could not commit data repair {}: {error}", repair.version))?;
    }
    Ok(())
}

pub(crate) fn verify_repair_ledger(connection: &Connection) -> Result<(), String> {
    ensure_ledger_exists(connection)?;
    let mut statement = connection
        .prepare("SELECT version, name, checksum FROM data_repairs ORDER BY version")
        .map_err(|error| format!("Could not inspect the data repair ledger: {error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| format!("Could not read the data repair ledger: {error}"))?;
    for row in rows {
        let (version, name, checksum) =
            row.map_err(|error| format!("Could not decode a data repair row: {error}"))?;
        let expected = REPAIRS.iter().find(|repair| repair.version == version);
        if expected
            .map(|repair| (repair.name, repair.checksum))
            != Some((name.as_str(), checksum.as_str()))
        {
            return Err(format!(
                "Data repair {version} is not recognized by this application build."
            ));
        }
    }
    Ok(())
}

fn recorded_repair(
    connection: &Connection,
    version: i64,
) -> Result<Option<(String, String)>, String> {
    connection
        .query_row(
            "SELECT name, checksum FROM data_repairs WHERE version = ?1",
            [version],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("Could not inspect data repair {version}: {error}"))
}

fn ensure_ledger_exists(connection: &Connection) -> Result<(), String> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = 'data_repairs')",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Could not inspect the data repair ledger: {error}"))?;
    if exists == 0 {
        return Err("The data repair ledger is missing from the public schema baseline.".to_string());
    }
    Ok(())
}
'''


def reset_script() -> str:
    return '''#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
  cat <<'EOF'
Usage: ./scripts/reset-pre-public-data.sh --data-dir <directory> --confirm

Moves the entire pre-release ostudy application-data directory to a timestamped
sibling backup and recreates an empty directory. It never deletes the old data.
Stop ostudy before running this command.
EOF
}

DATA_DIR=""
CONFIRMED=0
while (( $# > 0 )); do
  case "$1" in
    --data-dir) DATA_DIR="${2:-}"; shift ;;
    --confirm) CONFIRMED=1 ;;
    -h|--help) usage; exit 0 ;;
    *) printf 'Unknown option: %s\n' "$1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done

[[ -n "$DATA_DIR" && "$DATA_DIR" = /* ]] || {
  printf 'An absolute --data-dir path is required.\n' >&2
  exit 2
}
(( CONFIRMED == 1 )) || {
  printf 'Pass --confirm after reviewing the data directory.\n' >&2
  exit 2
}
if pgrep -x ostudy >/dev/null 2>&1; then
  printf 'ostudy is still running. Close it before resetting pre-release data.\n' >&2
  exit 1
fi
if [[ ! -d "$DATA_DIR" ]]; then
  mkdir -p "$DATA_DIR"
  printf 'Created empty ostudy data directory: %s\n' "$DATA_DIR"
  exit 0
fi

parent="$(dirname -- "$DATA_DIR")"
name="$(basename -- "$DATA_DIR")"
timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
backup="$parent/$name.pre-public-$timestamp"
[[ ! -e "$backup" ]] || {
  printf 'Backup destination already exists: %s\n' "$backup" >&2
  exit 1
}
mv -- "$DATA_DIR" "$backup"
mkdir -p -- "$DATA_DIR"
printf 'Moved pre-release ostudy data to: %s\n' "$backup"
printf 'Created empty public-baseline data directory: %s\n' "$DATA_DIR"
'''


def patch_migration_tooling() -> None:
    baseline, checksum, manifest = generate_public_baseline()
    migration_dir = ROOT / "src-tauri/migrations"
    for path in migration_dir.glob("[0-9][0-9][0-9][0-9]_*.sql"):
        path.unlink()
    write("src-tauri/migrations/0001_public_baseline.sql", baseline)
    write("src-tauri/src/database.rs", database_source(checksum))
    write("src-tauri/src/data_repairs.rs", data_repairs_source())
    write(
        "src-tauri/tests/fixtures/public-baseline-schema.json",
        json.dumps(manifest, indent=2, sort_keys=False) + "\n",
    )
    legacy_fixture = ROOT / "src-tauri/tests/fixtures/v0.1.0.sql"
    if legacy_fixture.exists():
        legacy_fixture.unlink()
    write("scripts/reset-pre-public-data.sh", reset_script(), executable=True)

    check_fast = "scripts/check-fast.sh"
    replace_once(
        check_fast,
        "database::tests::migrations_and_repairs_are_idempotent_and_reach_latest_schema",
        "database::tests::public_baseline_is_idempotent_and_matches_manifest",
    )
    replace_once(
        check_fast,
        "database::tests::released_schema_upgrades_forward_without_losing_integrity",
        "database::tests::public_migration_checksum_and_future_version_are_enforced",
    )
    replace_once(check_fast, 'step "Forward-upgrade integrity contract"', 'step "Public migration immutability contract"')

    manifests = "scripts/verify-project-manifests.mjs"
    replace_once(
        manifests,
        """const releaseFixture = await read("src-tauri", "tests", "fixtures", "v0.1.0.sql");
requireText("0.1.0 release fixture", releaseFixture, [
  "Immutable database fixture captured for the ostudy 0.1.0 release",
  "CREATE TABLE schema_migrations",
  "PRAGMA user_version = 3",
]);
""",
        """const publicBaselineManifest = JSON.parse(
  await read("src-tauri", "tests", "fixtures", "public-baseline-schema.json"),
);
equal("public schema lineage", publicBaselineManifest.schemaLineage, "ostudy-public-v1");
equal("public schema fixture version", publicBaselineManifest.schemaVersion, 1);
equal(
  "public baseline fixture checksum",
  publicBaselineManifest.baselineSha256,
  registry.get(1)?.checksum,
);
""",
    )
    replace_once(
        manifests,
        '  "src-tauri/tests/fixtures/v0.1.0.sql",',
        '  "src-tauri/tests/fixtures/public-baseline-schema.json",\n  "scripts/reset-pre-public-data.sh",',
    )


def patch_docs() -> None:
    write(
        "docs/history/pre-public-schema-evolution.md",
        '''# Pre-public database schema evolution

Before ostudy's first public release, the local schema evolved through 26 development migrations. No public installation depended on those intermediate states, so PR #54 folded their final result into `0001_public_baseline.sql` and began the supported migration lineage at `ostudy-public-v1`.

The original SQL remains permanently recoverable from Git history immediately before this baseline commit. It is not copied into a runtime archive because duplicate migration trees invite accidental registration and maintenance drift.

## Durable architectural eras

| Era | Durable result preserved in the public baseline |
| --- | --- |
| Initial study library | Decks, cards, settings, practice sessions, and managed images |
| Scheduling and review history | FSRS schedules, immutable review events, workload controls, suspend/bury/reset semantics |
| Organization and authoring | Tags, search/FTS, card types, cloze cards, rich templates, duplicate fingerprints |
| Transaction boundaries | Operation identities, idempotent writes, transactional schedule actions, deletion undo |
| Canonical ownership and analytics | Card-owned review history, stable review attribution, bounded projections |
| AI provider platform | Provider profiles, OS-backed credentials, model discovery, capability probes |
| Document intelligence | Managed sources, OCR metadata, semantic windows, resumable generation, usage calibration |
| Platform storage | Opaque external transfers, durable editor drafts, deck folders, library ordering |

## Intentionally retired transition mechanisms

- table-rebuild and backfill steps whose result is already represented by the baseline;
- pre-release migration-number special cases;
- historical data repairs for synthetic development data;
- compatibility with developer-only schema versions 1 through 26.

A database without the `ostudy-public-v1` lineage marker is never modified or deleted. Startup returns an explicit pre-public-reset error, and `scripts/reset-pre-public-data.sh` moves the complete old data directory to a timestamped sibling backup before creating an empty directory.

## Policy after public release

Every public migration is immutable, checksum-protected, append-only, and tested from the immediately previous public schema. Data repairs remain independently versioned and transactional.
''',
    )
    write(
        "docs/decisions/0035-structured-local-diagnostics.md",
        '''# ADR 0035: Structured local diagnostics

## Decision

ostudy uses a bounded, local-only JSONL diagnostics pipeline with stable event names, hierarchical subsystem targets, operation-safe metadata, dynamic filters, and a non-blocking writer queue. Normal releases persist sparse information/warning/error events; debug and trace output is enabled explicitly with `OSTUDY_LOG`, script flags, or a temporary in-app session.

Diagnostics never automatically upload data. Credentials, Authorization headers, prompts, provider responses, document/OCR text, card content, and QR payloads are prohibited fields and are centrally redacted. Current and previous files are each capped at 2 MiB. Queue saturation drops diagnostics rather than blocking study/database operations and records a bounded drop summary.

## Consequences

- Rust, webview, script, and Android boundaries use one event taxonomy.
- `--debug=db,ai,qr`, `--trace=<targets>`, and exact filters work across development and qualification scripts.
- Failure artifacts are bounded and intentionally omit the database and user content.
- Diagnostic logging is never a correctness or transaction boundary.
''',
    )
    write(
        "docs/decisions/0036-google-code-scanner.md",
        '''# ADR 0036: Google Code Scanner for Android pairing

## Decision

Android QR pairing uses Google Code Scanner (`play-services-code-scanner:16.1.0`) through the private ostudy platform plugin. The scanner is restricted to QR codes, enables first-party auto zoom, requests the `barcode_ui` module at install time, and delegates camera permission/UI to Google Play services. ZXing Embedded and the application camera permission are removed.

The native scanner returns only a bounded decoded string. Rust retains pairing-envelope parsing, expiry/protocol checks, and cryptographic validation. Raw QR contents never enter logs. Atomic guards reject concurrent scans and ignore duplicate terminal callbacks.

## Consequences

Google Play services is required for this Android backend. Unsupported devices receive a typed scanner failure rather than a silent stale-library fallback. The Rust/frontend scanner contract remains provider-neutral so a bundled non-GMS backend can be added later without redesigning device sync.
''',
    )
    write(
        "docs/decisions/0037-openrouter-provider-adapter.md",
        '''# ADR 0037: First-class OpenRouter provider adapter

## Decision

OpenRouter is a fixed-endpoint cloud provider at `https://openrouter.ai/api/v1/`, authenticated with the existing OS-backed bearer-secret boundary. ostudy uses the account-filtered `/models/user` catalog with a bounded public-catalog fallback, and `/chat/completions` for probes and document generation.

Structured document requests use strict JSON Schema plus `provider.require_parameters=true`. The default routing policy sets `provider.data_collection=deny` and allows compatible endpoint fallback. ostudy does not request OpenRouter plugins/context compression and never enables the provider's upstream-body debug echo. Streaming top-level errors abort the window and discard partial output.

The HTTP gateway remains responsible for transport, redirects, bounds, cancellation, and safe errors. Provider adapters own endpoint paths, request bodies, model parsing, and provider-specific error normalization.
''',
    )
    write(
        "docs/decisions/0038-public-schema-baseline.md",
        '''# ADR 0038: Public database schema baseline

## Decision

The first supported public schema lineage is `ostudy-public-v1`, represented by the immutable migration `0001_public_baseline.sql`. The final pre-release schema was built from all development migrations, validated, converted into canonical DDL plus seed rows, and checked against a committed semantic schema manifest.

Databases with development-era objects but no public lineage marker are rejected without writes. The explicit reset helper moves the complete application-data directory to a timestamped backup. Future migrations begin at version 2 and are append-only and checksum-protected.

Historical repair results are folded into the baseline. The independent repair framework remains with an empty public registry, ready for future versioned repairs.
''',
    )
    write(
        "docs/development/diagnostics.md",
        '''# Diagnostics and debug filters

Normal application and test runs use the `info` filter with noisy dependency targets suppressed by the application taxonomy. Enable targeted diagnostics without shell command tracing:

```bash
./scripts/dev.sh --debug=database,ai
./run-app.sh --trace=qr
./scripts/check-fast.sh --debug=migrations --keep-artifacts
./scripts/check.sh --log-filter='warn,ostudy.database=debug,ostudy.ai.provider.openrouter=trace'
./scripts/android-test.sh --debug=qr --keep-artifacts
```

Equivalent environment configuration:

```bash
OSTUDY_LOG='warn,ostudy.documents=debug' ./scripts/dev.sh
```

Targets include `app`, `database`, `ai`, `qr`, `device_sync`, `documents`, `frontend`, `filesystem`, and `diagnostics`. Target rules use longest-prefix matching. Debug/trace fields must contain only counts, durations, safe identifiers, provider/model identifiers, and normalized outcomes—never credentials, prompts, study content, OCR text, SQL parameters, or QR payloads.

Application JSONL files are capped at two 2 MiB generations under the application diagnostics directory. Script failure artifacts contain only bounded log tails and build context under `.artifacts/diagnostics/`; they do not contain the SQLite database or managed user files.
''',
    )
    append_once(
        "docs/user-guide/ai-providers.md",
        "## OpenRouter",
        '''## OpenRouter

Create an **OpenRouter** profile in Settings → AI/LLM, save the API key in the operating-system credential store, retrieve the account-visible model catalog, select a model, and run the connection/capability test. ostudy uses OpenRouter's fixed official API endpoint.

For document generation, ostudy requests strict JSON Schema output only from parameter-compatible endpoints and sets OpenRouter's provider-data-collection policy to `deny`. It does not enable provider-side prompt/body debugging or context-compression plugins. A stricter policy can reduce endpoint availability; the connection test reports that as a routing/capability failure rather than silently weakening the request.
''',
    )
    append_once(
        "docs/user-guide/device-sync.md",
        "## Android QR scanner backend",
        '''## Android QR scanner backend

Android pairing uses Google Code Scanner. The scanner UI and camera permission are owned by Google Play services; ostudy itself does not request the camera permission. The scanner accepts QR codes only and returns the decoded pairing envelope to ostudy for protocol and cryptographic validation. Devices without a compatible Google Play services scanner module receive an unavailable-scanner message and can use the manual pairing path.
''',
    )
    append_once(
        "docs/development/android.md",
        "## QR scanner verification",
        '''## QR scanner verification

`node scripts/verify-android-qr-scanner.mjs` enforces the Google Code Scanner dependency, install-time `barcode_ui` metadata, QR-only/auto-zoom configuration, concurrent-scan guard, and absence of ZXing Embedded. APK verification rejects any merged `android.permission.CAMERA` permission. `./scripts/android-test.sh --debug=qr --keep-artifacts` retains bounded ostudy QR Logcat evidence on failure without recording decoded payloads.
''',
    )
    append_once(
        "docs/history/README.md",
        "pre-public-schema-evolution.md",
        "- [Pre-public schema evolution](pre-public-schema-evolution.md) — architectural history retained after establishing the public schema baseline.",
    )


def main() -> None:
    expected_branch = "agent/pre-public-platform-hardening"
    branch = subprocess.check_output(
        ["git", "branch", "--show-current"], cwd=ROOT, text=True
    ).strip()
    if branch != expected_branch:
        raise RuntimeError(f"This transformation may run only on {expected_branch}, found {branch}")
    patch_provider_runtime()
    patch_gateway()
    patch_frontend_ai()
    patch_diagnostics_and_composition()
    patch_migration_tooling()
    patch_docs()
    subprocess.run(["cargo", "fmt", "--manifest-path", "src-tauri/Cargo.toml", "--all"], cwd=ROOT, check=True)
    subprocess.run(["node", "scripts/verify-android-qr-scanner.mjs"], cwd=ROOT, check=True)
    print("Applied deterministic pre-public platform hardening transformations.")


if __name__ == "__main__":
    main()
