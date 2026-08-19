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
