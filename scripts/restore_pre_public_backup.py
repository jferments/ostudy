#!/usr/bin/env python3
"""Restore an archived final pre-public app-data directory for automatic adoption."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys

CONTRACT_PATH = (
    Path(__file__).resolve().parents[1]
    / "src-tauri"
    / "src"
    / "startup"
    / "pre_public_v26_ledger.json"
)
CONTRACT = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
FINAL_PRE_PUBLIC_VERSION = int(CONTRACT["userVersion"])
EXPECTED_MIGRATIONS = [
    (int(entry["version"]), str(entry["name"]), str(entry["checksum"]))
    for entry in CONTRACT["migrations"]
]
EXPECTED_REPAIRS = [
    (int(entry["version"]), str(entry["name"]), str(entry["checksum"]))
    for entry in CONTRACT["repairs"]
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
