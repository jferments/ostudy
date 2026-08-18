#!/usr/bin/env python3
"""Disambiguate the global-outline max-token patch before the main PR #54 applicator."""
from pathlib import Path

path = Path("src-tauri/src/ai/document_intelligence.rs")
text = path.read_text()
old = '''            schema: &schema,
            max_tokens: 1_536,
            images: &[],
        },
    )
    .await
    .and_then(|value| {
        serde_json::from_value(value).map_err(|error| {'''
new = '''            schema: &schema,
            max_tokens: task.completion_token_ceiling(GLOBAL_OUTLINE_VISIBLE_COMPLETION_TOKENS),
            images: &[],
        },
    )
    .await
    .and_then(|value| {
        serde_json::from_value(value).map_err(|error| {'''
if new not in text:
    if text.count(old) != 1:
        raise SystemExit(f"global outline output anchor count was {text.count(old)}, expected 1")
    text = text.replace(old, new, 1)
path.write_text(text)
print("Disambiguated the global outline visible-output budget patch.")
