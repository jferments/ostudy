#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
SELF = pathlib.Path(__file__).resolve()
path = ROOT / "src-tauri/src/ai/document_intelligence.rs"
source = path.read_text()

old_direction = "Ordinary basic question/answer cards, multiple-choice cards, and critical-thinking/application cards are one-way. Use a meaningful variety when supported:"
new_direction = "Ordinary basic question/answer cards, multiple-choice cards, and critical-thinking/application cards are one-way question-to-answer cards and are never reversible. Never put an explanatory answer or descriptive passage on the question side with its question on the answer side. Use a meaningful variety when supported:"
if old_direction in source:
    source = source.replace(old_direction, new_direction, 1)
elif "are never reversible" not in source or "question side" not in source:
    raise RuntimeError("document_intelligence.rs: flashcard direction guardrail anchor missing")

old_heading = "If a question asks multiple linked parts such as “what ... and why/how?”, the answer must explicitly supply every part. Never ask for a numerical value, mechanism, relationship, or clinical implication unless the answer actually states it."
new_heading = "If a question asks multiple linked parts such as “what ... and why/how?”, the answer must explicitly supply every part. Never use a chapter heading, subsection title, figure label, or topic phrase such as “Blood-Gas Barrier Thickness” as the answer to a question asking for a thickness value and its significance. Never ask for a numerical value, mechanism, relationship, or clinical implication unless the answer actually states it."
if old_heading in source:
    source = source.replace(old_heading, new_heading, 1)
elif "Blood-Gas Barrier Thickness" not in source:
    raise RuntimeError("document_intelligence.rs: anti-heading prompt guardrail anchor missing")

path.write_text(source)
SELF.unlink()
