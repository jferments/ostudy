#!/usr/bin/env python3
"""Read-only oStudy library lineage/status report for production and development profiles."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import sqlite3
import sys
from typing import Any
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[1]
PROFILE_IDS = {
    "development": "io.github.jferments.ostudy.dev",
    "production": "io.github.jferments.ostudy",
}


def public_registry() -> list[tuple[int, str, str]]:
    source = (ROOT / "src-tauri/src/database.rs").read_text(encoding="utf-8")
    registry = source.split("const MIGRATIONS:", 1)[1].split(
        "pub(crate) const LATEST_SCHEMA_VERSION", 1
    )[0]
    rows: list[tuple[int, str, str]] = []
    for body in re.findall(r"Migration\s*\{([\s\S]*?)\}", registry):
        version = re.search(r"version:\s*(\d+)", body)
        name = re.search(r'name:\s*"([^"]+)"', body)
        checksum = re.search(r'checksum:\s*"([0-9a-f]{64})"', body)
        if not (version and name and checksum):
            raise RuntimeError("The public migration registry is malformed.")
        rows.append((int(version.group(1)), name.group(1), checksum.group(1)))
    return rows


def repair_registry() -> list[tuple[int, str, str]]:
    source = (ROOT / "src-tauri/src/data_repairs.rs").read_text(encoding="utf-8")
    registry = source.split("const REPAIRS:", 1)[1].split(
        "pub(crate) fn apply_required_repairs", 1
    )[0]
    rows: list[tuple[int, str, str]] = []
    for body in re.findall(r"DataRepair\s*\{([\s\S]*?)\}", registry):
        version = re.search(r"version:\s*(\d+)", body)
        name = re.search(r'name:\s*"([^"]+)"', body)
        checksum = re.search(r'checksum:\s*"([0-9a-f]{64})"', body)
        if not (version and name and checksum):
            raise RuntimeError("The public repair registry is malformed.")
        rows.append((int(version.group(1)), name.group(1), checksum.group(1)))
    return rows


def pre_public_contract() -> dict[str, Any]:
    return json.loads(
        (ROOT / "src-tauri/src/startup/pre_public_v26_ledger.json").read_text(
            encoding="utf-8"
        )
    )


def table_exists(connection: sqlite3.Connection, name: str) -> bool:
    return bool(
        connection.execute(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name=?)",
            (name,),
        ).fetchone()[0]
    )


def column_exists(connection: sqlite3.Connection, table: str, column: str) -> bool:
    if not table_exists(connection, table):
        return False
    return any(str(row[1]) == column for row in connection.execute(f'PRAGMA table_info("{table}")'))


def pre_public_shape_matches(connection: sqlite3.Connection) -> bool:
    required = (
        "schema_migrations",
        "data_repairs",
        "deck_folders",
        "decks",
        "notes",
        "cards",
        "review_events",
        "review_event_attributions",
        "ai_provider_profiles",
    )
    return all(table_exists(connection, table) for table in required) and not column_exists(
        connection, "ai_provider_profiles", "require_zero_data_retention"
    )


def ledger(connection: sqlite3.Connection, table: str) -> list[tuple[int, str, str]]:
    if not table_exists(connection, table):
        return []
    return [
        (int(row[0]), str(row[1]), str(row[2]))
        for row in connection.execute(
            f'SELECT version, name, checksum FROM "{table}" ORDER BY version'
        )
    ]


def metadata(connection: sqlite3.Connection, key: str) -> str | None:
    if not table_exists(connection, "schema_metadata"):
        return None
    row = connection.execute(
        "SELECT value FROM schema_metadata WHERE key = ?", (key,)
    ).fetchone()
    return None if row is None else str(row[0])


def safe_count(connection: sqlite3.Connection, table: str) -> int | None:
    if not table_exists(connection, table):
        return None
    return int(connection.execute(f'SELECT COUNT(*) FROM "{table}"').fetchone()[0])


def open_read_only(path: Path) -> sqlite3.Connection:
    # mode=ro prevents this diagnostic from creating/replacing the database.
    uri = f"file:{quote(str(path), safe='/')}?mode=ro"
    connection = sqlite3.connect(uri, uri=True, timeout=5)
    connection.execute("PRAGMA query_only = ON")
    return connection


def classify_database(
    path: Path,
    *,
    verify: bool,
    expected_migrations: list[tuple[int, str, str]],
    expected_repairs: list[tuple[int, str, str]],
    legacy: dict[str, Any],
) -> dict[str, Any]:
    report: dict[str, Any] = {
        "databasePath": str(path),
        "exists": path.is_file(),
        "state": "absent",
    }
    if not path.is_file():
        return report

    try:
        connection = open_read_only(path)
    except sqlite3.Error as error:
        report.update(state="unreadable", error=str(error))
        return report

    try:
        user_version = int(connection.execute("PRAGMA user_version").fetchone()[0])
        migration_rows = ledger(connection, "schema_migrations")
        repair_rows = ledger(connection, "data_repairs")
        lineage = metadata(connection, "schema_lineage")
        report.update(
            schemaLineage=lineage,
            sqliteUserVersion=user_version,
            migrationCount=len(migration_rows),
            latestMigration=migration_rows[-1][0] if migration_rows else None,
            repairCount=len(repair_rows),
            repairs=[name for _, name, _ in repair_rows],
        )

        if lineage == "ostudy-public-v1":
            prefix = expected_migrations[: len(migration_rows)]
            migrations_valid = bool(migration_rows) and migration_rows == prefix
            repairs_valid = all(
                row in expected_repairs and row[0] <= user_version
                for row in repair_rows
            )
            if not migrations_valid or user_version != len(migration_rows) or not repairs_valid:
                report["state"] = "public-unsupported-or-damaged"
            elif len(migration_rows) == len(expected_migrations) and repair_rows == expected_repairs:
                report["state"] = "public-current"
            else:
                report["state"] = "public-supported-older"
        elif lineage is not None:
            report["state"] = "unsupported-lineage"
        else:
            legacy_migrations = [
                (int(row["version"]), str(row["name"]), str(row["checksum"]))
                for row in legacy["migrations"]
            ]
            legacy_repairs = [
                (int(row["version"]), str(row["name"]), str(row["checksum"]))
                for row in legacy["repairs"]
            ]
            if (
                pre_public_shape_matches(connection)
                and migration_rows == legacy_migrations
                and repair_rows == legacy_repairs
                and user_version in legacy["acceptedSqliteUserVersions"]
            ):
                report["state"] = "pre-public-v26-exact"
            else:
                report["state"] = "unknown-pre-public"

        report["counts"] = {
            key: value
            for key, value in (
                ("decks", safe_count(connection, "decks")),
                ("cards", safe_count(connection, "cards")),
                ("reviews", safe_count(connection, "review_events")),
                ("documents", safe_count(connection, "documents")),
            )
            if value is not None
        }

        if verify:
            integrity = str(connection.execute("PRAGMA integrity_check").fetchone()[0])
            foreign_key = connection.execute("PRAGMA foreign_key_check LIMIT 1").fetchone()
            report["integrityCheck"] = integrity
            report["foreignKeyCheck"] = "ok" if foreign_key is None else list(foreign_key)
            if integrity != "ok" or foreign_key is not None:
                report["state"] = "integrity-failed"
    except sqlite3.Error as error:
        report.update(state="unreadable", error=str(error))
    finally:
        connection.close()
    return report


def app_data_root() -> Path:
    configured = os.environ.get("XDG_DATA_HOME")
    return Path(configured).expanduser() if configured else Path.home() / ".local/share"


def collect(profile: str, verify: bool) -> list[dict[str, Any]]:
    expected_migrations = public_registry()
    expected_repairs = repair_registry()
    legacy = pre_public_contract()
    names = PROFILE_IDS.keys() if profile == "both" else [profile]
    reports = []
    for name in names:
        app_dir = app_data_root() / PROFILE_IDS[name]
        report = classify_database(
            app_dir / "library.sqlite3",
            verify=verify,
            expected_migrations=expected_migrations,
            expected_repairs=expected_repairs,
            legacy=legacy,
        )
        report.update(profile=name, applicationId=PROFILE_IDS[name], appDataDir=str(app_dir))
        reports.append(report)
    return reports


def print_text(reports: list[dict[str, Any]], verify: bool) -> None:
    print("oStudy read-only library status")
    print("No database writes are performed. Close running oStudy processes for adoption evidence.")
    for report in reports:
        print()
        print(f"{report['profile']}: {report['state']}")
        print(f"  application id: {report['applicationId']}")
        print(f"  app data:       {report['appDataDir']}")
        if not report["exists"]:
            print("  library:        not present")
            continue
        print(f"  database:       {report['databasePath']}")
        print(f"  lineage:        {report.get('schemaLineage') or '<pre-public/none>'}")
        print(f"  user version:   {report.get('sqliteUserVersion')}")
        print(
            f"  migrations:     {report.get('migrationCount')} "
            f"(latest {report.get('latestMigration')})"
        )
        print(f"  repairs:        {report.get('repairCount')} {report.get('repairs', [])}")
        counts = report.get("counts", {})
        if counts:
            print(
                "  counts:         "
                + ", ".join(f"{key}={value}" for key, value in counts.items())
            )
        if verify:
            print(f"  integrity:      {report.get('integrityCheck', '<unavailable>')}")
            print(f"  foreign keys:   {report.get('foreignKeyCheck', '<unavailable>')}")
        if report.get("error"):
            print(f"  error:          {report['error']}")


def status_exit_code(reports: list[dict[str, Any]]) -> int:
    return status_exit_code(reports)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Read-only report for automatically resolved oStudy dev/production libraries."
    )
    parser.add_argument(
        "--profile",
        choices=["development", "production", "both"],
        default="both",
    )
    parser.add_argument("--verify", action="store_true", help="Run SQLite integrity/foreign-key checks.")
    parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON.")
    args = parser.parse_args()

    reports = collect(args.profile, args.verify)
    if args.json:
        print(json.dumps({"schemaVersion": 1, "libraries": reports}, indent=2, sort_keys=True))
    else:
        print_text(reports, args.verify)
    unsafe_states = {
        "unreadable",
        "integrity-failed",
        "public-unsupported-or-damaged",
        "unsupported-lineage",
        "unknown-pre-public",
    }
    return 1 if any(report["state"] in unsafe_states for report in reports) else 0


if __name__ == "__main__":
    sys.exit(main())
