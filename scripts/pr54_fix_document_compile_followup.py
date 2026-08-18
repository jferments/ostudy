#!/usr/bin/env python3
"""One-shot compile follow-up for the PR #54 document-output repair."""
from pathlib import Path


def replace_exact(path: str, old: str, new: str, label: str, *, expected: int = 1) -> None:
    p = Path(path)
    text = p.read_text()
    if new in text and old not in text:
        return
    count = text.count(old)
    if count != expected:
        raise SystemExit(f"{label}: expected {expected} anchor(s) in {path}, found {count}")
    p.write_text(text.replace(old, new, expected))


# The generic helper used by the earlier delivery script treats the replacement
# text as an idempotency marker. Here the replacement is a substring of the old
# form, so remove the test-only attribute explicitly.
replace_exact(
    "src-tauri/src/task_cancellation.rs",
    "#[cfg(test)]\npub(crate) fn current_reasoning_effort() -> Option<String> {",
    "pub(crate) fn current_reasoning_effort() -> Option<String> {",
    "production task reasoning accessor",
)

# document_intelligence.rs is compiled as implementation under the adapter, so
# crate-level provider types must use the absolute module path.
replace_exact(
    "src-tauri/src/ai/document_intelligence.rs",
    "capabilities: &super::document_provider::DocumentProviderCapabilities,",
    "capabilities: &crate::ai::document_provider::DocumentProviderCapabilities,",
    "document provider capability module path",
)

# The retry path lives outside the implementation submodule and needs the
# normalized completion-ceiling helper re-exported by the adapter.
replace_exact(
    "src-tauri/src/ai/document_intelligence_adapter.rs",
    "curate_document_cards, generate_window_flashcards, load_document_task_context, DocumentOutline,",
    "curate_document_cards, flashcard_completion_token_ceiling, generate_window_flashcards,\n    load_document_task_context, DocumentOutline,",
    "flashcard completion ceiling re-export",
)

# Apply the effective reasoning effort before both context-safe window planning
# and aggregate diagnostic budgeting. These replacements are deliberately fully
# specified instead of relying on broad anchors shared by several call sites.
for path in ["src-tauri/src/documents.rs", "src-tauri/src/document_more.rs"]:
    replace_exact(
        path,
        "let task = document_intelligence::load_document_task_context(database.inner())?;\n    let capabilities = document_provider::load_document_provider_capabilities(",
        "let mut task = document_intelligence::load_document_task_context(database.inner())?;\n    let capabilities = document_provider::load_document_provider_capabilities(",
        f"mutable task context in {path}",
    )
    marker = '''        &task.model_id,\n    )?;\n    let calibration = document_cost::load_cost_calibration('''
    replacement = '''        &task.model_id,\n    )?;\n    task.apply_provider_capabilities(&capabilities);\n    let calibration = document_cost::load_cost_calibration('''
    replace_exact(path, marker, replacement, f"reasoning capability application in {path}")
    replace_exact(
        path,
        "let plan = DocumentScalePlan::for_capabilities(source.unit_count, &capabilities, calibration)?;",
        '''let plan = DocumentScalePlan::for_capabilities(\n        source.unit_count,\n        &capabilities,\n        &task.reasoning_effort,\n        calibration,\n    )?;''',
        f"reasoning-aware scale plan in {path}",
    )
    replace_exact(
        path,
        '''        generation_windows.len(),\n        &capabilities,\n        calibration,\n    )?;''',
        '''        generation_windows.len(),\n        &capabilities,\n        &task.reasoning_effort,\n        calibration,\n    )?;''',
        f"reasoning-aware initial budget in {path}",
    ) if path.endswith("documents.rs") else replace_exact(
        path,
        '''        windows.len(),\n        &capabilities,\n        calibration,\n    )?;''',
        '''        windows.len(),\n        &capabilities,\n        &task.reasoning_effort,\n        calibration,\n    )?;''',
        f"reasoning-aware additional budget in {path}",
    )

# Tests that call the private helpers directly need the new explicit arguments
# and visible-output helper name.
path = Path("src-tauri/src/ai/document_intelligence.rs")
text = path.read_text()
text = text.replace(
    "assert_eq!(flashcard_completion_tokens(8), 4_096);",
    "assert_eq!(generation_visible_completion_tokens(8), 5_120);",
)
path.write_text(text)

replace_exact(
    "src-tauri/src/ai/probes.rs",
    '''        let capabilities = build_capabilities(\n            &profile,\n            true,\n            true,\n            true,\n            true,\n            &ProviderProperties::default(),\n        );''',
    '''        let capabilities = build_capabilities(\n            &profile,\n            None,\n            true,\n            true,\n            true,\n            true,\n            &ProviderProperties::default(),\n        );''',
    "probe capability regression model metadata argument",
)

print("Applied PR #54 document-output compile follow-up.")
