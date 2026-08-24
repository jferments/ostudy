use serde_json::{json, Value};

use crate::ai::{
    catalog::ProviderKind,
    endpoint_policy::join_endpoint,
    gateway::{AiGateway, AiGatewayError},
    profiles::ProfileSnapshot,
};

use super::{
    dialect::{command_to_gateway, ProviderDialect, DEFAULT_MAX_MODEL_LIST_BYTES},
    llama_cpp, openrouter, ProviderModel, ProviderProperties,
};

pub(crate) async fn list_models(
    gateway: &AiGateway,
    profile: &ProfileSnapshot,
    bearer: Option<&str>,
) -> Result<Vec<ProviderModel>, AiGatewayError> {
    let dialect = ProviderDialect::for_kind(profile.provider_kind)?;
    let url = dialect.model_endpoint(profile)?;
    let (maximum_bytes, maximum_models) = dialect.model_limits();
    crate::logging::debug_target(
        "ai.provider",
        "provider_model_discovery_started",
        "The provider model catalog request started.",
        json!({
            "providerKind": profile.provider_kind.as_str(),
            "maximumBytes": maximum_bytes,
            "maximumModels": maximum_models,
        }),
    );
    let value = match gateway
        .get_json(url, bearer, profile.request_timeout_ms, maximum_bytes)
        .await
    {
        Ok(value) => value,
        Err(error_value)
            if dialect == ProviderDialect::Openrouter
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
    let models = parse_model_list(&value, dialect, maximum_models)?;
    crate::logging::debug_target(
        "ai.provider",
        "provider_model_discovery_completed",
        "The provider model catalog request completed.",
        json!({
            "providerKind": profile.provider_kind.as_str(),
            "modelCount": models.len(),
        }),
    );
    Ok(models)
}

pub(crate) async fn load_properties(
    gateway: &AiGateway,
    profile: &ProfileSnapshot,
    bearer: Option<&str>,
) -> Result<ProviderProperties, AiGatewayError> {
    if profile.provider_kind != ProviderKind::LlamaCpp {
        return Ok(ProviderProperties::default());
    }
    let mut url = llama_cpp::root_endpoint(&profile.base_url, "props")?;
    url.query_pairs_mut().append_pair("model", &profile.model_id);
    match gateway
        .get_json(
            url,
            bearer,
            profile.request_timeout_ms,
            DEFAULT_MAX_MODEL_LIST_BYTES,
        )
        .await
    {
        Ok(value) => Ok(llama_cpp::parse_properties(&value)),
        Err(error_value)
            if matches!(
                error_value.safe.code.as_str(),
                "ai_provider_protocol_error" | "ai_http_not_found"
            ) => Ok(ProviderProperties::default()),
        Err(error_value) => Err(error_value),
    }
}

fn parse_model_list(
    value: &Value,
    dialect: ProviderDialect,
    maximum_models: usize,
) -> Result<Vec<ProviderModel>, AiGatewayError> {
    let data = value.get("data").and_then(Value::as_array).ok_or_else(|| {
        crate::ai::gateway::error(
            "ai_provider_protocol_error",
            "The provider model list did not use the expected response shape.",
            false,
            None,
        )
    })?;
    if data.len() > maximum_models {
        return Err(crate::ai::gateway::error(
            "ai_model_list_too_large",
            "The provider returned more models than ostudy accepts in one response.",
            false,
            None,
        ));
    }

    let mut models = Vec::with_capacity(data.len());
    let mut declared_incompatible = 0_usize;
    for item in data {
        if dialect == ProviderDialect::Openrouter
            && openrouter::model_structured_outputs_declared(item) == Some(false)
        {
            declared_incompatible = declared_incompatible.saturating_add(1);
            continue;
        }
        let id = item.get("id").and_then(Value::as_str).ok_or_else(|| {
            crate::ai::gateway::error(
                "ai_provider_protocol_error",
                "A provider model entry did not include an ID.",
                false,
                None,
            )
        })?;
        if id.trim().is_empty() || id.len() > 300 || id.chars().any(char::is_control) {
            return Err(crate::ai::gateway::error(
                "ai_provider_protocol_error",
                "A provider model ID exceeded ostudy's accepted bounds.",
                false,
                None,
            ));
        }
        let (display_name, owned_by, context_window_tokens, reasoning) =
            if dialect == ProviderDialect::Openrouter {
                (
                    openrouter::model_display_name(item, id),
                    openrouter::model_owner(item, id),
                    openrouter::model_context_window_tokens(item),
                    openrouter::model_reasoning(item),
                )
            } else {
                (
                    id.to_string(),
                    item.get("owned_by")
                        .and_then(Value::as_str)
                        .filter(|value| value.len() <= 200 && !value.chars().any(char::is_control))
                        .map(ToString::to_string),
                    None,
                    None,
                )
            };
        models.push(ProviderModel {
            id: id.to_string(),
            display_name,
            owned_by,
            context_window_tokens,
            reasoning,
        });
    }
    if declared_incompatible > 0 {
        crate::logging::debug_target(
            "ai.provider.openrouter",
            "openrouter_models_without_structured_output_hidden",
            "OpenRouter models that explicitly lack structured outputs were excluded from the selectable catalog.",
            json!({"modelCount": declared_incompatible}),
        );
    }
    models.sort_by(|left, right| {
        left.display_name
            .to_ascii_lowercase()
            .cmp(&right.display_name.to_ascii_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
    models.dedup_by(|left, right| left.id == right.id);
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::super::dialect::DEFAULT_MAX_MODELS;
    use super::*;

    #[test]
    fn common_model_parser_deduplicates_openai_style_lists() {
        let models = parse_model_list(
            &json!({"data":[
                {"id":"model-b","owned_by":"local"},
                {"id":"model-a"},
                {"id":"model-a"}
            ]}),
            ProviderDialect::LlamaCpp,
            DEFAULT_MAX_MODELS,
        )
        .expect("model list should parse");
        assert_eq!(
            models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            vec!["model-a", "model-b"]
        );
    }

    #[test]
    fn openrouter_catalog_enrichment_is_the_only_model_metadata_specialization() {
        let models = parse_model_list(
            &json!({"data":[{
                "id":"deepseek/example",
                "name":"DeepSeek Example",
                "context_length":1_310_720,
                "reasoning":{
                    "mandatory":false,
                    "default_enabled":true,
                    "supported_efforts":["max","high","low"],
                    "default_effort":"high"
                }
            }]}),
            ProviderDialect::Openrouter,
            openrouter::MAX_MODELS,
        )
        .expect("model list should parse");
        assert_eq!(models[0].display_name, "DeepSeek Example");
        assert_eq!(models[0].owned_by.as_deref(), Some("deepseek"));
        assert_eq!(models[0].context_window_tokens, Some(1_310_720));
        assert_eq!(
            models[0]
                .reasoning
                .as_ref()
                .and_then(|reasoning| reasoning.default_effort.as_deref()),
            Some("high")
        );
    }
}
