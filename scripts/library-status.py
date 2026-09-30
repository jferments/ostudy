#!/usr/bin/env python3
"""Read-only oStudy library lineage/status report for production and development profiles."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sqlite3
import sys
from typing import Any
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[1]
PROFILE_IDS = {
    "development": "io.github.jferments.ostudy.dev",
    "production": "io.github.jferments.ostudy",
}


CONTRACT_PATH = ROOT / "scripts" / "generated" / "database-contract.json"


def database_contract() -> dict[str, Any]:
    value = json.loads(CONTRACT_PATH.read_text(encoding="utf-8"))
    if value.get("schemaVersion") != 1:
        raise RuntimeError("The generated native database contract has an unsupported schema.")
    public = value.get("public")
    release = value.get("release")
    repairs = value.get("repairs")
    if not isinstance(public, dict) or not isinstance(release, dict) or not isinstance(repairs, list):
        raise RuntimeError("The generated native database contract is malformed.")
    return value


def _ledger_rows(rows: Any, label: str) -> list[tuple[int, str, str]]:
    if not isinstance(rows, list) or not rows:
        raise RuntimeError(f"The generated {label} registry is missing or malformed.")
    decoded: list[tuple[int, str, str]] = []
    for row in rows:
        if not isinstance(row, dict):
            raise RuntimeError(f"The generated {label} registry contains a malformed row.")
        version = row.get("version")
        name = row.get("name")
        checksum = row.get("checksum")
        if (
            not isinstance(version, int)
            or version < 1
            or not isinstance(name, str)
            or not name
            or not isinstance(checksum, str)
            or len(checksum) != 64
            or any(character not in "0123456789abcdef" for character in checksum)
        ):
            raise RuntimeError(f"The generated {label} registry contains invalid metadata.")
        decoded.append((version, name, checksum))
    return decoded


def public_registry() -> list[tuple[int, str, str]]:
    contract = database_contract()
    rows = _ledger_rows(contract["public"].get("migrations"), "public migration")
    latest = contract["public"].get("latestSchemaVersion")
    if [row[0] for row in rows] != list(range(1, len(rows) + 1)) or latest != rows[-1][0]:
        raise RuntimeError("The generated public migration registry is not contiguous.")
    return rows


def repair_registry() -> list[tuple[int, str, str]]:
    return _ledger_rows(database_contract().get("repairs"), "public repair")


def release_contract() -> dict[str, Any]:
    release = dict(database_contract()["release"])
    rows = _ledger_rows(release.get("migrations"), "release migration")
    baseline = release.get("baselineVersion")
    version = release.get("version")
    if (
        not isinstance(baseline, int)
        or not isinstance(version, int)
        or [row[0] for row in rows] != list(range(baseline, version + 1))
    ):
        raise RuntimeError("The generated release migration registry is not contiguous.")
    release["migrations"] = rows
    return release


def pre_public_contract() -> dict[str, Any]:
    return json.loads(
        (ROOT / "scripts/history/pre-public-v26/ledger.json").read_text(
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
    release: dict[str, Any],
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

        if lineage == release["lineage"]:
            expected_release_migrations = release["migrations"]
            prefix = expected_release_migrations[: len(migration_rows)]
            release_valid = (
                bool(migration_rows)
                and migration_rows == prefix
                and user_version == migration_rows[-1][0]
                and repair_rows == []
            )
            if release_valid and user_version == release["version"]:
                report["state"] = "release-current"
            elif release_valid and release["baselineVersion"] <= user_version < release["version"]:
                report["state"] = "release-supported-older"
            else:
                report["state"] = "release-unsupported-or-damaged"
        elif lineage == database_contract()["public"]["lineage"]:
            prefix = expected_migrations[: len(migration_rows)]
            migrations_valid = bool(migration_rows) and migration_rows == prefix
            repairs_valid = all(
                row in expected_repairs and row[0] <= user_version
                for row in repair_rows
            )
            if not migrations_valid or user_version != len(migration_rows) or not repairs_valid:
                report["state"] = "public-unsupported-or-damaged"
            elif len(migration_rows) == len(expected_migrations) and repair_rows == expected_repairs:
                report["state"] = "legacy-public-current"
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
            foreign_key = connection.execute("PRAGMA foreign_key_check").fetchone()
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
    release = release_contract()
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
            release=release,
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
    unsafe_states = {
        "unreadable",
        "integrity-failed",
        "release-unsupported-or-damaged",
        "public-unsupported-or-damaged",
        "unsupported-lineage",
        "unknown-pre-public",
    }
    return 1 if any(report["state"] in unsafe_states for report in reports) else 0


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
    return status_exit_code(reports)


if __name__ == "__main__":
    sys.exit(main())
