#!/usr/bin/env python3
"""Remove final warning-only imports after Stage 3.3 native refactors."""

from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file_path = Path(path)
    text = file_path.read_text(encoding="utf-8")
    if text.count(old) != 1:
        raise SystemExit(f"expected exactly one match in {path}: {old!r}")
    file_path.write_text(text.replace(old, new, 1), encoding="utf-8")


replace_once(
    "src-tauri/src/cards.rs",
    "use rusqlite::{params, Connection, OptionalExtension};",
    "use rusqlite::{Connection, OptionalExtension};\n#[cfg(test)]\nuse rusqlite::params;",
)
replace_once(
    "src-tauri/src/cards.rs",
    "    util::{unix_time_millis, validate_entity_id},",
    "    util::validate_entity_id,",
)
replace_once(
    "src-tauri/src/cards.rs",
    "const MAX_DOCUMENT_BYTES: usize = 1_000_000;",
    "#[cfg(test)]\nuse crate::util::unix_time_millis;\n\nconst MAX_DOCUMENT_BYTES: usize = 1_000_000;",
)
replace_once(
    "src-tauri/src/asset_lifecycle.rs",
    "    io::{Read, Write},",
    "    io::Read,",
)
