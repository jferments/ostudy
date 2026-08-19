#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
path = ROOT / "src-tauri/src/ai/document_intelligence.rs"
text = path.read_text()
pattern = re.compile(
    r"fn openrouter_document_body\s*\(\s*profile:\s*&ProfileSnapshot\s*,\s*request:\s*&JsonTaskRequest<'_>\s*,?\s*\)\s*->\s*Value\s*\{",
    flags=re.S,
)
replacement = """fn openrouter_document_body(
    profile: &ProfileSnapshot,
    request: &JsonTaskRequest<'_>,
) -> Value {"""
text, count = pattern.subn(replacement, text, count=1)
if count != 1 and "fn openrouter_document_body_with_routing" not in text:
    raise RuntimeError("OpenRouter document-body helper signature was not found")
path.write_text(text)
