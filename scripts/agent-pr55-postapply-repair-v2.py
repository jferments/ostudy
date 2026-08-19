#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content)


path = "src/controllers/useAppController.ts"
text = read(path)
helper_start = text.find("\nfunction normalizeDestination")
search_end = helper_start if helper_start >= 0 else len(text)
return_start = text.rfind("  return {", 0, search_end)
if return_start < 0:
    raise RuntimeError(f"{path}: useAppController return object not found")
return_end = text.find("\n  };", return_start, search_end)
if return_end < 0:
    raise RuntimeError(f"{path}: useAppController return boundary not found")
return_block = text[return_start:return_end]
if "sectionRootRevision," not in return_block:
    anchor = "    flashcardsWorkspace,\n"
    relative = return_block.find(anchor)
    if relative < 0:
        raise RuntimeError(f"{path}: flashcardsWorkspace return field not found")
    absolute = return_start + relative
    text = text[:absolute] + text[absolute:].replace(
        anchor,
        anchor + "    sectionRootRevision,\n",
        1,
    )
write(path, text)

path = "src-tauri/src/ai/document_provider.rs"
text = read(path)
pattern = r"#\[derive\(([^\)]*)\)\]\n((?:pub(?:\(crate\))?\s+)?)struct DocumentProviderCapabilities"
match = re.search(pattern, text)
if not match:
    raise RuntimeError(f"{path}: DocumentProviderCapabilities derive not found")
derives = [value.strip() for value in match.group(1).split(",")]
if "Clone" not in derives:
    derives.insert(0, "Clone")
    replacement = f"#[derive({', '.join(derives)})]\n{match.group(2)}struct DocumentProviderCapabilities"
    text = text[: match.start()] + replacement + text[match.end() :]
write(path, text)

path = "src-tauri/src/ai/document_intelligence.rs"
text = read(path)
text = re.sub(
    r"providers::openrouter_provider_preferences\(\s*true,\s*profile\.require_zero_data_retention,?\s*\)",
    "providers::openrouter_provider_preferences_with_routing(\n            true,\n            profile.require_zero_data_retention,\n            routing_mode,\n            provider_slugs,\n        )",
    text,
)
text = re.sub(
    r"openrouter_document_body\(\s*&task\.profile,\s*(&?request)\s*\)",
    r"openrouter_document_body_with_routing(\n                &task.openrouter_routing_mode,\n                &task.openrouter_provider_slugs,\n                &task.profile,\n                \1,\n            )",
    text,
)
write(path, text)

path = "src/styles/base/application.css"
text = read(path)
text = text.replace(
    ".card-row-preview > span { display: flex; min-width: 0; align-items: baseline; gap: .45rem; }",
    ".card-row-preview > span { display: flex; width: 100%; max-width: 100%; min-width: 0; align-items: baseline; gap: .45rem; }",
)
write(path, text)

path = "scripts/agent-pr55-test-refresh.py"
text = read(path)
anchor = 'for (const contract of ["DocumentLearningTarget", "document_generation_target_gap_pass_started", "strip_internal_generation_metadata", "prompt_policy_hash"]) {'
if anchor in text:
    text = text.replace(
        anchor,
        'for (const contract of ["DocumentLearningTarget", "document_generation_target_gap_pass_started", "strip_internal_generation_metadata", "prompt_policy_hash", "openrouter_provider_preferences_with_routing"]) {',
        1,
    )
write(path, text)
