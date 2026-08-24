use serde_json::{json, Value};

use super::{
    super::{
        catalog::ProviderKind,
        profiles::ProfileSnapshot,
        providers::{ProviderModel, ProviderProperties},
    },
    contracts::{AiCapabilityDto, CapabilitySource, CapabilityStatus},
};

pub(super) struct CapabilityEvidence<'a> {
    pub models: Option<&'a [ProviderModel]>,
    pub model_listing: bool,
    pub text_completion: bool,
    pub structured_output: bool,
    pub document_generation_schema: Option<bool>,
    pub usage_reported: bool,
    pub properties: &'a ProviderProperties,
}

pub(super) fn build_capabilities(
    profile: &ProfileSnapshot,
    evidence: CapabilityEvidence<'_>,
) -> Vec<AiCapabilityDto> {
    let CapabilityEvidence {
        models,
        model_listing,
        text_completion,
        structured_output,
        document_generation_schema,
        usage_reported,
        properties,
    } = evidence;
    let mut capabilities = default_unknown_capabilities(profile.provider_kind);
    set_capability(
        &mut capabilities,
        "model_listing",
        if model_listing {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unknown
        },
        CapabilitySource::Probed,
        None,
    );
    set_capability(
        &mut capabilities,
        "text_input",
        if text_completion {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unknown
        },
        CapabilitySource::Probed,
        None,
    );
    set_capability(
        &mut capabilities,
        "json_object_output",
        if structured_output {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unknown
        },
        CapabilitySource::Probed,
        None,
    );
    set_capability(
        &mut capabilities,
        "json_schema_output",
        match profile.provider_kind {
            ProviderKind::LlamaCpp | ProviderKind::Openrouter if structured_output => {
                CapabilityStatus::Supported
            }
            ProviderKind::LlamaCpp | ProviderKind::Openrouter => CapabilityStatus::Unknown,
            ProviderKind::Deepseek => CapabilityStatus::Unsupported,
            _ => CapabilityStatus::Unknown,
        },
        CapabilitySource::Probed,
        None,
    );
    if let Some(supported) = document_generation_schema {
        set_capability(
            &mut capabilities,
            "document_generation_schema",
            if supported {
                CapabilityStatus::Supported
            } else {
                CapabilityStatus::Unsupported
            },
            CapabilitySource::Probed,
            None,
        );
    }
    set_capability(
        &mut capabilities,
        "local_schema_validation",
        if structured_output {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unknown
        },
        CapabilitySource::Probed,
        None,
    );
    set_capability(
        &mut capabilities,
        "usage_reporting",
        if usage_reported {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unknown
        },
        CapabilitySource::Probed,
        None,
    );

    if let Some(model) =
        models.and_then(|models| models.iter().find(|model| model.id == profile.model_id))
    {
        if let Some(context_window_tokens) = model.context_window_tokens {
            set_capability(
                &mut capabilities,
                "context_window_tokens",
                CapabilityStatus::Supported,
                CapabilitySource::Declared,
                Some(json!(context_window_tokens)),
            );
        }
        if let Some(reasoning) = model.reasoning.as_ref() {
            set_capability(
                &mut capabilities,
                "reasoning_control",
                CapabilityStatus::Supported,
                CapabilitySource::Declared,
                serde_json::to_value(reasoning).ok(),
            );
        }
    }

    if let Some(present) = properties.server_chat_template_present {
        set_capability(
            &mut capabilities,
            "server_chat_template",
            if present {
                CapabilityStatus::Supported
            } else {
                CapabilityStatus::Unsupported
            },
            CapabilitySource::Declared,
            properties
                .server_chat_template_hash
                .as_ref()
                .map(|value| json!({ "sha256": value })),
        );
    }
    if let Some(image_input) = properties.image_input_declared {
        set_capability(
            &mut capabilities,
            "image_input",
            if image_input {
                CapabilityStatus::Supported
            } else {
                CapabilityStatus::Unsupported
            },
            CapabilitySource::Declared,
            None,
        );
    }
    if let Some(context_window_tokens) = properties.context_window_tokens {
        set_capability(
            &mut capabilities,
            "context_window_tokens",
            CapabilityStatus::Supported,
            CapabilitySource::Declared,
            Some(json!(context_window_tokens)),
        );
    }
    capabilities
}

pub(super) fn default_unknown_capabilities(provider_kind: ProviderKind) -> Vec<AiCapabilityDto> {
    [
        "text_input",
        "image_input",
        "json_object_output",
        "json_schema_output",
        "document_generation_schema",
        "local_schema_validation",
        "model_listing",
        "usage_reporting",
        "server_chat_template",
        "streaming",
        "reasoning_control",
        "embeddings",
        "context_window_tokens",
    ]
    .into_iter()
    .map(|key| AiCapabilityDto {
        key: key.to_string(),
        status: if key == "json_schema_output" && provider_kind == ProviderKind::Deepseek {
            CapabilityStatus::Unsupported
        } else {
            CapabilityStatus::Unknown
        },
        source: CapabilitySource::Probed,
        value: None,
    })
    .collect()
}

pub(super) fn set_capability(
    capabilities: &mut [AiCapabilityDto],
    key: &str,
    status: CapabilityStatus,
    source: CapabilitySource,
    value: Option<Value>,
) {
    if let Some(capability) = capabilities.iter_mut().find(|item| item.key == key) {
        capability.status = status;
        capability.source = source;
        capability.value = value;
    }
}
