#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: pathlib.Path | str) -> str:
    path = ROOT / path if isinstance(path, str) else path
    return path.read_text()


def write(path: pathlib.Path | str, content: str) -> None:
    path = ROOT / path if isinstance(path, str) else path
    path.write_text(content)


def matching_brace(text: str, opening: int) -> int:
    depth = 0
    state = "normal"
    raw_hashes = 0
    index = opening
    while index < len(text):
        character = text[index]
        next_character = text[index + 1] if index + 1 < len(text) else ""
        if state == "line_comment":
            if character == "\n":
                state = "normal"
        elif state == "block_comment":
            if character == "*" and next_character == "/":
                state = "normal"
                index += 1
        elif state == "string":
            if character == "\\":
                index += 1
            elif character == '"':
                state = "normal"
        elif state == "char":
            if character == "\\":
                index += 1
            elif character == "'":
                state = "normal"
        elif state == "raw":
            if character == '"' and text[index + 1 : index + 1 + raw_hashes] == "#" * raw_hashes:
                index += raw_hashes
                state = "normal"
        else:
            if character == "/" and next_character == "/":
                state = "line_comment"
                index += 1
            elif character == "/" and next_character == "*":
                state = "block_comment"
                index += 1
            elif character == "r":
                match = re.match(r'r(#{0,16})"', text[index:])
                if match:
                    raw_hashes = len(match.group(1))
                    state = "raw"
                    index += len(match.group(0)) - 1
            elif character == '"':
                state = "string"
            elif character == "'":
                # Lifetimes are followed by identifiers; character literals are
                # followed by a closing quote within a few bytes.
                closing = text.find("'", index + 1, min(len(text), index + 8))
                if closing >= 0:
                    state = "char"
            elif character == "{":
                depth += 1
            elif character == "}":
                depth -= 1
                if depth == 0:
                    return index
        index += 1
    raise RuntimeError("unbalanced Rust brace while updating test fixtures")


def populate_rust_literals(type_name: str, fields: str) -> None:
    for file in (ROOT / "src-tauri").rglob("*.rs"):
        text = file.read_text()
        cursor = 0
        changed = False
        while True:
            found = text.find(f"{type_name} {{", cursor)
            if found < 0:
                break
            line_start = text.rfind("\n", 0, found) + 1
            prefix = text[line_start:found]
            opening = found + len(type_name) + 1
            if "struct " in prefix or "->" in prefix:
                cursor = opening + 1
                continue
            closing = matching_brace(text, opening)
            block = text[opening:closing]
            first_field = fields.strip().split(":", 1)[0]
            if first_field in block:
                cursor = closing + 1
                continue
            indent_match = re.match(r"[ \t]*", text[line_start:])
            indent = (indent_match.group(0) if indent_match else "") + "    "
            rendered = "\n" + "\n".join(
                indent + line if line.strip() else line for line in fields.strip().splitlines()
            )
            text = text[: opening + 1] + rendered + text[opening + 1 :]
            changed = True
            cursor = opening + len(rendered) + 1
        if changed:
            file.write_text(text)


populate_rust_literals(
    "AppSettingsDto",
    '''ai_context_limit_tokens: None,
ai_image_input_policy: "automatic".to_string(),
ai_coverage_mode: "balanced".to_string(),
ai_openrouter_routing_mode: "automatic".to_string(),
ai_openrouter_provider_slugs: Vec::new(),
ai_prompt_preset_id: None,
ai_prompt_presets: Vec::new(),''',
)
populate_rust_literals(
    "DocumentTaskContext",
    '''context_limit_tokens: None,
image_input_policy: "automatic".to_string(),
coverage_mode: "balanced".to_string(),
outline_instructions: String::new(),
card_generation_instructions: String::new(),
prompt_policy_hash: "test-policy".to_string(),
openrouter_routing_mode: "automatic".to_string(),
openrouter_provider_slugs: Vec::new(),''',
)
populate_rust_literals(
    "CheckpointIdentity",
    '''prompt_policy_hash: "test-policy".to_string(),''',
)

# Remove only the retired density and Create Cards component APIs from bundled
# fixtures. The ordinary word "create" remains valid for deck/folder/dialog
# modes and for legacy checkpoint normalization, so it must never be stripped
# globally from TypeScript source.
for file in (ROOT / "src").rglob("*"):
    if file.suffix not in {".ts", ".tsx"}:
        continue
    text = file.read_text()
    updated = re.sub(r'^\s*deckViewDensity:\s*"(?:comfortable|compact)",?\n', "", text, flags=re.M)
    updated = re.sub(r'^\s*onCreateCards=\{[^\n]*\}\n', "", updated, flags=re.M)
    updated = re.sub(r'^\s*onBulkAdd=\{[^\n]*\}\n', "", updated, flags=re.M)
    if updated != text:
        file.write_text(updated)

for file in (ROOT / "src").rglob("*CreateCardsWorkspace*"):
    if file.is_file():
        file.unlink()

# Keep direct PracticeSetup fixtures source-compatible while the screen itself
# still supplies explicit state. Destructure workload before deriving the
# optional status default so TypeScript never observes a use-before-binding.
path = ROOT / "src/screens/practice/PracticeSetup.tsx"
text = path.read_text()
text = text.replace(
    '  workloadStatus: "idle" | "loading" | "loaded" | "error";',
    '  workloadStatus?: "idle" | "loading" | "loaded" | "error";',
)
text = text.replace(
    "  workloadStatus,\n  workload,",
    '  workload,\n  workloadStatus = workload ? "loaded" : isLoadingWorkload ? "loading" : "idle",',
)
path.write_text(text)

# Re-selecting Practice deliberately follows the same termination path as any
# other primary-root navigation. Prefix the retained compatibility parameter so
# the strict no-unused-parameter gate documents that intent.
path = ROOT / "src/controllers/useAppController.ts"
text = path.read_text().replace(
    "async function confirmPracticeTermination(next: AppSection): Promise<boolean>",
    "async function confirmPracticeTermination(_next: AppSection): Promise<boolean>",
    1,
)
path.write_text(text)

# The settings defaults intentionally changed for new installations.
for file in (ROOT / "src-tauri").rglob("*.rs"):
    text = file.read_text()
    updated = text.replace("assert!(settings.tutorials_enabled);", "assert!(!settings.tutorials_enabled);")
    updated = re.sub(r'^.*deck_view_density.*\n', "", updated, flags=re.M)
    if updated != text:
        file.write_text(updated)

# Preserve the valid Automatic choice without duplicating it in the advertised
# reasoning-effort options.
path = ROOT / "src/screens/settings/DefaultAiModelSettings.tsx"
text = path.read_text()
text = text.replace(
    '''  const visibleEfforts = allowedEfforts.includes(selectedEffort as AiReasoningEffort)
    ? allowedEfforts
    : [selectedEffort as AiReasoningEffort, ...allowedEfforts];''',
    '''  const visibleEfforts =
    selectedEffort === "automatic" || allowedEfforts.includes(selectedEffort as AiReasoningEffort)
      ? allowedEfforts
      : [selectedEffort as AiReasoningEffort, ...allowedEfforts];''',
)
path.write_text(text)

# Add focused route compatibility coverage after removing the old workspace.
route_test = ROOT / "src/navigation/flashcardsRoutes.test.ts"
route_test.write_text(
    '''import { describe, expect, it } from "vitest";\n\nimport { flashcardsWorkspaceNames, isFlashcardsWorkspaceName } from "./flashcardsRoutes";\n\ndescribe("flashcards routes", () => {\n  it("contains only current root workspaces", () => {\n    expect(flashcardsWorkspaceNames).toEqual(["decks", "templates", "search", "transfer"]);\n    expect(isFlashcardsWorkspaceName("create")).toBe(false);\n  });\n});\n'''
)
