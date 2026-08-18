#!/usr/bin/env python3
"""Restore the pre-existing card-coverage budgets after removing wall-clock deadlines."""
from pathlib import Path

path = Path("src-tauri/src/document_scale.rs")
value = path.read_text()
old_standard = "            max_cards_per_window: 10,"
old_large = "        max_cards_per_window: 10,"
if old_standard not in value or old_large not in value:
    raise SystemExit("document card-capacity anchors were not found")
value = value.replace(old_standard, "            max_cards_per_window: 12,", 1)
value = value.replace(old_large, "        max_cards_per_window: 16,", 1)
value = value.replace(
    "            assert_eq!(plan.max_cards_per_window, 10);",
    "            assert_eq!(plan.max_cards_per_window, 16);",
    1,
)
path.write_text(value)
print("Restored standard/large document card budgets to 12/16.")
