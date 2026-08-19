#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]


def patch(path: str, old: str, new: str) -> None:
    file = ROOT / path
    text = file.read_text()
    if new in text:
        return
    if old not in text:
        raise RuntimeError(f"{path}: repair anchor not found")
    file.write_text(text.replace(old, new, 1))


# Correct the DTO type used by the generated API-key placeholder.
file = ROOT / "scripts/agent-pr55-ai-settings.py"
text = file.read_text()
text = text.replace("  AiCredentialStatus,\n", "  AiCredentialStatusInfo,\n")
text = text.replace(
    "  status: AiCredentialStatus | undefined,",
    "  status: AiCredentialStatusInfo | undefined,",
)
file.write_text(text)

# Some callers previously relied on the optional reasoning default and therefore
# have no literal aiReasoningEffort insertion anchor. Make the generated settings
# defaults tolerant while still writing a complete explicit policy.
patch(
    "scripts/agent-pr55-ai-settings.py",
    '''for path in ["src/controllers/useAppController.ts", "src/screens/SettingsScreen.tsx"]:
    text = read(path)
    marker = '  aiReasoningEffort: "low",\\n'
    addition = marker + '  aiContextLimitTokens: null,\\n  aiImageInputPolicy: "automatic",\\n  aiCoverageMode: "balanced",\\n  aiOpenRouterRoutingMode: "automatic",\\n  aiOpenRouterProviderSlugs: [],\\n  aiPromptPresetId: null,\\n  aiPromptPresets: [],\\n'
    if "aiContextLimitTokens" not in text:
        text = replace_once(text, marker, addition, path)
    write(path, text)
''',
    '''for path in ["src/controllers/useAppController.ts", "src/screens/SettingsScreen.tsx"]:
    text = read(path)
    marker = '  aiReasoningEffort: "low",\\n'
    policy_fields = '  aiContextLimitTokens: null,\\n  aiImageInputPolicy: "automatic",\\n  aiCoverageMode: "balanced",\\n  aiOpenRouterRoutingMode: "automatic",\\n  aiOpenRouterProviderSlugs: [],\\n  aiPromptPresetId: null,\\n  aiPromptPresets: [],\\n'
    if "aiContextLimitTokens" not in text:
        if marker in text:
            text = text.replace(marker, marker + policy_fields, 1)
        else:
            default_start = text.find("export const DEFAULT_SETTINGS")
            if default_start < 0:
                default_start = text.find("const DEFAULTS: AppSettings")
            default_end = text.find("\\n};", default_start)
            if default_start < 0 or default_end < 0:
                raise RuntimeError(f"{path}: settings-default object not found")
            reasoning = '  aiReasoningEffort: "low",\\n'
            text = text[:default_end] + "\\n" + reasoning + policy_fields.rstrip("\\n") + text[default_end:]
    write(path, text)
''',
)

# Keep owned strings available after hashing the task policy.
file = ROOT / "scripts/agent-pr55-document-policy.py"
text = file.read_text()
text = text.replace('"presetId": settings.ai_prompt_preset_id,', '"presetId": settings.ai_prompt_preset_id.as_deref(),')
text = text.replace('"outline": outline_instructions,', '"outline": outline_instructions.as_str(),')
text = text.replace('"cards": card_generation_instructions,', '"cards": card_generation_instructions.as_str(),')
text = text.replace('"coverage": settings.ai_coverage_mode,', '"coverage": settings.ai_coverage_mode.as_str(),')
text = text.replace('"imageInput": settings.ai_image_input_policy,', '"imageInput": settings.ai_image_input_policy.as_str(),')
text = text.replace('"openRouterRouting": settings.ai_openrouter_routing_mode,', '"openRouterRouting": settings.ai_openrouter_routing_mode.as_str(),')
text = text.replace('"openRouterProviders": settings.ai_openrouter_provider_slugs,', '"openRouterProviders": &settings.ai_openrouter_provider_slugs,')
file.write_text(text)

# Make final curation metadata stripping independent of the exact left-hand-side
# formatting used in either initial or Generate More execution.
patch(
    "scripts/agent-pr55-semantic-targets.py",
    '''def insert_strip(path: str) -> None:
    text = read(path)
    marker = "let generated = document_intelligence::curate_document_cards("
    if marker not in text and "let mut generated = document_intelligence::curate_document_cards(" not in text:
        raise RuntimeError(f"{path}: curation assignment not found")
    text = text.replace(marker, "let mut generated = document_intelligence::curate_document_cards(", 1)
    start = text.find("let mut generated = document_intelligence::curate_document_cards(")
    await_marker = ".await?;"
    await_index = text.find(await_marker, start)
    if await_index < 0:
        raise RuntimeError(f"{path}: curation await boundary not found")
    insert_at = await_index + len(await_marker)
    strip = "\\n    document_intelligence::strip_internal_generation_metadata(&mut generated);"
    if strip.strip() not in text[start:insert_at + 220]:
        text = text[:insert_at] + strip + text[insert_at:]
    write(path, text)
''',
    '''def insert_strip(path: str) -> None:
    text = read(path)
    call = "document_intelligence::curate_document_cards("
    start = text.find(call)
    if start < 0:
        raise RuntimeError(f"{path}: curation call not found")
    assignment_start = text.rfind("let ", 0, start)
    assignment_end = text.find("=", assignment_start, start)
    if assignment_start < 0 or assignment_end < 0:
        raise RuntimeError(f"{path}: curation assignment not found")
    assignment = text[assignment_start:assignment_end]
    variable_match = re.search(r"let(?:\\s+mut)?\\s+([A-Za-z_][A-Za-z0-9_]*)", assignment)
    if not variable_match:
        raise RuntimeError(f"{path}: curation result variable not found")
    variable = variable_match.group(1)
    if "let mut" not in assignment:
        text = text[:assignment_start] + text[assignment_start:assignment_end].replace("let ", "let mut ", 1) + text[assignment_end:]
        start = text.find(call, assignment_start)
    await_marker = ".await?;"
    await_index = text.find(await_marker, start)
    if await_index < 0:
        raise RuntimeError(f"{path}: curation await boundary not found")
    insert_at = await_index + len(await_marker)
    strip = f"\\n    document_intelligence::strip_internal_generation_metadata(&mut {variable});"
    if strip.strip() not in text[start:insert_at + 260]:
        text = text[:insert_at] + strip + text[insert_at:]
    write(path, text)
''',
)

# The permanent verifier must tolerate formatter line wrapping around compact
# Q/A labels.
file = ROOT / "scripts/agent-pr55-test-refresh.py"
text = file.read_text()
text = text.replace(
    '''requireText(decks.includes('aria-hidden="true">Q:'), "Compact question preview is missing");
requireText(decks.includes('aria-hidden="true">A:'), "Compact answer preview is missing");''',
    '''requireText(/aria-hidden="true"[^>]*>\\s*Q:/s.test(decks), "Compact question preview is missing");
requireText(/aria-hidden="true"[^>]*>\\s*A:/s.test(decks), "Compact answer preview is missing");''',
)
file.write_text(text)

# Repair the native setup applicator before it runs: align with the existing IPC
# suite title and insert setup helpers into the ./aiProfiles import block rather
# than accidentally rewriting @tauri-apps/api/core.
file = ROOT / "scripts/agent-pr55-native-ai-setup.py"
text = file.read_text()
text = text.replace(
    'anchor = "describe(\\\"aiProfiles IPC\\\", () => {\\n"',
    'anchor = "describe(\\\"AI provider IPC\\\", () => {\\n"',
)
old_import_patch = '''    text = text.replace(
        'import {',
        'import {\\n  beginAiProviderSetup,\\n  cancelAiProviderSetup,\\n  commitAiProviderSetup,',
        1,
    )'''
new_import_patch = '''    text = text.replace(
        'import {\\n  clearAiProviderCredential,',
        'import {\\n  beginAiProviderSetup,\\n  cancelAiProviderSetup,\\n  commitAiProviderSetup,\\n  clearAiProviderCredential,',
        1,
    )'''
if old_import_patch in text:
    text = text.replace(old_import_patch, new_import_patch, 1)
elif new_import_patch not in text:
    raise RuntimeError("native AI setup IPC import repair target not found")

# First-model default assignment must tolerate a restored database whose seeded
# task-default row is absent. Use insert-or-update semantics but preserve an
# existing explicit default.
old_default_sql = '''        transaction
            .execute(
                "UPDATE ai_task_defaults
                 SET profile_id = CASE WHEN profile_id IS NULL THEN ?1 ELSE profile_id END,
                     updated_at = CASE WHEN profile_id IS NULL THEN ?2 ELSE updated_at END
                 WHERE task_kind = 'flashcard_generation'",
                params![input.profile_id, now],
            )'''
new_default_sql = '''        transaction
            .execute(
                "INSERT INTO ai_task_defaults (task_kind, profile_id, updated_at)
                 VALUES ('flashcard_generation', ?1, ?2)
                 ON CONFLICT(task_kind) DO UPDATE SET
                     profile_id = CASE
                         WHEN ai_task_defaults.profile_id IS NULL THEN excluded.profile_id
                         ELSE ai_task_defaults.profile_id
                     END,
                     updated_at = CASE
                         WHEN ai_task_defaults.profile_id IS NULL THEN excluded.updated_at
                         ELSE ai_task_defaults.updated_at
                     END",
                params![input.profile_id, now],
            )'''
if old_default_sql in text:
    text = text.replace(old_default_sql, new_default_sql, 1)
elif new_default_sql not in text:
    raise RuntimeError("native AI setup default-assignment repair target not found")
file.write_text(text)
