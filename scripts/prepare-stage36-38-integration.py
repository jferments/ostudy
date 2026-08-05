#!/usr/bin/env python3
"""Prepare the readable integration applicator without deleting shared helpers."""
from pathlib import Path

path = Path(__file__).resolve().with_name("apply-stage36-38-integration.py")
content = path.read_text()
old = 'start = content.find("#[tauri::command]\\npub fn list_card_templates(")'
if old not in content:
    raise SystemExit("list-card-template applicator marker is missing")
path.write_text(content.replace(old, "start = -1", 1))
print("Prepared integration applicator.")
