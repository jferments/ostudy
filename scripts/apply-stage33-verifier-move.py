#!/usr/bin/env python3
"""Teach the historical Stage 2 verifier the new deletion command module."""

from pathlib import Path

path = Path("scripts/verify-stage2.mjs")
text = path.read_text(encoding="utf-8")
old = '''  readJoined(
    ["src-tauri", "src", "authoring.rs"],
    ["src-tauri", "src", "authoring", "types.rs"],'''
new = '''  readJoined(
    ["src-tauri", "src", "authoring.rs"],
    ["src-tauri", "src", "deletion_commands.rs"],
    ["src-tauri", "src", "authoring", "types.rs"],'''
if text.count(old) != 1:
    raise SystemExit("Stage 2 authoring boundary anchor was not found exactly once")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
