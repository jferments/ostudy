#!/usr/bin/env python3
from pathlib import Path


def remove_once(path: str, text: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    if text not in value:
        return
    p.write_text(value.replace(text, "", 1))


def replace_once(path: str, old: str, new: str, label: str) -> None:
    p = Path(path)
    value = p.read_text()
    if old not in value:
        if new in value:
            return
        raise SystemExit(f"{label}: anchor not found in {path}")
    p.write_text(value.replace(old, new, 1))


remove_once(
    "src-tauri/src/document_generation.rs",
    "use tokio::time::timeout;\n",
    "stale generation timeout import",
)
remove_once(
    "src-tauri/src/document_generation.rs",
    "const GENERATION_PROVIDER_ATTEMPT_SECONDS: u64 = 180;\n",
    "stale generation deadline constant",
)
remove_once(
    "src-tauri/src/ai/document_intelligence_adapter.rs",
    "const OUTLINE_PROVIDER_ATTEMPT_SECONDS: u64 = 120;\n",
    "stale outline deadline constant",
)
replace_once(
    "src-tauri/src/document_scale.rs",
    "        max_cards_per_window: 16,",
    "        max_cards_per_window: 10,",
    "large-document card budget",
)
print("PR #54 resilience cleanup applied.")
