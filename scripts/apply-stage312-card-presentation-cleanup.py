#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
path = ROOT / "src-tauri/src/cards.rs"
text = path.read_text(encoding="utf-8")

text = text.replace(
    "//! Shared card hydration, deletion, and rich-text validation.\n",
    "//! Shared card-presentation transport and rich-text validation.\n",
    1,
)
text = text.replace(
    "use rusqlite::{Connection, OptionalExtension};\n",
    "use rusqlite::Connection;\n",
    1,
)
text = text.replace(
    "    assets::{get_assets_for_note, AssetDto},\n",
    "    assets::AssetDto,\n",
    1,
)
text = text.replace("    paths::AppPaths,\n", "", 1)

start = text.index("struct RawCard {\n")
end = text.index("pub(crate) fn validate_content(\n", start)
removed = text[start:end]
for expected in ["struct RawCard", "get_card_from_connection", "hydrate_card", "raw_card_from_row"]:
    if expected not in removed:
        raise SystemExit(f"expected stale hydrator symbol missing from removal block: {expected}")
text = text[:start] + text[end:]

for obsolete in ["RawCard", "get_card_from_connection", "hydrate_card", "raw_card_from_row", "get_assets_for_note", "AppPaths", "OptionalExtension"]:
    if obsolete in text:
        raise SystemExit(f"stale card hydrator residue remains: {obsolete}")

path.write_text(text, encoding="utf-8")
print("Removed obsolete forward-card hydrator after CardNote/CardPresentation split.")
