#!/usr/bin/env python3
"""One-shot helper for connector-authored Stage 3.3 native compile fixes.

This file is intentionally temporary. It removes superseded pre-Undo delete
paths and materializes rusqlite query results before prepared statements drop.
"""

from __future__ import annotations

import re
from pathlib import Path


class FixError(RuntimeError):
    pass


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise FixError(f"{label}: expected one exact match in {path}, found {count}")
    path.write_text(text.replace(old, new, 1), encoding="utf-8")


def regex_once(path: Path, pattern: str, replacement: str, label: str) -> None:
    text = path.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.DOTALL)
    if count != 1:
        raise FixError(f"{label}: expected one regex match in {path}, found {count}")
    path.write_text(updated, encoding="utf-8")


def remove_superseded_delete_paths() -> None:
    cards = Path("src-tauri/src/cards.rs")
    regex_once(
        cards,
        r"\n#\[tauri::command\]\npub fn delete_card\(.*?\n\}\n",
        "\n",
        "remove old delete_card IPC",
    )
    regex_once(
        cards,
        r"\nfn delete_card_in_connection\(.*?\n\}\n\n(?=fn hydrate_card)",
        "\n",
        "remove old card tombstone helper",
    )
    text = cards.read_text(encoding="utf-8")
    text = text.replace(
        "    util::{unix_time_millis, validate_entity_id},",
        "    util::validate_entity_id,",
    )
    text = text.replace("use rusqlite::{params, Connection, OptionalExtension};", "use rusqlite::{Connection, OptionalExtension};")
    cards.write_text(text, encoding="utf-8")

    decks = Path("src-tauri/src/decks.rs")
    regex_once(
        decks,
        r"\n#\[tauri::command\]\npub fn delete_deck\(.*?\n\}\n",
        "\n",
        "remove old delete_deck IPC",
    )
    regex_once(
        decks,
        r"\nfn delete_deck_in_connection\(.*?\n\}\n\n(?=pub\(crate\) fn validate_name)",
        "\n",
        "remove old deck tombstone helper",
    )


def fix_asset_queries() -> None:
    path = Path("src-tauri/src/asset_lifecycle.rs")
    replace_once(
        path,
        "    let retained = normalize_asset_ids(input.asset_ids)?;",
        "    let retained = normalize_asset_ids(input.asset_ids)?\n"
        "        .into_iter()\n"
        "        .collect::<BTreeSet<_>>();",
        "materialize retained asset IDs",
    )

    regex_once(
        path,
        r"(let asset_ids = \{\n\s+let mut statement = .*?\.map_err\(CommandError::database\)\?;)\n"
        r"\s+statement\n(\s+\.query_map\(\[\], \|row\| row\.get::<_, String>\(0\)\).*?"
        r"\.map_err\(CommandError::database\)\?)\n\s+\};",
        r"\1\n        let rows = statement\n\2;\n        rows\n    };",
        "materialize reconciled asset IDs",
    )
    regex_once(
        path,
        r"(let draft_ids = \{\n\s+let mut statement = .*?\.map_err\(CommandError::database\)\?;)\n"
        r"\s+statement\n(\s+\.query_map\(\[now\], \|row\| row\.get::<_, String>\(0\)\).*?"
        r"\.map_err\(CommandError::database\)\?)\n\s+\};",
        r"\1\n        let rows = statement\n\2;\n        rows\n    };",
        "materialize expired draft IDs",
    )
    regex_once(
        path,
        r"(fn load_assets_for_draft\(.*?let mut statement = .*?\.map_err\(CommandError::database\)\?;)\n"
        r"\s+statement\n(\s+\.query_map\(\[draft_id\],.*?"
        r"\.map_err\(CommandError::database\))\n\}",
        r"\1\n    let assets = statement\n\2?;\n    Ok(assets)\n}",
        "materialize draft assets",
    )


def fix_deletion_queries() -> None:
    path = Path("src-tauri/src/deletion.rs")
    regex_once(
        path,
        r"(let operations = \{\n\s+let mut statement = .*?\.map_err\(CommandError::database\)\?;)\n"
        r"\s+statement\n(\s+\.query_map\(\[now\], operation_from_row\).*?"
        r"\.map_err\(CommandError::database\)\?)\n\s+\};",
        r"\1\n        let rows = statement\n\2;\n        rows\n    };",
        "materialize expired deletion operations",
    )
    regex_once(
        path,
        r"(fn load_active_note_ids_for_deck\(.*?let mut statement = .*?\.map_err\(CommandError::database\)\?;)\n"
        r"\s+statement\n(\s+\.query_map\(\[deck_id\],.*?"
        r"\.map_err\(CommandError::database\))\n\}",
        r"\1\n    let note_ids = statement\n\2?;\n    Ok(note_ids)\n}",
        "materialize active note IDs",
    )


def main() -> None:
    remove_superseded_delete_paths()
    fix_asset_queries()
    fix_deletion_queries()


if __name__ == "__main__":
    main()
