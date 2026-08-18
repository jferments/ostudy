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
# substring of an old form or where a broad anchor existed in several callers.
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

# The existing settings subsystem already keeps an active reasoning preference
# synchronized on load/save. Snapshot that value at task creation rather than
# introducing another settings lifetime or leaving the existing reader unused.
replace_exact(
    "src-tauri/src/ai/document_intelligence.rs",
    "let requested_reasoning_effort = crate::settings::load_ai_reasoning_effort(connection)?;",
    "let requested_reasoning_effort = crate::settings::current_ai_reasoning_effort();",
    "task reasoning preference snapshot",
)

# Remove the pre-refactor completion reserve helpers that the new visible-output
# policy supersedes.
path = Path("src-tauri/src/document_scale.rs")
text = path.read_text()
obsolete = '''fn outline_completion_reserve(_provider_kind: &str) -> usize {
    // Local outline fragments are deliberately concise. A 1.5k-token reserve
    // leaves ample room for the strict fragment schema without encouraging a
    // multi-thousand-token restatement of source prose.
    1_536
}

fn generation_completion_reserve(maximum_cards: usize) -> usize {
    1_024_usize
        .saturating_add(maximum_cards.saturating_mul(384))
        .clamp(1_536, 5_120)
}

'''
if obsolete in text:
    text = text.replace(obsolete, "", 1)
path.write_text(text)

# ---------------------------------------------------------------------------
# Keep function signatures below the repository's strict Clippy boundary by
# grouping related output limits and aggregate-budget calibration.
# ---------------------------------------------------------------------------
path = Path("src-tauri/src/ai/document_intelligence.rs")
text = path.read_text()
old_signature = '''pub(crate) async fn generate_window_flashcards(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    outline: &DocumentOutline,
    window: &SemanticWindow,
    images: &[SourcePageImage],
    maximum_cards: usize,
    completion_token_floor: usize,
) -> Result<Vec<Value>, CommandError> {
    let maximum_cards = maximum_cards.clamp(1, MAX_WINDOW_CARDS);'''
new_signature = '''pub(crate) struct FlashcardGenerationLimits {
    pub maximum_cards: usize,
    pub completion_token_floor: usize,
}

pub(crate) async fn generate_window_flashcards(
    runtime: &AiRuntimeState,
    task: &DocumentTaskContext,
    document_title: &str,
    outline: &DocumentOutline,
    window: &SemanticWindow,
    images: &[SourcePageImage],
    limits: FlashcardGenerationLimits,
) -> Result<Vec<Value>, CommandError> {
    let maximum_cards = limits.maximum_cards.clamp(1, MAX_WINDOW_CARDS);'''
if old_signature not in text:
    if new_signature not in text:
        raise SystemExit("flashcard generation limits signature anchor not found")
else:
    text = text.replace(old_signature, new_signature, 1)
text = text.replace(
    '''        maximum_cards,
        completion_token_floor,
    )''',
    '''        maximum_cards,
        limits.completion_token_floor,
    )''',
    1,
)
path.write_text(text)

path = Path("src-tauri/src/ai/document_intelligence_adapter.rs")
text = path.read_text()
old = '''curate_document_cards, flashcard_completion_token_ceiling, generate_window_flashcards,
    load_document_task_context, DocumentOutline,'''
new = '''curate_document_cards, flashcard_completion_token_ceiling, generate_window_flashcards,
    load_document_task_context, DocumentOutline, FlashcardGenerationLimits,'''
if old in text:
    text = text.replace(old, new, 1)
elif new not in text:
    raise SystemExit("flashcard limits adapter export anchor not found")
path.write_text(text)

path = Path("src-tauri/src/document_generation.rs")
text = path.read_text()
old = '''        input.images,
        first_budget,
        0,
    )'''
new = '''        input.images,
        document_intelligence::FlashcardGenerationLimits {
            maximum_cards: first_budget,
            completion_token_floor: 0,
        },
    )'''
if old in text:
    text = text.replace(old, new, 1)
elif new not in text:
    raise SystemExit("initial flashcard limits call anchor not found")
old = '''                input.images,
                reduced_budget,
                completion_token_floor,
            )'''
new = '''                input.images,
                document_intelligence::FlashcardGenerationLimits {
                    maximum_cards: reduced_budget,
                    completion_token_floor,
                },
            )'''
if old in text:
    text = text.replace(old, new, 1)
elif new not in text:
    raise SystemExit("retry flashcard limits call anchor not found")
path.write_text(text)

path = Path("src-tauri/src/document_scale.rs")
text = path.read_text()
context_struct = '''pub(crate) struct DocumentBudgetContext<'a> {
    pub capabilities: &'a DocumentProviderCapabilities,
    pub reasoning_effort: &'a str,
    pub calibration: DocumentCostCalibration,
}

'''
anchor = "#[cfg(test)]\npub(crate) fn validate_automatic_job("
if context_struct not in text:
    if anchor not in text:
        raise SystemExit("document budget context insertion anchor not found")
    text = text.replace(anchor, context_struct + anchor, 1)
old = '''        &DocumentProviderCapabilities::conservative(provider_kind, "unknown"),
        "none",
        DocumentCostCalibration::default(),
    )'''
new = '''        DocumentBudgetContext {
            capabilities: &DocumentProviderCapabilities::conservative(provider_kind, "unknown"),
            reasoning_effort: "none",
            calibration: DocumentCostCalibration::default(),
        },
    )'''
if old in text:
    text = text.replace(old, new, 1)
elif new not in text:
    raise SystemExit("test document budget context call anchor not found")
old = '''pub(crate) fn validate_automatic_job_calibrated(
    source_unit_count: usize,
    source_tokens: usize,
    plan: DocumentScalePlan,
    outline_window_count: usize,
    generation_window_count: usize,
    capabilities: &DocumentProviderCapabilities,
    effective_reasoning_effort: &str,
    calibration: DocumentCostCalibration,
) -> Result<DocumentJobBudget, CommandError> {
    if source_unit_count > MAX_SUPPORTED_SOURCE_UNITS {'''
new = '''pub(crate) fn validate_automatic_job_calibrated(
    source_unit_count: usize,
    source_tokens: usize,
    plan: DocumentScalePlan,
    outline_window_count: usize,
    generation_window_count: usize,
    context: DocumentBudgetContext<'_>,
) -> Result<DocumentJobBudget, CommandError> {
    let DocumentBudgetContext {
        capabilities,
        reasoning_effort: effective_reasoning_effort,
        calibration,
    } = context;
    if source_unit_count > MAX_SUPPORTED_SOURCE_UNITS {'''
if old in text:
    text = text.replace(old, new, 1)
elif new not in text:
    raise SystemExit("document budget function signature anchor not found")
path.write_text(text)

for file_name, window_argument in [
    ("src-tauri/src/documents.rs", "generation_windows.len()"),
    ("src-tauri/src/document_more.rs", "windows.len()"),
]:
    path = Path(file_name)
    text = path.read_text()
    old = f'''        {window_argument},
        &capabilities,
        &task.reasoning_effort,
        calibration,
    )?;'''
    new = f'''        {window_argument},
        document_scale::DocumentBudgetContext {{
            capabilities: &capabilities,
            reasoning_effort: &task.reasoning_effort,
            calibration,
        }},
    )?;'''
    if old in text:
        text = text.replace(old, new, 1)
    elif new not in text:
        raise SystemExit(f"document budget context call anchor not found in {file_name}")
    path.write_text(text)

print("Updated PR #54 output recovery regressions and warning-clean integration.")
