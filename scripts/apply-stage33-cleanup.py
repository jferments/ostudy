#!/usr/bin/env python3
"""Apply exact Stage 3.3 compile and stale-code cleanup edits once."""

from __future__ import annotations

import re
from pathlib import Path


def replace_once(path: str, old: str, new: str, label: str) -> None:
    file_path = Path(path)
    text = file_path.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match in {path}, found {count}")
    file_path.write_text(text.replace(old, new, 1), encoding="utf-8")


def regex_once(path: str, pattern: str, replacement: str, label: str) -> None:
    file_path = Path(path)
    text = file_path.read_text(encoding="utf-8")
    updated, count = re.subn(pattern, replacement, text, count=1, flags=re.DOTALL)
    if count != 1:
        raise SystemExit(f"{label}: expected one match in {path}, found {count}")
    file_path.write_text(updated, encoding="utf-8")


# Card tests still use these helpers even though production deletion moved out.
replace_once(
    "src-tauri/src/cards.rs",
    "use rusqlite::{Connection, OptionalExtension};",
    "use rusqlite::{params, Connection, OptionalExtension};",
    "restore rusqlite params for fixtures",
)
replace_once(
    "src-tauri/src/cards.rs",
    "    util::validate_entity_id,",
    "    util::{unix_time_millis, validate_entity_id},",
    "restore fixture clock import",
)
replace_once(
    "src-tauri/src/cards.rs",
    """        database
            .with_connection(|connection| delete_card_in_connection(connection, &note_id))
            .expect(\"card should delete\");""",
    """        let deletion = database
            .with_connection(|connection| {
                crate::deletion::delete_cards(
                    connection,
                    vec![note_id.clone()],
                    \"Flashcard\".to_string(),
                )
            })
            .expect(\"card should delete\");
        assert_eq!(deletion.affected_count, 1);""",
    "use exact deletion domain in card fixture",
)

# Remove staged-import prototypes superseded by the package import journal.
regex_once(
    "src-tauri/src/asset_lifecycle.rs",
    r"\npub\(crate\) fn begin_import_draft\(.*?\n\}\n\npub\(crate\) fn stage_import_asset\(.*?\n\}\n\n(?=pub\(crate\) fn finalize_assets_for_owner)",
    "\n",
    "remove unused import staging prototypes",
)
regex_once(
    "src-tauri/src/asset_lifecycle.rs",
    r"\nfn write_new_file\(.*?\n\}\n\n(?=fn create_parent)",
    "\n",
    "remove unused byte writer",
)
replace_once(
    "src-tauri/src/asset_lifecycle.rs",
    """fn quarantine_unknown_files(
    connection: &Connection,
    paths: &AppPaths,
    now: i64,
) -> Result<(), CommandError> {
    let mut known = HashSet::new();""",
    """fn quarantine_unknown_files(
    connection: &Connection,
    paths: &AppPaths,
    now: i64,
) -> Result<(), CommandError> {
    // Recreate the conservative holding directory if an external cleanup tool
    // removed it between launches.
    fs::create_dir_all(paths.asset_quarantine_dir()).map_err(CommandError::io)?;
    let mut known = HashSet::new();""",
    "use managed quarantine path in production",
)

# The deletion command owns this input now; leave no stale duplicate DTO.
regex_once(
    "src-tauri/src/authoring/types.rs",
    r"\n#\[derive\(Debug, Deserialize\)\]\n#\[serde\(rename_all = \"camelCase\", deny_unknown_fields\)\]\npub struct DeleteCardsInput \{\n    pub note_ids: Vec<String>,\n\}\n",
    "\n",
    "remove stale authoring deletion input",
)
