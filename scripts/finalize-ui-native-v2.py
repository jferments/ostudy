#!/usr/bin/env python3
"""Apply the final U6 native integration from the exact PR #45 base.

The script is deliberately one-shot and idempotent. Its workflow removes this
file and every earlier finalizer after committing the generated native patch.
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
DECK_LIBRARY_PATH = ROOT / "src/screens/flashcards/DeckLibraryWorkspace.tsx"


def git_show(path: str) -> str:
    return subprocess.run(
        ["git", "show", f"{BASE_SHA}:{path}"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout


def migration_block(checksum: str) -> str:
    return f'''    Migration {{
        version: 26,
        name: "deck_folders",
        checksum: "{checksum}",
        sql: include_str!("../migrations/0026_deck_folders.sql"),
    }},'''


def insert_migration(source: str, checksum: str, label: str) -> str:
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
    if count != 1:
        raise RuntimeError(f"Could not update LATEST_SCHEMA_VERSION in {label}")
    return source


def patch_lib(source: str) -> str:
    module_anchor = "mod deck_cards;\n"
    if module_anchor not in source:
        raise RuntimeError("Could not locate the deck module registration anchor")
    source = source.replace(module_anchor, module_anchor + "mod deck_folders;\n", 1)

    command_anchor = "            decks::set_deck_archived,\n"
    if command_anchor not in source:
        raise RuntimeError("Could not locate the deck command registration anchor")
    commands = (
        "            deck_folders::get_deck_library_organization,\n"
        "            deck_folders::create_deck_folder,\n"
        "            deck_folders::update_deck_folder,\n"
        "            deck_folders::delete_deck_folder,\n"
        "            deck_folders::move_deck_in_library,\n"
        "            deck_folders::move_deck_folder,\n"
    )
    return source.replace(command_anchor, command_anchor + commands, 1)


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


def patch_deck_library(source: str) -> str:
    detail_before = '''          if (view === "library" && detailOpenedRef.current) {
            detailOpenedRef.current = false;
            setDetailRequest(null);
          }'''
    detail_after = '''          if (view === "library" && detailOpenedRef.current) {
            detailOpenedRef.current = false;
            setDetailRequest(null);
            void reloadOrganization().catch((error: unknown) =>
              showToast("error", getErrorMessage(error)),
            );
          }'''
    if detail_before in source:
        source = source.replace(detail_before, detail_after, 1)
    elif detail_after not in source:
        raise RuntimeError("Could not locate deck-detail return refresh anchor")

    drop_before = '''      onDragOver={(event) => event.preventDefault()}
      onDrop={onDrop}
    >'''
    drop_after = '''      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.stopPropagation();
        onDrop(event);
      }}
    >'''
    if drop_before in source:
        source = source.replace(drop_before, drop_after, 1)
    elif drop_after not in source:
        raise RuntimeError("Could not locate nested deck drop anchor")

    stale_before = '''      setOrganization(previous);
      showToast("error", `${getErrorMessage(error)} The previous order was restored.`);'''
    stale_after = '''      setOrganization(previous);
      void reloadOrganization().catch(() => undefined);
      showToast("error", `${getErrorMessage(error)} The previous order was restored.`);'''
    source = source.replace(stale_before, stale_after)
    return source


def main() -> None:
    checksum = hashlib.sha256(MIGRATION_PATH.read_bytes()).hexdigest()
    DATABASE_PATH.write_text(
        insert_migration(
            git_show("src-tauri/src/database.rs"),
            checksum,
            "src-tauri/src/database.rs",
        ),
        encoding="utf-8",
    )
    LIB_PATH.write_text(patch_lib(git_show("src-tauri/src/lib.rs")), encoding="utf-8")

    if STAGE_TEST_PATH.exists():
        base_stage_tests = git_show("src-tauri/src/stage34_35_regression_tests.rs")
        STAGE_TEST_PATH.write_text(
            insert_migration(
                base_stage_tests,
                checksum,
                "src-tauri/src/stage34_35_regression_tests.rs",
            ),
            encoding="utf-8",
        )

    if MANIFEST_CHECK_PATH.exists():
        MANIFEST_CHECK_PATH.write_text(
            patch_manifest_check(MANIFEST_CHECK_PATH.read_text(encoding="utf-8")),
            encoding="utf-8",
        )

    DECK_LIBRARY_PATH.write_text(
        patch_deck_library(DECK_LIBRARY_PATH.read_text(encoding="utf-8")),
        encoding="utf-8",
    )

    print(f"Registered final migration 26 checksum {checksum} on the PR #45 native base.")


if __name__ == "__main__":
    main()
