#!/usr/bin/env python3
"""Explicit offline recovery for the exact final pre-public v26 oStudy lineage."""

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
import time
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = ROOT / "scripts" / "history" / "pre-public-v26" / "ledger.json"
CONTRACT = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
FINAL_PRE_PUBLIC_VERSION = int(CONTRACT["schemaVersion"])
ACCEPTED_SQLITE_USER_VERSIONS = tuple(
    int(value) for value in CONTRACT["acceptedSqliteUserVersions"]
)
HISTORICAL_SQLITE_USER_VERSION = 0
EXPECTED_MIGRATIONS = [
    (int(entry["version"]), str(entry["name"]), str(entry["checksum"]))
    for entry in CONTRACT["migrations"]
]
EXPECTED_REPAIRS = [
    (int(entry["version"]), str(entry["name"]), str(entry["checksum"]))
    for entry in CONTRACT["repairs"]
]

PUBLIC_SCHEMA_LINEAGE = "ostudy-public-v1"
PUBLIC_BASELINE_VERSION = 1
PUBLIC_BASELINE_NAME = "public_baseline"
PUBLIC_BASELINE_CHECKSUM = (
    "2d978f8be026ecb820e71ba77a8585e1de51cb08fc2820714526a33284167f85"
)

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

PRESERVED_ID_COLUMNS = {
    "decks": "id",
    "notes": "id",
    "cards": "id",
    "review_events": "id",
    "review_event_attributions": "review_event_id",
    "ai_provider_profiles": "id",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Recover an exact final pre-public-v26 app-data directory through an "
            "explicit staged conversion to ostudy-public-v1. The source is copied "
            "before SQLite opens it; the current active directory is archived rather "
            "than deleted."
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
        raise RuntimeError("ostudy is still running. Close it before recovering data.")


def table_names(connection: sqlite3.Connection) -> set[str]:
    return {
        str(row[0])
        for row in connection.execute(
            "SELECT name FROM sqlite_schema WHERE type = 'table'"
        )
    }


def column_names(connection: sqlite3.Connection, table: str) -> set[str]:
    return {str(row[1]) for row in connection.execute(f'PRAGMA table_info("{table}")')}


def validate_integrity(connection: sqlite3.Connection) -> None:
    integrity = str(connection.execute("PRAGMA integrity_check").fetchone()[0])
    if integrity != "ok":
        raise ValueError(f"SQLite integrity_check failed: {integrity}")
    violation = connection.execute("PRAGMA foreign_key_check").fetchone()
    if violation is not None:
        raise ValueError(f"SQLite foreign_key_check failed: {violation}")


def read_ledger(connection: sqlite3.Connection, table: str) -> list[tuple[int, str, str]]:
    return [
        (int(row[0]), str(row[1]), str(row[2]))
        for row in connection.execute(
            f'SELECT version, name, checksum FROM "{table}" ORDER BY version'
        )
    ]


def validate_final_pre_public_copy(staged_dir: Path) -> Path:
    if not staged_dir.is_dir():
        raise ValueError(f"Staged directory does not exist: {staged_dir}")
    database = staged_dir / "library.sqlite3"
    if not database.is_file():
        raise ValueError(f"Staged copy has no library.sqlite3: {staged_dir}")

    with sqlite3.connect(database) as connection:
        tables = table_names(connection)
        if "schema_metadata" in tables:
            raise ValueError(
                "The supplied archive already belongs to a public schema lineage; "
                "refusing to treat it as pre-public data."
            )
        missing_tables = sorted(REQUIRED_FINAL_TABLES - tables)
        if missing_tables:
            raise ValueError(
                "The archive is not the final pre-public schema; missing tables: "
                + ", ".join(missing_tables)
            )
        if "require_zero_data_retention" in column_names(
            connection, "ai_provider_profiles"
        ):
            raise ValueError(
                "The supplied archive already has the public OpenRouter profile shape."
            )

        sqlite_user_version = int(connection.execute("PRAGMA user_version").fetchone()[0])
        if sqlite_user_version not in ACCEPTED_SQLITE_USER_VERSIONS:
            expected = ", ".join(str(value) for value in ACCEPTED_SQLITE_USER_VERSIONS)
            raise ValueError(
                "Expected the historical pre-public SQLite user_version to be one "
                f"of [{expected}], found {sqlite_user_version}."
            )
        if read_ledger(connection, "schema_migrations") != EXPECTED_MIGRATIONS:
            raise ValueError(
                "The archive does not have the exact final pre-public migration ledger. "
                "It was left unchanged."
            )
        if read_ledger(connection, "data_repairs") != EXPECTED_REPAIRS:
            raise ValueError(
                "The archive does not have the exact final pre-public repair ledger. "
                "It was left unchanged."
            )
        validate_integrity(connection)
    return database


def application_snapshot(connection: sqlite3.Connection) -> dict[str, Any]:
    tables = [
        str(row[0])
        for row in connection.execute(
            """
            SELECT name
            FROM sqlite_schema
            WHERE type = 'table'
              AND name NOT LIKE 'sqlite_%'
              AND name NOT GLOB 'cards_fts_*'
              AND name NOT IN ('schema_metadata', 'schema_migrations', 'data_repairs')
            ORDER BY name
            """
        )
    ]
    counts = {
        table: int(connection.execute(f'SELECT COUNT(*) FROM "{table}"').fetchone()[0])
        for table in tables
    }
    identities: dict[str, list[str]] = {}
    for table, column in PRESERVED_ID_COLUMNS.items():
        if table in tables:
            identities[table] = [
                str(row[0])
                for row in connection.execute(
                    f'SELECT "{column}" FROM "{table}" ORDER BY "{column}"'
                )
            ]
    return {"counts": counts, "identities": identities}


def convert_staged_copy_to_public_v1(staged_dir: Path) -> Path:
    database = validate_final_pre_public_copy(staged_dir)
    with sqlite3.connect(database) as connection:
        before = application_snapshot(connection)
        connection.execute("PRAGMA foreign_keys = OFF")
        try:
            connection.execute("BEGIN IMMEDIATE")
            connection.execute(
                """
                CREATE TABLE ai_provider_profiles_public (
                    id TEXT PRIMARY KEY CHECK(length(id) = 32),
                    display_name TEXT NOT NULL CHECK(length(trim(display_name)) BETWEEN 1 AND 100),
                    normalized_name TEXT NOT NULL CHECK(length(normalized_name) BETWEEN 1 AND 100),
                    provider_kind TEXT NOT NULL CHECK(length(provider_kind) BETWEEN 1 AND 80),
                    base_url TEXT NOT NULL CHECK(length(base_url) BETWEEN 1 AND 2048),
                    model_id TEXT NOT NULL DEFAULT '' CHECK(length(model_id) <= 300),
                    auth_mode TEXT NOT NULL CHECK(auth_mode IN ('none', 'bearer')),
                    enabled INTEGER NOT NULL DEFAULT 1 CHECK(enabled IN (0, 1)),
                    allow_insecure_http INTEGER NOT NULL DEFAULT 0 CHECK(allow_insecure_http IN (0, 1)),
                    require_zero_data_retention INTEGER NOT NULL DEFAULT 0
                        CHECK(require_zero_data_retention IN (0, 1)),
                    request_timeout_ms INTEGER NOT NULL DEFAULT 60000
                        CHECK(request_timeout_ms BETWEEN 1000 AND 300000),
                    credential_storage TEXT NOT NULL DEFAULT 'none'
                        CHECK(credential_storage IN ('none', 'secure', 'session')),
                    credential_epoch INTEGER NOT NULL DEFAULT 0 CHECK(credential_epoch >= 0),
                    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1),
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL,
                    deleted_at INTEGER
                ) STRICT
                """
            )
            connection.execute(
                """
                INSERT INTO ai_provider_profiles_public (
                    id, display_name, normalized_name, provider_kind, base_url, model_id,
                    auth_mode, enabled, allow_insecure_http, require_zero_data_retention,
                    request_timeout_ms, credential_storage, credential_epoch, revision,
                    created_at, updated_at, deleted_at
                )
                SELECT
                    id, display_name, normalized_name, provider_kind, base_url, model_id,
                    auth_mode, enabled, allow_insecure_http, 0,
                    request_timeout_ms, credential_storage, credential_epoch, revision,
                    created_at, updated_at, deleted_at
                FROM ai_provider_profiles
                """
            )
            connection.execute("DROP TABLE ai_provider_profiles")
            connection.execute(
                "ALTER TABLE ai_provider_profiles_public RENAME TO ai_provider_profiles"
            )
            connection.execute(
                """
                CREATE UNIQUE INDEX ai_provider_profiles_active_name_idx
                ON ai_provider_profiles(normalized_name)
                WHERE deleted_at IS NULL
                """
            )
            connection.execute(
                """
                CREATE INDEX ai_provider_profiles_active_updated_idx
                ON ai_provider_profiles(updated_at DESC)
                WHERE deleted_at IS NULL
                """
            )
            connection.execute(
                """
                CREATE TABLE schema_metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                ) STRICT
                """
            )
            connection.execute(
                "INSERT INTO schema_metadata(key, value) VALUES ('schema_lineage', ?)",
                (PUBLIC_SCHEMA_LINEAGE,),
            )
            connection.execute("DELETE FROM schema_migrations")
            connection.execute("DELETE FROM data_repairs")
            connection.execute(
                """
                INSERT INTO schema_migrations(version, name, checksum, applied_at)
                VALUES (?, ?, ?, ?)
                """,
                (
                    PUBLIC_BASELINE_VERSION,
                    PUBLIC_BASELINE_NAME,
                    PUBLIC_BASELINE_CHECKSUM,
                    int(time.time() * 1000),
                ),
            )
            connection.execute(f"PRAGMA user_version = {PUBLIC_BASELINE_VERSION}")

            after = application_snapshot(connection)
            if after != before:
                raise ValueError(
                    "Learner data changed during explicit pre-public recovery; "
                    "the staged conversion was rolled back."
                )
            validate_integrity(connection)
            connection.commit()
        except Exception:
            connection.rollback()
            raise
        finally:
            connection.execute("PRAGMA foreign_keys = ON")

        lineage = connection.execute(
            "SELECT value FROM schema_metadata WHERE key = 'schema_lineage'"
        ).fetchone()
        if lineage != (PUBLIC_SCHEMA_LINEAGE,):
            raise ValueError("Recovered copy did not enter the public schema lineage.")
        if int(connection.execute("PRAGMA user_version").fetchone()[0]) != PUBLIC_BASELINE_VERSION:
            raise ValueError("Recovered copy has the wrong public SQLite user_version.")
        if read_ledger(connection, "schema_migrations") != [
            (PUBLIC_BASELINE_VERSION, PUBLIC_BASELINE_NAME, PUBLIC_BASELINE_CHECKSUM)
        ]:
            raise ValueError("Recovered copy has the wrong public baseline ledger.")
        if read_ledger(connection, "data_repairs"):
            raise ValueError("Recovered copy retained the historical repair ledger.")
        validate_integrity(connection)
    return database


def unique_sibling(parent: Path, name: str) -> Path:
    candidate = parent / name
    suffix = 1
    while candidate.exists():
        candidate = parent / f"{name}-{suffix}"
        suffix += 1
    return candidate


def restore_backup(
    backup_dir: Path, data_dir: Path
) -> tuple[Path | None, Path, Path]:
    if not backup_dir.is_dir():
        raise ValueError(f"Backup directory does not exist: {backup_dir}")

    timestamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    parent = data_dir.parent
    parent.mkdir(parents=True, exist_ok=True)

    source_backup = backup_dir
    if backup_dir.resolve(strict=False) == data_dir.resolve(strict=False):
        source_backup = unique_sibling(
            parent, f"{data_dir.name}.pre-public-v26-source-{timestamp}"
        )
        shutil.copytree(
            data_dir, source_backup, symlinks=True, copy_function=shutil.copy2
        )

    staging = unique_sibling(
        parent, f".{data_dir.name}.pre-public-recovery-{timestamp}-{os.getpid()}"
    )
    previous_active = unique_sibling(
        parent, f"{data_dir.name}.before-pre-public-recovery-{timestamp}"
    )

    shutil.copytree(
        source_backup, staging, symlinks=True, copy_function=shutil.copy2
    )
    restored_previous: Path | None = None
    try:
        convert_staged_copy_to_public_v1(staging)
        if data_dir.exists():
            os.replace(data_dir, previous_active)
            restored_previous = previous_active
        os.replace(staging, data_dir)
    except Exception:
        if staging.exists():
            shutil.rmtree(staging, ignore_errors=True)
        if (
            restored_previous is not None
            and restored_previous.exists()
            and not data_dir.exists()
        ):
            os.replace(restored_previous, data_dir)
        raise

    return restored_previous, data_dir, source_backup


def main() -> int:
    args = parse_args()
    if not args.confirm:
        print("Pass --confirm after reviewing both directories.", file=sys.stderr)
        return 2

    try:
        backup_dir = require_absolute(args.backup_dir, "--backup-dir")
        data_dir = require_absolute(args.data_dir, "--data-dir")
        ensure_ostudy_stopped()
        previous_active, restored, preserved_source = restore_backup(
            backup_dir, data_dir
        )
    except (OSError, RuntimeError, sqlite3.Error, ValueError) as error:
        print(f"Pre-public recovery failed: {error}", file=sys.stderr)
        return 1

    print(f"Verified pre-public source retained at: {preserved_source}")
    print(f"Installed an explicitly converted public-v1 copy at: {restored}")
    if previous_active is not None:
        print(f"Archived the previous active directory at: {previous_active}")
    print(
        "The preserved pre-public source was never opened by SQLite during conversion."
    )
    print(
        "Start current oStudy normally. Its ordinary public migration chain will "
        "advance the recovered public-v1 copy to the current schema."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
