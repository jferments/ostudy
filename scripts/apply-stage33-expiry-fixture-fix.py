#!/usr/bin/env python3
"""Correct the Stage 3.3 expired-operation fixture without bypassing schema checks."""

from pathlib import Path

path = Path("src-tauri/src/deletion.rs")
text = path.read_text(encoding="utf-8")
old = '"UPDATE deletion_operations SET expires_at = 0 WHERE id = ?1"'
new = '''"UPDATE deletion_operations
                         SET deleted_at = 1, expires_at = 1, created_at = 1
                         WHERE id = ?1"'''
if text.count(old) != 1:
    raise SystemExit("expired deletion fixture anchor was not found exactly once")
path.write_text(text.replace(old, new, 1), encoding="utf-8")
