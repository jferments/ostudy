#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]

path = ROOT / "src-tauri/src/device_sync/merge.rs"
text = path.read_text()
entry = '    TableSpec::new("ai_prompt_presets", &["id"]),\n'
if entry not in text:
    anchor = '''    TableSpec {
        name: "ai_provider_profiles",'''
    if anchor not in text:
        raise RuntimeError("device sync AI table anchor not found")
    text = text.replace(anchor, entry + anchor, 1)
path.write_text(text)

path = ROOT / "docs/architecture/device-sync.md"
text = path.read_text()
addition = '''
### AI prompt presets versus device-specific provider policy

Custom outline/card-generation prompt presets are learner-owned study configuration and synchronize through the normal three-way row merge. Provider profiles remain in the existing explicit API-settings scope and credentials remain outside SQLite. `ai_task_policies` is intentionally not a synchronized table: reasoning/context limits, OpenRouter routing choices, page-image policy, and the selected local profile/preset assignment may differ by device. A received preset becomes available locally without silently changing the receiving device's AI provider or generation policy.
'''
if "### AI prompt presets versus device-specific provider policy" not in text:
    text += addition
path.write_text(text)
