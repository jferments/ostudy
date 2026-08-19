#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
for relative, title in [
    ("docs/architecture/document-ai-routing.md", "# Document AI routing\n"),
    ("docs/architecture/ui.md", "# UI architecture\n"),
    ("docs/README.md", "# ostudy documentation\n"),
]:
    path = ROOT / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        path.write_text(title)
