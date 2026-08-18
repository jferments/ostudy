#!/usr/bin/env python3
"""One-shot PR #54 outline-cache correctness and AI observability fix."""
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    if new in value:
        return
    if old not in value:
        raise SystemExit(f"{label}: anchor not found in {path}")
    p.write_text(value.replace(old, new, 1))


path = "src-tauri/src/documents.rs"
replace_once(
    path,
    '''    crate::logging::info(
        "document_ai_scale_plan",
        format!(
            "document={} format={} provider={} model={} scale={} units={} source_tokens={} outline_windows={} generation_windows={} card_capacity={} estimated_job_tokens={} ceiling={} context={} calibration_samples={}",
            document_id,
            source.format.as_str(),
            task.provider_kind,
            task.model_id,
            plan.label(),
            source.unit_count,
            estimated_source_tokens,
            outline_windows.len(),
            generation_windows.len(),
            budget.generation_card_capacity,
            budget.estimated_tokens,
            budget.ceiling_tokens,
            budget.context_window_tokens.map_or_else(|| "unknown".to_string(), |value| value.to_string()),
            budget.calibration_sample_count,
        ),
    );''',
    '''    crate::logging::debug_target(
        "ai.document.plan",
        "document_ai_scale_plan",
        "Planned bounded document-AI work without logging source content.",
        serde_json::json!({
            "document": document_id,
            "format": source.format.as_str(),
            "provider": task.provider_kind.as_str(),
            "model": task.model_id.as_str(),
            "scale": plan.label(),
            "sourceUnits": source.unit_count,
            "sourceTokens": estimated_source_tokens,
            "outlineWindows": outline_windows.len(),
            "generationWindows": generation_windows.len(),
            "cardCapacity": budget.generation_card_capacity,
            "estimatedJobTokens": budget.estimated_tokens,
            "contextWindowTokens": budget.context_window_tokens,
            "calibrationSamples": budget.calibration_sample_count,
        }),
    );''',
    "AI scale-plan diagnostic",
)

old = '''    let outline_contract = document_checkpoints::outline_contract_key(&checkpoint_identity, plan);
    let reusable_final_outline = document_checkpoints::load_final_outline_best_effort(
        database.inner(),
        document_id,
        &checkpoint_identity,
    );'''
new = '''    let outline_contract = document_checkpoints::outline_contract_key(&checkpoint_identity, plan);
    let cached_outline = document_checkpoints::load_outline_fragments_best_effort(
        database.inner(),
        document_id,
        &outline_contract,
        &outline_windows,
    );
    let cached_count = cached_outline
        .iter()
        .filter(|value| value.is_some())
        .count();
    // A persisted whole-document outline is reusable only when every fragment
    // required by the CURRENT semantic-window plan is also present and validates
    // against the current source hashes. This prevents a final outline assembled
    // under an older partition/window plan from silently surviving a planner
    // change merely because provider/model/prompt versions stayed the same.
    let reusable_final_outline = if cached_count == outline_windows.len() {
        document_checkpoints::load_final_outline_best_effort(
            database.inner(),
            document_id,
            &checkpoint_identity,
        )
    } else {
        None
    };'''
replace_once(path, old, new, "current-plan final-outline cache gate")

p = Path(path)
value = p.read_text()
duplicate = '''        let cached_outline = document_checkpoints::load_outline_fragments_best_effort(
            database.inner(),
            document_id,
            &outline_contract,
            &outline_windows,
        );
        let cached_count = cached_outline
            .iter()
            .filter(|value| value.is_some())
            .count();
'''
# The same load used to live inside the cache-miss branch. After the gate above
# it must occur exactly once, before deciding whether the final outline is valid.
if value.count(duplicate) > 0:
    value = value.replace(duplicate, "", 1)
p.write_text(value)

replace_once(
    path,
    '''        crate::logging::info(
            "document_final_outline_reused",
            format!(
                "document={} provider={} model={} sections={}",
                document_id,
                task.provider_kind,
                task.model_id,
                outline.sections.len(),
            ),
        );''',
    '''        crate::logging::debug_target(
            "ai.document.outline",
            "document_final_outline_reused",
            "Reused a whole-document outline only after validating the complete current-plan fragment checkpoint set.",
            serde_json::json!({
                "document": document_id,
                "provider": task.provider_kind.as_str(),
                "model": task.model_id.as_str(),
                "outlineWindows": outline_windows.len(),
                "cachedWindows": cached_count,
                "sections": outline.sections.len(),
            }),
        );''',
    "AI-targeted final outline reuse diagnostic",
)

# Bump the checkpoint contract marker because final-outline reuse semantics now
# explicitly depend on a complete checkpoint set for the current window plan.
replace_once(
    "src-tauri/src/document_checkpoints.rs",
    'const OUTLINE_CHECKPOINT_CONTRACT: &str = "document-outline-v2";',
    'const OUTLINE_CHECKPOINT_CONTRACT: &str = "document-outline-v3";',
    "outline checkpoint contract version",
)

print("PR #54 outline cache/observability fix applied.")
