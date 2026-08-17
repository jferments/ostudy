//! Provider-neutral capabilities used by document planning.
//!
//! Document services depend on this compact model rather than provider wire
//! formats. Provider adapters/probes are responsible for discovering concrete
//! limits and features; the planner consumes only normalized capability facts.

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DocumentProviderCapabilities {
    pub provider_kind: String,
    pub model_id: String,
    pub context_window_tokens: Option<usize>,
    pub image_input: bool,
    pub usage_reporting: bool,
    pub structured_output: StructuredOutputStrategy,
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
        }
    }

    pub(crate) fn planning_fingerprint(&self) -> String {
        let context = self
            .context_window_tokens
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let source = format!(
            "provider={}|model={}|context={context}|images={}|usage={}|structured={}",
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
        };
        let mut second = first.clone();
        second.context_window_tokens = Some(32_768);
        assert_ne!(first.planning_fingerprint(), second.planning_fingerprint());
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
