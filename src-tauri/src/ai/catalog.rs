use serde::{Deserialize, Serialize};

use crate::error::CommandError;

pub const DEFAULT_TIMEOUT_MS: i64 = 60_000;
pub const MIN_TIMEOUT_MS: i64 = 1_000;
pub const MAX_TIMEOUT_MS: i64 = 300_000;
pub const DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    LlamaCpp,
    Deepseek,
    Ollama,
    Openai,
    Gemini,
    Claude,
}

impl ProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LlamaCpp => "llama_cpp",
            Self::Deepseek => "deepseek",
            Self::Ollama => "ollama",
            Self::Openai => "openai",
            Self::Gemini => "gemini",
            Self::Claude => "claude",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "llama_cpp" => Ok(Self::LlamaCpp),
            "deepseek" => Ok(Self::Deepseek),
            "ollama" => Ok(Self::Ollama),
            "openai" => Ok(Self::Openai),
            "gemini" => Ok(Self::Gemini),
            "claude" => Ok(Self::Claude),
            _ => Err(CommandError::new(
                "invalid_ai_provider",
                "The saved AI provider kind is not supported by this ostudy build.",
            )),
        }
    }

    pub fn implemented(self) -> bool {
        matches!(self, Self::LlamaCpp | Self::Deepseek)
    }

    pub fn fixed_base_url(self) -> Option<&'static str> {
        matches!(self, Self::Deepseek).then_some(DEEPSEEK_BASE_URL)
    }

    pub fn default_base_url(self) -> &'static str {
        match self {
            Self::LlamaCpp => "http://127.0.0.1:8080/",
            Self::Deepseek => DEEPSEEK_BASE_URL,
            Self::Ollama => "http://127.0.0.1:11434/",
            Self::Openai => "https://api.openai.com/",
            Self::Gemini => "https://generativelanguage.googleapis.com/",
            Self::Claude => "https://api.anthropic.com/",
        }
    }

    pub fn default_auth_mode(self) -> AuthMode {
        match self {
            Self::LlamaCpp | Self::Ollama => AuthMode::None,
            Self::Deepseek | Self::Openai | Self::Gemini | Self::Claude => AuthMode::Bearer,
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::LlamaCpp => "llama.cpp / Unsloth GGUF",
            Self::Deepseek => "DeepSeek",
            Self::Ollama => "Ollama",
            Self::Openai => "OpenAI",
            Self::Gemini => "Gemini",
            Self::Claude => "Claude",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    None,
    Bearer,
}

impl AuthMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bearer => "bearer",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "none" => Ok(Self::None),
            "bearer" => Ok(Self::Bearer),
            _ => Err(CommandError::new(
                "invalid_ai_auth_mode",
                "The saved AI authentication mode is invalid.",
            )),
        }
    }

    pub fn requires_credential(self) -> bool {
        matches!(self, Self::Bearer)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AiTaskKind {
    FlashcardGeneration,
}

impl AiTaskKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FlashcardGeneration => "flashcard_generation",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, CommandError> {
        match value {
            "flashcard_generation" => Ok(Self::FlashcardGeneration),
            _ => Err(CommandError::new(
                "invalid_ai_task",
                "The saved AI task assignment is invalid.",
            )),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCatalogEntryDto {
    pub provider_kind: ProviderKind,
    pub display_name: &'static str,
    pub description: &'static str,
    pub implementation_status: &'static str,
    pub default_base_url: &'static str,
    pub base_url_editable: bool,
    pub authentication_required: bool,
    pub model_id_required: bool,
    pub model_discovery_supported: bool,
    pub chat_template_policy: &'static str,
    pub default_timeout_ms: i64,
}

pub fn provider_catalog() -> Vec<ProviderCatalogEntryDto> {
    [
        ProviderKind::LlamaCpp,
        ProviderKind::Deepseek,
        ProviderKind::Ollama,
        ProviderKind::Openai,
        ProviderKind::Gemini,
        ProviderKind::Claude,
    ]
    .into_iter()
    .map(|kind| ProviderCatalogEntryDto {
        provider_kind: kind,
        display_name: kind.display_name(),
        description: match kind {
            ProviderKind::LlamaCpp => {
                "Connect to an existing llama.cpp server, including GGUF models exported by Unsloth."
            }
            ProviderKind::Deepseek => "Connect to DeepSeek's official cloud API.",
            ProviderKind::Ollama => "Native Ollama support is planned for a later AI stage.",
            ProviderKind::Openai => "OpenAI Responses support is planned for a later AI stage.",
            ProviderKind::Gemini => "Gemini support is planned for a later AI stage.",
            ProviderKind::Claude => "Claude support is planned for a later AI stage.",
        },
        implementation_status: if kind.implemented() {
            "available"
        } else {
            "planned"
        },
        default_base_url: kind.default_base_url(),
        base_url_editable: kind.fixed_base_url().is_none(),
        authentication_required: kind.default_auth_mode().requires_credential(),
        model_id_required: true,
        model_discovery_supported: kind.implemented(),
        chat_template_policy: if matches!(kind, ProviderKind::LlamaCpp | ProviderKind::Ollama) {
            "server_model"
        } else {
            "provider_managed"
        },
        default_timeout_ms: DEFAULT_TIMEOUT_MS,
    })
    .collect()
}
