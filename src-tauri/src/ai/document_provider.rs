//! Provider-neutral capabilities used by document planning.
//!
//! Document services depend on this compact model rather than provider wire
//! formats. Provider adapters/probes are responsible for discovering concrete
//! limits and features; the planner consumes only normalized capability facts.

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::{database::Database, error::CommandError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StructuredOutputStrategy {
    JsonSchema,
    StrictTool,
    Unknown,
}

impl StructuredOutputStrategy {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::JsonSchema => "json_schema",
            Self::StrictTool => "strict_tool",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentReasoningCapabilities {
    pub supported_efforts: Option<Vec<String>>,
    pub default_effort: Option<String>,
    pub default_enabled: Option<bool>,
    #[serde(default)]
    pub supports_max_tokens: bool,
    #[serde(default)]
    pub mandatory: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DocumentProviderCapabilities {
    pub provider_kind: String,
    pub model_id: String,
    pub context_window_tokens: Option<usize>,
    pub image_input: bool,
    pub usage_reporting: bool,
    pub structured_output: StructuredOutputStrategy,
    pub reasoning: Option<DocumentReasoningCapabilities>,
}

impl DocumentProviderCapabilities {
    pub(crate) fn conservative(provider_kind: &str, model_id: &str) -> Self {
        Self {
            provider_kind: provider_kind.to_string(),
            model_id: model_id.to_string(),
            context_window_tokens: None,
            image_input: false,
            usage_reporting: false,
            structured_output: structured_strategy(provider_kind),
            reasoning: None,
        }
    }

    pub(crate) fn resolve_reasoning_effort(&self, requested: &str) -> String {
        // llama.cpp and direct DeepSeek requests explicitly disable thinking in
        // their provider adapters. OpenRouter is the only implemented document
        // transport whose unified reasoning parameter needs model negotiation.
        if self.provider_kind != "openrouter" {
            return "none".to_string();
        }
        let Some(reasoning) = self.reasoning.as_ref() else {
            return "none".to_string();
        };
        let supports = |effort: &str| {
            reasoning
                .supported_efforts
                .as_ref()
                .is_none_or(|efforts| efforts.iter().any(|value| value == effort))
        };
        let lowest_supported = || {
            ["minimal", "low", "medium", "high", "xhigh", "max"]
                .into_iter()
                .find(|effort| supports(effort))
                .unwrap_or("medium")
                .to_string()
        };

        if requested == "none" {
            return if reasoning.mandatory {
                lowest_supported()
            } else {
                "none".to_string()
            };
        }
        if requested == "automatic" {
            if reasoning.default_enabled != Some(true) && !reasoning.mandatory {
                return "none".to_string();
            }
            if let Some(default) = reasoning.default_effort.as_deref() {
                if default == "none" && !reasoning.mandatory {
                    return "none".to_string();
                }
                if supports(default) {
                    return default.to_string();
                }
            }
            return if reasoning.mandatory {
                lowest_supported()
            } else {
                "none".to_string()
            };
        }
        if supports(requested) {
            return requested.to_string();
        }
        if reasoning.mandatory {
            lowest_supported()
        } else {
            // A user asking for Low must never be silently upgraded to High just
            // because a model exposes only high/xhigh. Disable optional reasoning
            // rather than consuming most of the structured-output token budget.
            "none".to_string()
        }
    }

    pub(crate) fn completion_token_ceiling(
        &self,
        effective_reasoning_effort: &str,
        visible_completion_tokens: usize,
    ) -> usize {
        if self.provider_kind != "openrouter" || effective_reasoning_effort == "none" {
            return visible_completion_tokens;
        }
        let visible_percent = match effective_reasoning_effort {
            "minimal" => 90_usize,
            "low" => 80,
            "medium" => 50,
            "high" => 20,
            "xhigh" | "max" => 5,
            _ => 100,
        };
        visible_completion_tokens
            .saturating_mul(100)
            .saturating_add(visible_percent - 1)
            / visible_percent
    }

    pub(crate) fn planning_fingerprint(&self) -> String {
        let context = self
            .context_window_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let reasoning = self
            .reasoning
            .as_ref()
            .map(|value| {
                format!(
                    "efforts={:?};default={:?};enabled={:?};mandatory={}",
                    value.supported_efforts,
                    value.default_effort,
                    value.default_enabled,
                    value.mandatory,
                )
            })
            .unwrap_or_else(|| "unknown".to_string());
        let source = format!(
            "provider={}|model={}|context={context}|images={}|usage={}|structured={}|reasoning={reasoning}",
            self.provider_kind,
            self.model_id,
            self.image_input,
            self.usage_reporting,
            self.structured_output.as_str(),
        );
        format!("{:x}", Sha256::digest(source.as_bytes()))
    }
}

pub(crate) fn load_document_provider_capabilities(
    database: &Database,
    profile_id: &str,
    provider_kind: &str,
    model_id: &str,
) -> Result<DocumentProviderCapabilities, CommandError> {
    database.with_connection(|connection| {
        let mut result = DocumentProviderCapabilities::conservative(provider_kind, model_id);
        let mut statement = connection
            .prepare(
                "SELECT capability_key, status, value_json
                 FROM ai_provider_capabilities
                 WHERE profile_id = ?1
                 ORDER BY capability_key",
            )
            .map_err(CommandError::database)?;
        let rows = statement
            .query_map([profile_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(CommandError::database)?;
        for row in rows {
            let (key, status, value_json) = row.map_err(CommandError::database)?;
            match key.as_str() {
                "context_window_tokens" if status == "supported" => {
                    result.context_window_tokens = parse_positive_usize(&value_json);
                }
                "image_input" => result.image_input = status == "supported",
                "usage_reporting" => result.usage_reporting = status == "supported",
                "reasoning_control" if status == "supported" => {
                    result.reasoning =
                        serde_json::from_str::<Option<DocumentReasoningCapabilities>>(&value_json)
                            .ok()
                            .flatten();
                }
                "json_schema_output" if status == "supported" => {
                    result.structured_output = StructuredOutputStrategy::JsonSchema;
                }
                _ => {}
            }
        }

        // DeepSeek document generation uses strict tool calling even though its
        // generic probe describes JSON-object support. Provider wire details
        // remain here rather than leaking into planners.
        if provider_kind == "deepseek" {
            result.structured_output = StructuredOutputStrategy::StrictTool;
        }
        Ok(result)
    })
}

fn structured_strategy(provider_kind: &str) -> StructuredOutputStrategy {
    match provider_kind {
        "llama_cpp" | "openrouter" => StructuredOutputStrategy::JsonSchema,
        "deepseek" => StructuredOutputStrategy::StrictTool,
        _ => StructuredOutputStrategy::Unknown,
    }
}

fn parse_positive_usize(value_json: &str) -> Option<usize> {
    let value = serde_json::from_str::<serde_json::Value>(value_json).ok()?;
    let value = value.as_i64()?;
    if value <= 0 {
        return None;
    }
    usize::try_from(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprints_change_on_semantically_relevant_provider_limits() {
        let first = DocumentProviderCapabilities {
            provider_kind: "llama_cpp".to_string(),
            model_id: "model".to_string(),
            context_window_tokens: Some(8_192),
            image_input: false,
            usage_reporting: true,
            structured_output: StructuredOutputStrategy::JsonSchema,
            reasoning: None,
        };
        let mut second = first.clone();
        second.context_window_tokens = Some(32_768);
        assert_ne!(first.planning_fingerprint(), second.planning_fingerprint());
    }

    #[test]
    fn optional_reasoning_never_promotes_an_unsupported_low_request() {
        let capabilities = DocumentProviderCapabilities {
            provider_kind: "openrouter".to_string(),
            model_id: "reasoning-model".to_string(),
            context_window_tokens: Some(1_000_000),
            image_input: false,
            usage_reporting: true,
            structured_output: StructuredOutputStrategy::JsonSchema,
            reasoning: Some(DocumentReasoningCapabilities {
                supported_efforts: Some(vec!["high".to_string(), "xhigh".to_string()]),
                default_effort: Some("high".to_string()),
                default_enabled: Some(true),
                supports_max_tokens: false,
                mandatory: false,
            }),
        };
        assert_eq!(capabilities.resolve_reasoning_effort("low"), "none");
        assert_eq!(capabilities.resolve_reasoning_effort("high"), "high");
        assert_eq!(
            capabilities.completion_token_ceiling("none", 10_000),
            10_000
        );
        assert_eq!(
            capabilities.completion_token_ceiling("high", 10_000),
            50_000
        );
    }

    #[test]
    fn mandatory_reasoning_uses_the_lowest_supported_effort() {
        let capabilities = DocumentProviderCapabilities {
            provider_kind: "openrouter".to_string(),
            model_id: "mandatory".to_string(),
            context_window_tokens: Some(1_000_000),
            image_input: false,
            usage_reporting: true,
            structured_output: StructuredOutputStrategy::JsonSchema,
            reasoning: Some(DocumentReasoningCapabilities {
                supported_efforts: Some(vec!["xhigh".to_string(), "high".to_string()]),
                default_effort: Some("xhigh".to_string()),
                default_enabled: Some(true),
                supports_max_tokens: false,
                mandatory: true,
            }),
        };
        assert_eq!(capabilities.resolve_reasoning_effort("none"), "high");
        assert_eq!(capabilities.resolve_reasoning_effort("low"), "high");
    }

    #[test]
    fn deepseek_document_strategy_is_strict_tool_output() {
        let capabilities = DocumentProviderCapabilities::conservative("deepseek", "model");
        assert_eq!(
            capabilities.structured_output,
            StructuredOutputStrategy::StrictTool
        );
    }

    #[test]
    fn openrouter_document_strategy_is_strict_json_schema() {
        let capabilities = DocumentProviderCapabilities::conservative("openrouter", "model");
        assert_eq!(
            capabilities.structured_output,
            StructuredOutputStrategy::JsonSchema
        );
    }

    #[test]
    fn context_capability_parser_rejects_nonpositive_values() {
        assert_eq!(parse_positive_usize("8192"), Some(8192));
        assert_eq!(parse_positive_usize("0"), None);
        assert_eq!(parse_positive_usize("null"), None);
    }
}
