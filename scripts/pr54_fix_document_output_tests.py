#!/usr/bin/env python3
"""One-shot PR #54 regression/finalization updates for structured output."""
from pathlib import Path


def replace_exact(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    text = p.read_text()
    if old not in text:
        if new in text:
            return
        raise SystemExit(f"{label}: anchor not found in {path}")
    if text.count(old) != 1:
        raise SystemExit(f"{label}: expected one anchor in {path}, found {text.count(old)}")
    p.write_text(text.replace(old, new, 1))


# ---------------------------------------------------------------------------
# Regressions and visible structured-output budget.
# ---------------------------------------------------------------------------
path = Path("src-tauri/src/ai/document_intelligence.rs")
text = path.read_text()
old = '''    #[test]
    fn truncated_provider_output_is_reported_separately_from_schema_drift() {
        let error = ensure_document_response_complete(&json!({
            "choices": [{ "finish_reason": "length" }]
        }))
        .unwrap_err();
        assert_eq!(error.code, "ai_document_output_truncated");
        assert!(error.message.contains("completion limit"));
    }'''
intermediate = old.replace(
    "ensure_document_response_complete",
    "ensure_document_response_not_blocked",
)
new = '''    #[test]
    fn length_is_truncation_only_when_the_structured_result_is_incomplete() {
        ensure_document_response_not_blocked(&json!({
            "choices": [{ "finish_reason": "length" }]
        }))
        .expect("length alone is not a policy or resource failure");
        let error = truncated_output_error();
        assert_eq!(error.code, "ai_document_output_truncated");
        assert!(error.message.contains("completion limit"));
    }'''
if new not in text:
    if text.count(intermediate) == 1:
        text = text.replace(intermediate, new, 1)
    elif text.count(old) == 1:
        text = text.replace(old, new, 1)
    else:
        raise SystemExit(
            f"length-finish regression anchor counts old={text.count(old)} intermediate={text.count(intermediate)}"
        )
# The old helper name was removed by the main applicator. Assert the new pure
# visible-output budget directly; the reasoning headroom is tested separately.
text = text.replace(
    "assert_eq!(flashcard_completion_tokens(8), 4_096);",
    "assert_eq!(generation_visible_completion_tokens(8), 5_120);",
)
path.write_text(text)

path = Path("src-tauri/src/document_checkpoints.rs")
text = path.read_text()
old = '''            extraction_backend: "extractor-v1".to_string(),
            provider_capability_fingerprint: "caps".to_string(),
        };'''
new = '''            extraction_backend: "extractor-v1".to_string(),
            provider_capability_fingerprint: "caps".to_string(),
            reasoning_effort: "none".to_string(),
        };'''
if new not in text:
    if text.count(old) != 1:
        raise SystemExit("checkpoint identity regression anchor not found")
    text = text.replace(old, new, 1)
path.write_text(text)

path = Path("src-tauri/src/document_scale.rs")
text = path.read_text()
old = '''pub(crate) fn generation_visible_completion_tokens(maximum_cards: usize) -> usize {
    1_536_usize
        .saturating_add(maximum_cards.saturating_mul(512))
        .clamp(2_048, 10_240)
}'''
new = '''pub(crate) fn generation_visible_completion_tokens(maximum_cards: usize) -> usize {
    2_048_usize
        .saturating_add(maximum_cards.saturating_mul(384))
        .clamp(2_048, 8_192)
}'''
if new not in text:
    if text.count(old) != 1:
        raise SystemExit("generation visible-output budget anchor not found")
    text = text.replace(old, new, 1)
path.write_text(text)

# ---------------------------------------------------------------------------
# Compile integration. The earlier generic applicators deliberately use simple
# exact replacements. Finish the places where replacement text was itself a
# substring of an old form or where the same broad anchor existed elsewhere.
# ---------------------------------------------------------------------------
replace_exact(
    "src-tauri/src/task_cancellation.rs",
    "#[cfg(test)]\npub(crate) fn current_reasoning_effort() -> Option<String> {",
    "pub(crate) fn current_reasoning_effort() -> Option<String> {",
    "production task reasoning accessor",
)
replace_exact(
    "src-tauri/src/ai/document_intelligence.rs",
    "capabilities: &super::document_provider::DocumentProviderCapabilities,",
    "capabilities: &crate::ai::document_provider::DocumentProviderCapabilities,",
    "document provider capability module path",
)
replace_exact(
    "src-tauri/src/ai/document_intelligence_adapter.rs",
    "curate_document_cards, generate_window_flashcards, load_document_task_context, DocumentOutline,",
    "curate_document_cards, flashcard_completion_token_ceiling, generate_window_flashcards,\n    load_document_task_context, DocumentOutline,",
    "completion ceiling adapter re-export",
)

# Reasoning is resolved once from the selected model's normalized declarations
# before either window planning or aggregate budgeting.
for file_name, window_argument in [
    ("src-tauri/src/documents.rs", "generation_windows.len()"),
    ("src-tauri/src/document_more.rs", "windows.len()"),
]:
    replace_exact(
        file_name,
        "let task = document_intelligence::load_document_task_context(database.inner())?;\n    let capabilities = document_provider::load_document_provider_capabilities(",
        "let mut task = document_intelligence::load_document_task_context(database.inner())?;\n    let capabilities = document_provider::load_document_provider_capabilities(",
        f"mutable document task in {file_name}",
    )
    replace_exact(
        file_name,
        '''        &task.model_id,
    )?;
    let calibration = document_cost::load_cost_calibration(''',
        '''        &task.model_id,
    )?;
    task.apply_provider_capabilities(&capabilities);
    let calibration = document_cost::load_cost_calibration(''',
        f"reasoning resolution in {file_name}",
    )
    replace_exact(
        file_name,
        "let plan = DocumentScalePlan::for_capabilities(source.unit_count, &capabilities, calibration)?;",
        '''let plan = DocumentScalePlan::for_capabilities(
        source.unit_count,
        &capabilities,
        &task.reasoning_effort,
        calibration,
    )?;''',
        f"reasoning-aware scale plan in {file_name}",
    )
    old_budget = f'''        {window_argument},
        &capabilities,
        calibration,
    )?;'''
    new_budget = f'''        {window_argument},
        &capabilities,
        &task.reasoning_effort,
        calibration,
    )?;'''
    replace_exact(
        file_name,
        old_budget,
        new_budget,
        f"reasoning-aware budget in {file_name}",
    )

replace_exact(
    "src-tauri/src/ai/probes.rs",
    '''        let capabilities = build_capabilities(
            &profile,
            true,
            true,
            true,
            true,
            &ProviderProperties::default(),
        );''',
    '''        let capabilities = build_capabilities(
            &profile,
            None,
            true,
            true,
            true,
            true,
            &ProviderProperties::default(),
        );''',
    "probe capability regression model metadata argument",
)

print("Updated PR #54 length-finish, checkpoint, visible-output, and compile integration regressions.")
