//! Hosted embedding wire contracts. Semantic sources, database rows, and
//! conversations never enter this provider-specific module.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::{json, Value};
use url::Url;

use crate::error::CommandError;

use super::super::{
    catalog::ProviderKind,
    profiles::{
        embedding_options::{EmbeddingInputPolicy, EmbeddingOptions},
        ProfileSnapshot,
    },
};

pub(crate) const EMBEDDING_TRANSPORT_VERSION: i64 = 1;
pub(crate) const MAX_EMBEDDING_BATCH: usize = 32;
pub(crate) const MAX_EMBEDDING_INPUT_CHARS: usize = 12_000;
pub(crate) const MAX_EMBEDDING_RESPONSE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub(crate) enum EmbeddingPurpose {
    Query,
    Document,
}

pub(crate) struct EmbeddingRequest {
    pub url: Url,
    pub body: Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingModelChoice {
    pub id: String,
    pub name: String,
}

pub(crate) fn resolved_policy(model: &str, policy: EmbeddingInputPolicy) -> EmbeddingInputPolicy {
    if policy != EmbeddingInputPolicy::Automatic {
        return policy;
    }
    let model = model.to_ascii_lowercase();
    if model.contains("qwen") && model.contains("embedding") {
        EmbeddingInputPolicy::QwenRetrieval
    } else if model.contains("gemini-embedding-001") {
        EmbeddingInputPolicy::SearchTypes
    } else {
        EmbeddingInputPolicy::Symmetric
    }
}

pub(crate) fn prepare_embedding_request(
    profile: &ProfileSnapshot,
    options: &EmbeddingOptions,
    purpose: EmbeddingPurpose,
    inputs: &[String],
) -> Result<EmbeddingRequest, CommandError> {
    profile.validate_for_provider_request()?;
    if inputs.is_empty()
        || inputs.len() > MAX_EMBEDDING_BATCH
        || inputs
            .iter()
            .any(|text| text.trim().is_empty() || text.chars().count() > MAX_EMBEDDING_INPUT_CHARS)
    {
        return Err(invalid(
            "The embedding input batch is empty or exceeds its supported size.",
        ));
    }
    let policy = resolved_policy(&profile.model_id, options.input_policy);
    let inputs = inputs.iter().map(|text| {
        if policy == EmbeddingInputPolicy::QwenRetrieval && matches!(purpose, EmbeddingPurpose::Query) {
            format!("Instruct: Retrieve study material relevant to answering the question.\nQuery: {text}")
        } else {
            text.clone()
        }
    }).collect::<Vec<_>>();
    let mut body = json!({
        "model": profile.model_id,
        "input": inputs,
        "encoding_format": "float",
    });
    if let Some(dimensions) = options.dimensions {
        if !(32..=4096).contains(&dimensions) {
            return Err(invalid("The configured embedding dimensions are invalid."));
        }
        body["dimensions"] = json!(dimensions);
    }
    match profile.provider_kind {
        ProviderKind::Openrouter => {
            body["provider"] = json!({"data_collection": "deny", "require_parameters": true});
            if profile.require_zero_data_retention {
                body["provider"]["zdr"] = json!(true);
            }
        }
        ProviderKind::LlamaCpp => {}
        _ => return Err(unsupported()),
    }
    if policy == EmbeddingInputPolicy::SearchTypes {
        if profile.provider_kind != ProviderKind::Openrouter {
            return Err(invalid(
                "The search-type embedding policy requires a provider that supports input_type.",
            ));
        }
        body["input_type"] = json!(match purpose {
            EmbeddingPurpose::Query => "search_query",
            EmbeddingPurpose::Document => "search_document",
        });
    }
    Ok(EmbeddingRequest {
        url: endpoint(profile, false)?,
        body,
    })
}

pub(crate) fn embedding_models_url(profile: &ProfileSnapshot) -> Result<Url, CommandError> {
    if !matches!(
        profile.provider_kind,
        ProviderKind::Openrouter | ProviderKind::LlamaCpp
    ) {
        return Err(unsupported());
    }
    if !profile.enabled {
        return Err(CommandError::new(
            "ai_profile_disabled",
            "Enable this profile before loading embedding models.",
        ));
    }
    // Listing models is part of choosing one. Do not require model_id here.
    super::super::endpoint_policy::validate_endpoint(
        profile.provider_kind,
        &profile.base_url,
        profile.allow_insecure_http,
    )?;
    endpoint(profile, true)
}

fn endpoint(profile: &ProfileSnapshot, models: bool) -> Result<Url, CommandError> {
    let base = Url::parse(&profile.base_url)
        .map_err(|_| invalid("The embedding provider endpoint is invalid."))?;
    let path = match profile.provider_kind {
        ProviderKind::Openrouter => {
            if models {
                "embeddings/models"
            } else {
                "embeddings"
            }
        }
        ProviderKind::LlamaCpp => {
            if base.path().trim_end_matches('/').ends_with("/v1") {
                if models {
                    "models"
                } else {
                    "embeddings"
                }
            } else if models {
                "v1/models"
            } else {
                "v1/embeddings"
            }
        }
        _ => return Err(unsupported()),
    };
    base.join(path)
        .map_err(|_| invalid("The embedding provider endpoint is invalid."))
}

pub(crate) fn decode_embeddings(
    response: &Value,
    expected_count: usize,
    expected_dimensions: Option<usize>,
) -> Result<Vec<Vec<f32>>, CommandError> {
    let data = response
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("The embedding provider did not return a data array."))?;
    if expected_count == 0 || data.len() != expected_count || data.len() > MAX_EMBEDDING_BATCH {
        return Err(invalid(
            "The embedding provider returned the wrong number of vectors.",
        ));
    }
    let mut result = vec![None; expected_count];
    let mut batch_dimensions = expected_dimensions;
    for entry in data {
        let index = entry
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .filter(|value| *value < expected_count)
            .ok_or_else(|| invalid("An embedding has an invalid input index."))?;
        if result[index].is_some() {
            return Err(invalid("The embedding provider duplicated an input index."));
        }
        let components = entry
            .get("embedding")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("An embedding was not a numeric vector."))?;
        let dimensions = batch_dimensions.get_or_insert(components.len());
        if components.len() != *dimensions || !(1..=4096).contains(dimensions) {
            return Err(invalid(
                "An embedding has incompatible or excessive dimensions.",
            ));
        }
        let vector = components
            .iter()
            .map(|component| {
                let value = component
                    .as_f64()
                    .ok_or_else(|| invalid("An embedding contained a non-numeric component."))?;
                let converted = value as f32;
                if !value.is_finite() || !converted.is_finite() {
                    return Err(invalid("An embedding contained a nonfinite component."));
                }
                Ok(converted)
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        let norm = vector
            .iter()
            .map(|value| f64::from(*value).powi(2))
            .sum::<f64>()
            .sqrt();
        if !norm.is_finite() || norm == 0.0 {
            return Err(invalid(
                "The embedding provider returned a zero or invalid vector.",
            ));
        }
        result[index] = Some(
            vector
                .into_iter()
                .map(|value| (f64::from(value) / norm) as f32)
                .collect(),
        );
    }
    result
        .into_iter()
        .map(|vector| vector.ok_or_else(|| invalid("An embedding input was omitted.")))
        .collect()
}

pub(crate) fn decode_embedding_models(
    response: &Value,
) -> Result<Vec<EmbeddingModelChoice>, CommandError> {
    let data = response
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("The provider did not return an embedding model catalog."))?;
    if data.len() > 10_000 {
        return Err(invalid(
            "The embedding model catalog exceeds the supported limit.",
        ));
    }
    let mut seen = HashSet::new();
    let mut models = Vec::new();
    for model in data {
        let Some(id) = model.get("id").and_then(Value::as_str) else {
            continue;
        };
        if id.trim().is_empty() || id.chars().count() > 300 || !seen.insert(id.to_string()) {
            continue;
        }
        let name = model.get("name").and_then(Value::as_str).unwrap_or(id);
        models.push(EmbeddingModelChoice {
            id: id.to_string(),
            name: name.chars().take(300).collect(),
        });
    }
    models.sort_by(|left, right| left.name.cmp(&right.name).then(left.id.cmp(&right.id)));
    Ok(models)
}

fn invalid(message: &str) -> CommandError {
    CommandError::new("invalid_embedding_response", message)
}

fn unsupported() -> CommandError {
    CommandError::new(
        "ai_embeddings_unsupported",
        "This provider does not have an implemented hosted embedding adapter.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_discovery_allows_a_saved_connection_without_a_model() {
        let mut profile = ProfileSnapshot {
            id: "a".repeat(32),
            provider_kind: ProviderKind::Openrouter,
            base_url: "https://openrouter.ai/api/v1/".to_string(),
            model_id: String::new(),
            reasoning_effort: "none".to_string(),
            auth_mode: crate::ai::catalog::AuthMode::Bearer,
            enabled: true,
            allow_insecure_http: false,
            require_zero_data_retention: false,
            request_timeout_ms: 1000,
            credential_epoch: 0,
            revision: 1,
        };
        assert_eq!(
            embedding_models_url(&profile).unwrap().path(),
            "/api/v1/embeddings/models"
        );
        profile.enabled = false;
        assert!(embedding_models_url(&profile).is_err());
    }

    #[test]
    fn vectors_are_ordered_validated_and_normalized() {
        let response = json!({"data": [
            {"index": 1, "embedding": [0.0, 4.0]},
            {"index": 0, "embedding": [3.0, 0.0]}
        ]});
        assert_eq!(
            decode_embeddings(&response, 2, Some(2)).unwrap(),
            vec![vec![1.0, 0.0], vec![0.0, 1.0]]
        );
        assert!(decode_embeddings(&response, 1, Some(2)).is_err());
        assert!(decode_embeddings(&response, 2, Some(3)).is_err());
        assert!(
            decode_embeddings(&json!({"data":[{"index":0,"embedding":[0,0]}]}), 1, Some(2))
                .is_err()
        );
        assert!(decode_embeddings(
            &json!({"data":[{"index":0,"embedding":[1e100,1]}]}),
            1,
            Some(2)
        )
        .is_err());
    }

    #[test]
    fn duplicate_or_missing_indices_are_rejected() {
        let response = json!({"data": [
            {"index":0,"embedding":[1,0]}, {"index":0,"embedding":[0,1]}
        ]});
        assert!(decode_embeddings(&response, 2, None).is_err());
        assert!(decode_embeddings(&json!({"data":[{"embedding":[1,0]}]}), 1, None).is_err());
    }

    #[test]
    fn automatic_policy_is_model_specific_not_dimension_based() {
        assert_eq!(
            resolved_policy("qwen/qwen3-embedding-4b", EmbeddingInputPolicy::Automatic),
            EmbeddingInputPolicy::QwenRetrieval
        );
        assert_eq!(
            resolved_policy("provider/other", EmbeddingInputPolicy::Automatic),
            EmbeddingInputPolicy::Symmetric
        );
        assert_eq!(
            resolved_policy("qwen/qwen3-embedding-4b", EmbeddingInputPolicy::Symmetric),
            EmbeddingInputPolicy::Symmetric
        );
    }
}
