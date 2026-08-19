#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
SELF = pathlib.Path(__file__).resolve()


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content)


# The broad retired-density cleanup intentionally removes the old Rust field,
# but an old multiline assertion can otherwise be left as assert_eq!(expr, ).
path = "src-tauri/src/settings/extended_tests.rs"
text = read(path)
text = re.sub(
    r'''\n\s*let mut value = settings\(\);\n\s*assert_eq!\(\n\s*validate_settings\(&value\)\.unwrap_err\(\)\.code,\n\s*\);''',
    "",
    text,
    count=1,
    flags=re.S,
)
write(path, text)

# document_intelligence_adapter is the public native facade used by document
# executors. Re-export only the durable semantic-coverage helpers that callers
# actually use; the strip helper remains private to the implementation/tests.
path = "src-tauri/src/ai/document_intelligence_adapter.rs"
text = read(path)
old = '''pub(crate) use implementation::{
    curate_document_cards, flashcard_completion_token_ceiling, generate_window_flashcards,
    load_document_task_context, DocumentOutline, DocumentTaskContext, FlashcardGenerationLimits,
    OutlineBuildSources, SourcePageImage, OUTLINE_PROMPT_VERSION, OUTLINE_SCHEMA_VERSION,
};'''
new = '''pub(crate) use implementation::{
    curate_document_cards, extract_internal_generation_metadata,
    flashcard_completion_token_ceiling, generate_window_flashcards,
    load_covered_learning_target_ids, load_document_task_context,
    persist_document_learning_targets, persist_learning_target_coverage, DocumentOutline,
    DocumentTaskContext, FlashcardGenerationLimits, OutlineBuildSources, SourcePageImage,
    OUTLINE_PROMPT_VERSION, OUTLINE_SCHEMA_VERSION,
};'''
if old in text:
    text = text.replace(old, new, 1)
elif "persist_document_learning_targets" not in text:
    raise RuntimeError(f"{path}: document-intelligence facade export anchor missing")
else:
    text = text.replace("    strip_internal_generation_metadata, DocumentOutline, DocumentTaskContext,\n", "    DocumentOutline, DocumentTaskContext,\n")
write(path, text)

# The final durable-coverage pass adds the exclusion field to test literals.
# Fully qualify it everywhere so tests do not depend on local HashSet imports,
# and remove the one accidental insertion into a helper function parameter list.
for rust_file in (ROOT / "src-tauri").rglob("*.rs"):
    source = rust_file.read_text()
    source = source.replace(
        "learning_target_exclusions: HashSet::new(),",
        "learning_target_exclusions: std::collections::HashSet::new(),",
    )
    source = re.sub(
        r'''(prompt_policy_hash:\s*&str,\n)\s*learning_target_exclusions:\s*std::collections::HashSet::new\(\),\n''',
        r"\1",
        source,
    )
    rust_file.write_text(source)

# Prompt helpers now take explicit learner-policy text. Any remaining no-argument
# calls are compatibility/test call sites; production request paths were already
# rewritten to pass the task-specific instructions.
path = "src-tauri/src/ai/document_intelligence.rs"
text = read(path)
text = text.replace("outline_system_prompt()", 'outline_system_prompt("")')
text = text.replace("flashcard_system_prompt()", 'flashcard_system_prompt("")')

# Avoid holding a rusqlite mapped-row temporary across statement destruction.
old = '''        statement
            .query_map(rusqlite::params![document_id, prompt_policy_hash], |row| row.get(0))
            .map_err(CommandError::database)?
            .collect::<Result<HashSet<_>, _>>()
            .map_err(CommandError::database)'''
new = '''        let rows = statement
            .query_map(rusqlite::params![document_id, prompt_policy_hash], |row| row.get(0))
            .map_err(CommandError::database)?;
        let covered = rows
            .collect::<Result<HashSet<_>, _>>()
            .map_err(CommandError::database)?;
        Ok(covered)'''
if old in text:
    text = text.replace(old, new, 1)
elif "let covered = rows" not in text:
    raise RuntimeError(f"{path}: covered-target query lifetime anchor missing")
write(path, text)

# Generate More needs both the legacy second-pass summary and the original
# outline for the durable learning-target ledger. Clone the small outline value
# before augmenting it rather than moving it away from the coverage planner.
path = "src-tauri/src/document_more.rs"
text = read(path)
text = text.replace(
    "let second_pass_outline = second_pass_outline(outline, &existing_coverage);",
    "let second_pass_outline = second_pass_outline(outline.clone(), &existing_coverage);",
    1,
)
write(path, text)

# Apply the same statement-lifetime pattern to native prompt-preset loading.
path = "src-tauri/src/settings.rs"
text = read(path)
old = '''    statement
        .query_map([], |row| {
            Ok(AiPromptPresetDto {
                id: row.get(0)?,
                name: row.get(1)?,
                outline_instructions: row.get(2)?,
                card_generation_instructions: row.get(3)?,
                revision: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(CommandError::database)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)'''
new = '''    let rows = statement
        .query_map([], |row| {
            Ok(AiPromptPresetDto {
                id: row.get(0)?,
                name: row.get(1)?,
                outline_instructions: row.get(2)?,
                card_generation_instructions: row.get(3)?,
                revision: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })
        .map_err(CommandError::database)?;
    let presets = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(CommandError::database)?;
    Ok(presets)'''
if old in text:
    text = text.replace(old, new, 1)
elif "let presets = rows" not in text:
    raise RuntimeError(f"{path}: prompt-preset query lifetime anchor missing")
write(path, text)

# This is one-shot scaffolding. Remove it before the qualified product tree is
# committed, just like the agent-pr55 applicators.
SELF.unlink()
