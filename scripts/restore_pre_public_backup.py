#!/usr/bin/env python3
"""Restore an archived final pre-public app-data directory for automatic adoption."""

from __future__ import annotations

import argparse
import datetime as dt
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys

FINAL_PRE_PUBLIC_VERSION = 26
EXPECTED_MIGRATIONS = [
    (1, "create_decks", "771e157d6f7eb5e1c024862966718e46ef744298f49287636c75167065904fd6"),
    (2, "cards_assets_settings_practice", "af64ceea1bfb3f1c04fed33f4558fae051174b50b15873d9ddf5ab4b36e55c8b"),
    (3, "fsrs_reviews", "49dd1e24e463c9a08a59e243d90143e2874459dd2a4a8d5f61613d5df7ebcae3"),
    (4, "card_organization", "0e0e25b3de24f04614aa949fce883fb6cc5bf12d1aab6e08dcaab12c548a768d"),
    (5, "remove_legacy_duplicate_trigger", "e275254085414039a7cf5f3a5e08ae35a61ea8f7d3d1b8e016a8d1f087c03029"),
    (6, "card_types_and_templates", "c07b900b2119f6eae6a4de58aeb3af9ab5882ee21d5c29ac7a5c7fcee6b86be8"),
    (7, "rich_templates_single_cloze", "91d120cb0b72c72c40d499a2c8be9b090be96ca760b2bb5a71072e2b71ed45e1"),
    (8, "scheduling_controls", "862b5277d92154d5586b7924023fb64f7cfee08d566b52efbd625fcbab410b2e"),
    (9, "operation_boundaries", "1482b734fadc2e71720e53c397131916179a3e7966c8461bf9d4fe3d5e51dc15"),
    (10, "transactional_schedule_actions", "bff6ab81045823654725e1f022de7c2c9b1715c63205273a24fd7db03eb0a82a"),
    (11, "managed_asset_lifecycle", "fc9d0b9601cbb45a99bb93b8ed448e150013186b88cb87430c84193176ad7280"),
    (12, "versioned_data_repairs", "6776fd4d70ed8bac79dd01774d27d7dca97ece65742dabd69a55077dc6a721a3"),
    (13, "canonical_data_model", "2e56fa0b68049f8a4d2e937b1dbdb18ec45d9c9041b06f5994b3c847f1d2f049"),
    (14, "review_attribution_and_projection_indexes", "ed5b4f510d014b34f7c6b83c3fcf7914baaa3942e51345a74fb8a7179cc1e11a"),
    (15, "ai_provider_profiles", "241246c924f8b050138011088014f515bdb26145eecd4536320331d905ad783b"),
    (16, "ai_provider_probe_capabilities", "12b4ec2518478a9798a19e87577f440a89943d06abfe7cb725f56727a854b75a"),
    (17, "document_flashcard_generation", "6f503141d0a99e0e603fc465c18ae456f18a9c7c66d16b3289b4d5cd4ed7baaa"),
    (18, "document_semantics_and_outline", "06b0023711038b8f9ea2c90c17c5ffb9dda8b286c01e34c4881a0596e96a1c7e"),
    (19, "resumable_large_document_generation", "80dc940f7018b6a754d440cbf5c3b287150b2dfb4def096539c4ff1f14d5fc66"),
    (20, "document_platform_generalization", "ad25bb267ff0061d7dea31f1bac0d7376be5c5af225854b329c31ec6caf29e32"),
    (21, "document_ocr_fallback", "31466a2bec357f46ee1e05680f3b26d5a297445bf686bfc2afd5aef470206e5d"),
    (22, "document_processing_metrics", "92ed1082377e685570ce3fb79aacf1610c38cbea58e60f289104d7a96ce24c8c"),
    (23, "document_provenance_tag_cleanup", "ec70e81cd4f7c5db36793e1ced79d646ab1981ffd2eaee90244908fbc4b71892"),
    (24, "external_file_transfers", "392cfda8f689f8e6f08be650003cf58d283d113452efa6d6d3f905b959bcb2d8"),
    (25, "durable_editor_drafts", "842ab270202dd3e936216e57878cc2b3f503059d78fc1476b4fdeb9a05fbb54e"),
    (26, "deck_folders", "4361a6ea23c1fe910b078b5edfb98c1a44a5dd6e3c307df1f0436db6076bcc58"),
]
EXPECTED_REPAIRS = [
    (1, "duplicate_fingerprints", "fb2f07be41d479048759ac7d754a59022f2c63c3f60082eaf9a40fd28bab7993"),
    (2, "template_and_generated_card_canonicalization", "ec486bad97c8c5495755bae160bf019d61c084444cc09d88ba8e1f0135b3e663"),
    (3, "review_event_historical_attribution", "97014e9b56c2608f9d83d3ba449d6f4308a1fcb3292cfef5c2640ed2cc5e7700"),
]
REQUIRED_FINAL_TABLES = {
    "ai_provider_profiles",
    "cards",
    "data_repairs",
    "deck_folders",
    "decks",
    "notes",
    "review_event_attributions",
    "review_events",
    "schema_migrations",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Copy a timestamped pre-public app-data backup back into the active "
            "location. The current active directory is archived first; nothing "
            "is deleted. The next ostudy start performs the verified v26-to-public adoption."
        )
    )
    parser.add_argument("--backup-dir", required=True, type=Path)
    parser.add_argument("--data-dir", required=True, type=Path)
    parser.add_argument("--confirm", action="store_true")
    return parser.parse_args()


def require_absolute(path: Path, label: str) -> Path:
    if not path.is_absolute():
        raise ValueError(f"{label} must be an absolute path: {path}")
    return path.resolve(strict=False)


def ensure_ostudy_stopped() -> None:
    try:
        result = subprocess.run(
            ["pgrep", "-x", "ostudy"],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
    except FileNotFoundError:
        return
    if result.returncode == 0:
        raise RuntimeError("ostudy is still running. Close it before restoring data.")


def table_names(connection: sqlite3.Connection) -> set[str]:
    return {
        row[0]
        for row in connection.execute(
            "SELECT name FROM sqlite_schema WHERE type = 'table'"
        )
    }


def column_names(connection: sqlite3.Connection, table: str) -> set[str]:
    return {row[1] for row in connection.execute(f"PRAGMA table_info({table})")}


def validate_final_pre_public_backup(backup_dir: Path) -> Path:
    if not backup_dir.is_dir():
        raise ValueError(f"Backup directory does not exist: {backup_dir}")
    database = backup_dir / "library.sqlite3"
    if not database.is_file():
        raise ValueError(f"Backup has no library.sqlite3: {backup_dir}")

    uri = f"file:{database.as_posix()}?mode=ro"
    with sqlite3.connect(uri, uri=True) as connection:
        tables = table_names(connection)
        if "schema_metadata" in tables:
            raise ValueError(
                "The supplied backup already belongs to a public schema lineage; "
                "refusing to treat it as pre-public data."
            )
        missing_tables = sorted(REQUIRED_FINAL_TABLES - tables)
        if missing_tables:
            raise ValueError(
                "The backup is not the final pre-public schema; missing tables: "
                + ", ".join(missing_tables)
            )
        if "require_zero_data_retention" in column_names(
            connection, "ai_provider_profiles"
        ):
            raise ValueError(
                "The supplied backup already has the public OpenRouter profile shape."
            )

        user_version = connection.execute("PRAGMA user_version").fetchone()[0]
        if user_version != FINAL_PRE_PUBLIC_VERSION:
            raise ValueError(
                f"Expected final pre-public user_version {FINAL_PRE_PUBLIC_VERSION}, "
                f"found {user_version}."
            )
        migrations = connection.execute(
            "SELECT version, name, checksum FROM schema_migrations ORDER BY version"
        ).fetchall()
        if migrations != EXPECTED_MIGRATIONS:
            raise ValueError(
                "The backup does not have the exact final pre-public migration ledger. "
                "It was left unchanged."
            )
        repairs = connection.execute(
            "SELECT version, name, checksum FROM data_repairs ORDER BY version"
        ).fetchall()
        if repairs != EXPECTED_REPAIRS:
            raise ValueError(
                "The backup does not have the exact final pre-public repair ledger. "
                "It was left unchanged."
            )
        integrity = connection.execute("PRAGMA integrity_check").fetchone()[0]
        if integrity != "ok":
            raise ValueError(f"Backup SQLite integrity_check failed: {integrity}")
        foreign_key_violation = connection.execute(
            "PRAGMA foreign_key_check"
        ).fetchone()
        if foreign_key_violation is not None:
            raise ValueError(
                "Backup contains a foreign-key violation and cannot be restored safely: "
                f"{foreign_key_violation}"
            )
    return database


def unique_sibling(parent: Path, name: str) -> Path:
    candidate = parent / name
    suffix = 1
    while candidate.exists():
        candidate = parent / f"{name}-{suffix}"
        suffix += 1
    return candidate


def restore_backup(backup_dir: Path, data_dir: Path) -> tuple[Path | None, Path]:
    timestamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    parent = data_dir.parent
    parent.mkdir(parents=True, exist_ok=True)
    staging = unique_sibling(
        parent, f".{data_dir.name}.pre-public-restore-{timestamp}-{os.getpid()}"
    )
    previous_active = unique_sibling(
        parent, f"{data_dir.name}.before-pre-public-restore-{timestamp}"
    )

    shutil.copytree(backup_dir, staging, symlinks=True, copy_function=shutil.copy2)
    restored_previous: Path | None = None
    try:
        if data_dir.exists():
            os.replace(data_dir, previous_active)
            restored_previous = previous_active
        os.replace(staging, data_dir)
    except Exception:
        if staging.exists():
            shutil.rmtree(staging, ignore_errors=True)
        if restored_previous is not None and restored_previous.exists() and not data_dir.exists():
            os.replace(restored_previous, data_dir)
        raise

    return restored_previous, data_dir


def main() -> int:
    args = parse_args()
    if not args.confirm:
        print("Pass --confirm after reviewing both directories.", file=sys.stderr)
        return 2

    try:
        backup_dir = require_absolute(args.backup_dir, "--backup-dir")
        data_dir = require_absolute(args.data_dir, "--data-dir")
        if backup_dir == data_dir:
            raise ValueError("--backup-dir and --data-dir must be different directories.")
        ensure_ostudy_stopped()
        validate_final_pre_public_backup(backup_dir)
        previous_active, restored = restore_backup(backup_dir, data_dir)
    except (OSError, RuntimeError, sqlite3.Error, ValueError) as error:
        print(f"Pre-public restore failed: {error}", file=sys.stderr)
        return 1

    print(f"Copied verified final pre-public data from: {backup_dir}")
    print(f"Restored active ostudy data directory: {restored}")
    if previous_active is not None:
        print(f"Archived the previous active directory at: {previous_active}")
    print("The original timestamped pre-public backup was retained unchanged.")
    print("Start ostudy normally; it will create a SQLite backup and adopt the v26 data in place.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
