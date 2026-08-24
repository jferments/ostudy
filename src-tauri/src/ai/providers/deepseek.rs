use serde_json::{json, Value};
use url::Url;

use crate::ai::{
    endpoint_policy::join_endpoint,
    gateway::{error, AiGatewayError},
    profiles::ProfileSnapshot,
};

use super::schema::portable_strict_schema;

pub(super) const DOCUMENT_TOOL_NAME: &str = "submit_ostudy_document_result";

pub(super) fn completion_endpoint(
    profile: &ProfileSnapshot,
    strict_tool: bool,
) -> Result<Url, AiGatewayError> {
    let path = if strict_tool {
        "beta/chat/completions"
    } else {
        "chat/completions"
    };
    join_endpoint(&profile.base_url, path).map_err(command_to_gateway)
}

pub(super) fn disable_thinking(body: &mut Value) {
    body["thinking"] = json!({ "type": "disabled" });
}

pub(super) fn apply_strict_document_tool(body: &mut Value, schema: &Value) {
    let schema = portable_strict_schema(schema);
    body["tools"] = json!([{
        "type": "function",
        "function": {
            "name": DOCUMENT_TOOL_NAME,
            "description": "Submit the exact structured ostudy document-task result.",
            "strict": true,
            "parameters": schema,
        }
    }]);
    body["tool_choice"] = json!({
        "type": "function",
        "function": {"name": DOCUMENT_TOOL_NAME}
    });
}

fn command_to_gateway(error_value: crate::error::CommandError) -> AiGatewayError {
    error(&error_value.code, &error_value.message, false, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_document_requests_use_beta_tool_contract_and_portable_schema() {
        let profile = ProfileSnapshot {
            id: "a".repeat(32),
            provider_kind: crate::ai::catalog::ProviderKind::Deepseek,
            base_url: "https://api.deepseek.com/".to_string(),
            model_id: "deepseek-v4-flash".to_string(),
            auth_mode: crate::ai::catalog::AuthMode::Bearer,
            enabled: true,
            allow_insecure_http: false,
            require_zero_data_retention: false,
            request_timeout_ms: 60_000,
            credential_epoch: 0,
            revision: 1,
        };
        assert_eq!(
            completion_endpoint(&profile, true).unwrap().as_str(),
            "https://api.deepseek.com/beta/chat/completions"
        );
        let mut body = json!({});
        disable_thinking(&mut body);
        apply_strict_document_tool(
            &mut body,
            &json!({
                "type":"object",
                "properties":{"name":{"type":"string","maxLength":20}},
                "required":["name"]
            }),
        );
        assert_eq!(body.pointer("/thinking/type"), Some(&json!("disabled")));
        assert_eq!(
            body.pointer("/tools/0/function/strict"),
            Some(&json!(true))
        );
        assert!(body
            .pointer("/tools/0/function/parameters/properties/name/maxLength")
            .is_none());
    }
}
