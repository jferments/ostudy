#!/usr/bin/env python3
"""Restore PR #45 native architecture and apply the U6 migration additively.

This is intentionally narrow: it replaces only the two files that were
incorrectly rewritten during early UI-branch development, then inserts the
version-26 migration and command registration using stable source anchors.
"""

from __future__ import annotations

import hashlib
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
BASE_SHA = "d735eff95d28cb00b3ec181b26bb79ec1b9bb7ca"
MIGRATION_PATH = ROOT / "src-tauri/migrations/0026_deck_folders.sql"
DATABASE_PATH = ROOT / "src-tauri/src/database.rs"
LIB_PATH = ROOT / "src-tauri/src/lib.rs"
STAGE_TEST_PATH = ROOT / "src-tauri/src/stage34_35_regression_tests.rs"
MANIFEST_CHECK_PATH = ROOT / "scripts/verify-project-manifests.mjs"


def git_show(path: str) -> str:
    completed = subprocess.run(
        ["git", "show", f"{BASE_SHA}:{path}"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout


def migration_block(checksum: str) -> str:
    return f'''    Migration {{
        version: 26,
        name: "deck_folders",
        checksum: "{checksum}",
        sql: include_str!("../migrations/0026_deck_folders.sql"),
    }},'''


def insert_migration(source: str, checksum: str, label: str) -> str:
    if "0026_deck_folders.sql" in source:
        source = re.sub(
            r'(version:\s*26,\s*name:\s*"deck_folders",\s*checksum:\s*")[^"]+("\s*,)',
            rf"\g<1>{checksum}\g<2>",
            source,
            count=1,
            flags=re.DOTALL,
        )
    else:
        pattern = re.compile(
            r'(\s*Migration\s*\{\s*version:\s*25,.*?'
            r'include_str!\("\.\./migrations/0025_durable_editor_drafts\.sql"\),'
            r'\s*\},)(\s*\];)',
            re.DOTALL,
        )
        source, count = pattern.subn(
            lambda match: f"{match.group(1)}\n{migration_block(checksum)}{match.group(2)}",
            source,
            count=1,
        )
        if count != 1:
            raise RuntimeError(f"Could not locate migration 25 terminator in {label}")

    source, count = re.subn(
        r'(LATEST_SCHEMA_VERSION:\s*i64\s*=\s*)25(\s*;)',
        r"\g<1>26\g<2>",
        source,
    )
    if count == 0:
        raise RuntimeError(f"Could not update LATEST_SCHEMA_VERSION in {label}")
    return source


def patch_lib(source: str) -> str:
    if "mod deck_folders;" not in source:
        source, count = source.replace(
            "mod deck_cards;\n",
            "mod deck_cards;\nmod deck_folders;\n",
            1,
        ), source.count("mod deck_cards;\n")
        if count == 0:
            raise RuntimeError("Could not register the deck_folders module")

    if "deck_folders::get_deck_library_organization" not in source:
        anchor = "            decks::set_deck_archived,\n"
        commands = (
            "            deck_folders::get_deck_library_organization,\n"
            "            deck_folders::create_deck_folder,\n"
            "            deck_folders::update_deck_folder,\n"
            "            deck_folders::delete_deck_folder,\n"
            "            deck_folders::move_deck_in_library,\n"
            "            deck_folders::move_deck_folder,\n"
        )
        if anchor not in source:
            raise RuntimeError("Could not locate the deck command registration anchor")
        source = source.replace(anchor, anchor + commands, 1)
    return source


def patch_manifest_check(source: str) -> str:
    source = re.sub(
        r'(LATEST_SCHEMA_VERSION\s*=\s*)25\b',
        r"\g<1>26",
        source,
    )
    if "0025_durable_editor_drafts.sql" in source and "0026_deck_folders.sql" not in source:
        source = source.replace(
            '"0025_durable_editor_drafts.sql",',
            '"0025_durable_editor_drafts.sql",\n  "0026_deck_folders.sql",',
            1,
        )
    return source


def main() -> None:
    checksum = hashlib.sha256(MIGRATION_PATH.read_bytes()).hexdigest()

    database = insert_migration(
        git_show("src-tauri/src/database.rs"),
        checksum,
        "src-tauri/src/database.rs",
    )
    DATABASE_PATH.write_text(database, encoding="utf-8")

    lib = patch_lib(git_show("src-tauri/src/lib.rs"))
    LIB_PATH.write_text(lib, encoding="utf-8")

    if STAGE_TEST_PATH.exists():
        stage_tests = insert_migration(
            STAGE_TEST_PATH.read_text(encoding="utf-8"),
            checksum,
            str(STAGE_TEST_PATH.relative_to(ROOT)),
        )
        STAGE_TEST_PATH.write_text(stage_tests, encoding="utf-8")

    if MANIFEST_CHECK_PATH.exists():
        manifest_check = patch_manifest_check(
            MANIFEST_CHECK_PATH.read_text(encoding="utf-8")
        )
        MANIFEST_CHECK_PATH.write_text(manifest_check, encoding="utf-8")

    print(f"Restored PR #45 native architecture and registered migration 26 ({checksum}).")


if __name__ == "__main__":
    main()
