#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content)


# Ensure the explicit root-reset revision is part of the controller's public
# return object after all legacy navigation transformations.
path = "src/controllers/useAppController.ts"
text = read(path)
return_start = text.rfind("  return {")
if return_start < 0:
    raise RuntimeError(f"{path}: controller return object not found")
return_end = text.find("\n  };", return_start)
if return_end < 0:
    raise RuntimeError(f"{path}: controller return boundary not found")
return_block = text[return_start:return_end]
if "sectionRootRevision," not in return_block:
    anchor = "    flashcardsWorkspace,\n"
    if anchor not in return_block:
        raise RuntimeError(f"{path}: flashcardsWorkspace return field not found")
    replacement = anchor + "    sectionRootRevision,\n"
    absolute = return_start + return_block.find(anchor)
    text = text[:absolute] + text[absolute:].replace(anchor, replacement, 1)
write(path, text)

# Document policy planning takes a value snapshot, so the capability contract
# must be cheaply cloneable even when a provider adapter grows new fields.
path = "src-tauri/src/ai/document_provider.rs"
text = read(path)
pattern = r"#\[derive\(([^\)]*)\)\]\n(pub\(crate\) )?struct DocumentProviderCapabilities"
match = re.search(pattern, text)
if not match:
    raise RuntimeError(f"{path}: DocumentProviderCapabilities derive not found")
derives = [value.strip() for value in match.group(1).split(",")]
if "Clone" not in derives:
    derives.insert(0, "Clone")
    replacement = f"#[derive({', '.join(derives)})]\n{match.group(2) or ''}struct DocumentProviderCapabilities"
    text = text[: match.start()] + replacement + text[match.end() :]
write(path, text)

# Apply the selected OpenRouter host policy to production document bodies while
# retaining the old helper only for default-policy tests.
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

# Keep the compact Q/A row as a two-line content surface at phone widths while
# preserving a separate accessible action target.
path = "src/styles/base/application.css"
text = read(path)
if ".card-row-preview > span" in text and "max-width: 100%" not in text[text.find(".card-row-preview > span"):text.find(".card-row-preview > span") + 300]:
    text = text.replace(
        ".card-row-preview > span { display: flex; min-width: 0; align-items: baseline; gap: .45rem; }",
        ".card-row-preview > span { display: flex; width: 100%; max-width: 100%; min-width: 0; align-items: baseline; gap: .45rem; }",
        1,
    )
write(path, text)

# Protect the real routed call in the permanent source verifier.
path = "scripts/agent-pr55-test-refresh.py"
text = read(path)
anchor = 'for (const contract of ["DocumentLearningTarget", "document_generation_target_gap_pass_started", "strip_internal_generation_metadata", "prompt_policy_hash"]) {'
if anchor in text and "openrouter_provider_preferences_with_routing" not in text:
    text = text.replace(
        anchor,
        'for (const contract of ["DocumentLearningTarget", "document_generation_target_gap_pass_started", "strip_internal_generation_metadata", "prompt_policy_hash", "openrouter_provider_preferences_with_routing"]) {',
        1,
    )
write(path, text)
