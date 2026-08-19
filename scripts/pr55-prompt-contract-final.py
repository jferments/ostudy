#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
SELF = pathlib.Path(__file__).resolve()
path = ROOT / "src-tauri/src/ai/document_intelligence.rs"
source = path.read_text()

old = "Ordinary basic question/answer cards, multiple-choice cards, and critical-thinking/application cards are one-way. Use a meaningful variety when supported:"
new = "Ordinary basic question/answer cards, multiple-choice cards, and critical-thinking/application cards are one-way question-to-answer cards and are never reversible. Never put an explanatory answer or descriptive passage on the question side with its question on the answer side. Use a meaningful variety when supported:"

if old in source:
    source = source.replace(old, new, 1)
elif "are never reversible" not in source or "question side" not in source:
    raise RuntimeError("document_intelligence.rs: flashcard direction guardrail anchor missing")

path.write_text(source)
SELF.unlink()
