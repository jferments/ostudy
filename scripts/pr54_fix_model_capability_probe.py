#!/usr/bin/env python3
"""One-shot PR #54 fix to preserve model declarations through full provider probes."""
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    text = p.read_text()
    if new in text:
        return
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one anchor in {path}, found {count}")
    p.write_text(text.replace(old, new, 1))


path = "src-tauri/src/ai/probes.rs"
replace_once(
    path,
    '''            let capabilities =
                build_capabilities(&profile, models.is_some(), false, false, false, &properties);''',
    '''            let capabilities = build_capabilities(
                &profile,
                models.as_deref(),
                models.is_some(),
                false,
                false,
                false,
                &properties,
            );''',
    "failed probe capability metadata",
)
replace_once(
    path,
    '''    let capabilities = build_capabilities(
        &profile,
        models.is_some(),
        true,
        structured_outcome.is_some(),
        usage_reported,
        &properties,
    );''',
    '''    let capabilities = build_capabilities(
        &profile,
        models.as_deref(),
        models.is_some(),
        true,
        structured_outcome.is_some(),
        usage_reported,
        &properties,
    );''',
    "successful probe capability metadata",
)
replace_once(
    path,
    '''fn build_capabilities(
    profile: &ProfileSnapshot,
    model_listing: bool,''',
    '''fn build_capabilities(
    profile: &ProfileSnapshot,
    models: Option<&[ProviderModel]>,
    model_listing: bool,''',
    "capability builder model metadata signature",
)
replace_once(
    path,
    '''    set_capability(
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

    if let Some(present) = properties.server_chat_template_present {''',
    '''    set_capability(
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

    if let Some(model) = models.and_then(|models| models.iter().find(|model| model.id == profile.model_id)) {
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

    if let Some(present) = properties.server_chat_template_present {''',
    "model declarations in probe capabilities",
)

print("Applied PR #54 selected-model capability probe fixes.")
