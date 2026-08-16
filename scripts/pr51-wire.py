#!/usr/bin/env python3
"""One-shot exact-anchor wiring for PR #51.

This script exists only because GitHub's contents API replaces entire existing
files. It performs small asserted edits against the checked-out PR branch and
then deletes itself and its temporary workflow.
"""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content, encoding="utf-8")


def insert_after_anchor(content: str, anchor: str, insertion: str, label: str) -> str:
    if insertion.strip() in content:
        return content
    if content.count(anchor) != 1:
        raise RuntimeError(f"Expected one {label} anchor, found {content.count(anchor)}")
    return content.replace(anchor, anchor + insertion, 1)


def replace_once(content: str, old: str, new: str, label: str) -> str:
    if new in content:
        return content
    if content.count(old) != 1:
        raise RuntimeError(f"Expected one {label} target, found {content.count(old)}")
    return content.replace(old, new, 1)


def patch_lib() -> None:
    path = "src-tauri/src/lib.rs"
    content = read(path)
    content = insert_after_anchor(content, "mod decks;\n", "mod device_sync;\n", "device_sync module")

    if "mod document_background;" not in content:
        candidates = ("mod document_backup;\n", "mod document_checkpoints;\n", "mod document_domain;\n")
        for anchor in candidates:
            if anchor in content:
                content = content.replace(anchor, anchor + "mod document_background;\n", 1)
                break
        else:
            raise RuntimeError("Could not locate the document module declaration group")

    commands = (
        "            device_sync::get_device_sync_identity,\n"
        "            device_sync::prepare_device_sync_snapshot,\n"
        "            device_sync::preview_device_sync,\n"
        "            device_sync::commit_device_sync,\n"
    )
    if "device_sync::get_device_sync_identity" not in content:
        anchor = "tauri::generate_handler![\n"
        if content.count(anchor) != 1:
            raise RuntimeError("Could not locate the unique Tauri command registry")
        content = content.replace(anchor, anchor + commands, 1)
    write(path, content)


def patch_document_execution() -> None:
    path = "src-tauri/src/document_operations.rs"
    content = read(path)
    if "document_background::DocumentExecutionLease" not in content:
        anchor = "    database::Database,\n"
        if content.count(anchor) != 1:
            raise RuntimeError("Could not locate document_operations import anchor")
        content = content.replace(
            anchor,
            anchor + "    document_background::DocumentExecutionLease,\n",
            1,
        )

    signature = (
        "pub async fn run_pdf_flashcard_generation(\n"
        "    database: State<'_, Database>,\n"
    )
    updated_signature = (
        "pub async fn run_pdf_flashcard_generation(\n"
        "    app: AppHandle,\n"
        "    database: State<'_, Database>,\n"
    )
    content = replace_once(
        content,
        signature,
        updated_signature,
        "document generation AppHandle",
    )

    old_body = (
        "    let document_id = input.document_id.clone();\n"
        "    let database_after = database.clone();\n"
    )
    new_body = (
        "    let document_id = input.document_id.clone();\n"
        "    let _execution_lease = DocumentExecutionLease::begin(\n"
        "        app,\n"
        "        format!(\"generation-{document_id}\"),\n"
        "        \"Generating study material\",\n"
        "        \"Processing the selected PDF\",\n"
        "    )\n"
        "    .map_err(|detail| {\n"
        "        CommandError::new(\n"
        "            \"document_background_unavailable\",\n"
        "            format!(\"Android could not start foreground document processing: {detail}\"),\n"
        "        )\n"
        "    })?;\n"
        "    let database_after = database.clone();\n"
    )
    content = replace_once(content, old_body, new_body, "document execution lease")
    write(path, content)


def patch_android_capability() -> None:
    path = "src-tauri/src/platform/android.rs"
    content = read(path)
    old = (
        "                background_execution: ServiceCapability::unavailable(\n"
        "                    \"android_background_execution_not_implemented\",\n"
        "                ),\n"
    )
    new = "                background_execution: ServiceCapability::available(),\n"
    if old in content or new in content:
        content = replace_once(content, old, new, "Android background capability")
    else:
        raise RuntimeError("Could not locate Android background execution capability")
    write(path, content)


def patch_package_scripts() -> None:
    path = ROOT / "package.json"
    package = json.loads(path.read_text(encoding="utf-8"))
    scripts = package.setdefault("scripts", {})
    scripts.update(
        {
            "android:build:debug": "node scripts/android-package.mjs build-debug",
            "android:install:debug": "node scripts/android-package.mjs install-debug",
            "android:build:release": "node scripts/android-package.mjs build-release",
            "android:bundle:release": "node scripts/android-package.mjs bundle-release",
            "android:artifacts": "node scripts/android-package.mjs artifacts",
        }
    )
    path.write_text(json.dumps(package, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def remove_one_shot_files() -> None:
    for relative in ("scripts/pr51-wire.py", ".github/workflows/pr51-wire.yml"):
        path = ROOT / relative
        if path.exists():
            path.unlink()


def main() -> None:
    patch_lib()
    patch_document_execution()
    patch_android_capability()
    patch_package_scripts()
    remove_one_shot_files()


if __name__ == "__main__":
    main()
