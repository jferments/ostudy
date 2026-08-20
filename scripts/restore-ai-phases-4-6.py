#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def remove_once(path: str, old: str, label: str) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    if old not in text:
        return
    target.write_text(text.replace(old, "", 1), encoding="utf-8")

# Phase 5 moves document reasoning out of device-wide AppSettings. The restored
# native/settings DTO already omits it; keep the clean Phase 1-3 controller
# instead of overwriting its navigation work and remove only the legacy field.
remove_once(
    "src/controllers/useAppController.ts",
    '  aiReasoningEffort: "low",\n',
    "legacy default AI reasoning setting",
)
remove_once(
    "src/controllers/useAppController.ts",
    '    aiReasoningEffort: settings.aiReasoningEffort ?? "low",\n',
    "legacy normalized AI reasoning setting",
)

print("Applied Phase 4-6 compatibility adjustments to the clean UI head.")
