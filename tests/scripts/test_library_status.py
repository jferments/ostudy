import importlib.util
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "library-status.py"
SPEC = importlib.util.spec_from_file_location("ostudy_library_status", SCRIPT)
assert SPEC and SPEC.loader
status = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(status)


def create_ledger_db(path: Path, migrations, repairs, *, lineage=None, user_version=0):
    connection = sqlite3.connect(path)
    connection.executescript(
        """
        CREATE TABLE schema_migrations (
          version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL, applied_at INTEGER NOT NULL
        );
        CREATE TABLE data_repairs (
          version INTEGER PRIMARY KEY, name TEXT NOT NULL, checksum TEXT NOT NULL,
          applied_at INTEGER NOT NULL, changed_rows INTEGER NOT NULL
        );
        CREATE TABLE deck_folders (id TEXT PRIMARY KEY);
        CREATE TABLE decks (id TEXT PRIMARY KEY);
        CREATE TABLE notes (id TEXT PRIMARY KEY);
        CREATE TABLE cards (id TEXT PRIMARY KEY);
        CREATE TABLE review_events (id TEXT PRIMARY KEY);
        CREATE TABLE review_event_attributions (review_event_id TEXT PRIMARY KEY);
        CREATE TABLE ai_provider_profiles (id TEXT PRIMARY KEY);
        CREATE TABLE documents (id TEXT PRIMARY KEY);
        """
    )
    if lineage is not None:
        connection.execute(
            "CREATE TABLE schema_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)"
        )
        connection.execute(
            "INSERT INTO schema_metadata VALUES ('schema_lineage', ?)", (lineage,)
        )
    connection.executemany(
        "INSERT INTO schema_migrations VALUES (?, ?, ?, 0)", migrations
    )
    connection.executemany(
        "INSERT INTO data_repairs VALUES (?, ?, ?, 0, 0)", repairs
    )
    connection.execute(f"PRAGMA user_version = {int(user_version)}")
    connection.commit()
    connection.close()


class LibraryStatusTests(unittest.TestCase):
    def setUp(self):
        self.migrations = status.public_registry()
        self.repairs = status.repair_registry()
        self.release = status.release_contract()
        self.legacy = status.pre_public_contract()

    def classify(self, path: Path, verify=False):
        return status.classify_database(
            path,
            verify=verify,
            expected_migrations=self.migrations,
            expected_repairs=self.repairs,
            release=self.release,
            legacy=self.legacy,
        )

    def test_absent_library_is_not_an_error(self):
        with tempfile.TemporaryDirectory() as root:
            report = self.classify(Path(root) / "missing.sqlite3")
            self.assertEqual(report["state"], "absent")

    def test_current_release_library_is_recognized_and_verified_read_only(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "library.sqlite3"
            create_ledger_db(
                path,
                self.release["migrations"],
                [],
                lineage=self.release["lineage"],
                user_version=self.release["version"],
            )
            before = path.read_bytes()
            report = self.classify(path, verify=True)
            self.assertEqual(report["state"], "release-current")
            self.assertEqual(report["integrityCheck"], "ok")
            self.assertEqual(report["foreignKeyCheck"], "ok")
            self.assertEqual(path.read_bytes(), before)

    def test_supported_older_release_library_is_recognized_as_upgrade_input(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "library.sqlite3"
            baseline = [self.release["migrations"][0]]
            create_ledger_db(
                path,
                baseline,
                [],
                lineage=self.release["lineage"],
                user_version=self.release["baselineVersion"],
            )
            self.assertEqual(self.classify(path)["state"], "release-supported-older")

    def test_terminal_public_v1_library_is_recognized_as_legacy_conversion_input(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "library.sqlite3"
            create_ledger_db(
                path,
                self.migrations,
                self.repairs,
                lineage="ostudy-public-v1",
                user_version=len(self.migrations),
            )
            report = self.classify(path)
            self.assertEqual(report["state"], "legacy-public-current")

    def test_exact_final_pre_public_contract_is_distinguished_from_unknown_data(self):
        with tempfile.TemporaryDirectory() as root:
            exact = Path(root) / "exact.sqlite3"
            legacy_migrations = [
                (row["version"], row["name"], row["checksum"])
                for row in self.legacy["migrations"]
            ]
            legacy_repairs = [
                (row["version"], row["name"], row["checksum"])
                for row in self.legacy["repairs"]
            ]
            create_ledger_db(exact, legacy_migrations, legacy_repairs, user_version=0)
            self.assertEqual(self.classify(exact)["state"], "pre-public-v26-exact")

            partial = Path(root) / "partial.sqlite3"
            create_ledger_db(partial, legacy_migrations, legacy_repairs, user_version=0)
            connection = sqlite3.connect(partial)
            connection.execute("DROP TABLE review_event_attributions")
            connection.commit()
            connection.close()
            self.assertEqual(self.classify(partial)["state"], "unknown-pre-public")

            damaged = Path(root) / "damaged.sqlite3"
            bad = list(legacy_migrations)
            bad[-1] = (bad[-1][0], bad[-1][1], "0" * 64)
            create_ledger_db(damaged, bad, legacy_repairs, user_version=0)
            self.assertEqual(self.classify(damaged)["state"], "unknown-pre-public")

    def test_mismatched_release_ledger_is_not_called_current(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "library.sqlite3"
            bad = list(self.release["migrations"])
            bad[-1] = (bad[-1][0], bad[-1][1], "f" * 64)
            create_ledger_db(
                path,
                bad,
                [],
                lineage=self.release["lineage"],
                user_version=self.release["version"],
            )
            self.assertEqual(
                self.classify(path)["state"], "release-unsupported-or-damaged"
            )

    def test_future_or_mismatched_public_ledger_is_not_called_current(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "library.sqlite3"
            bad = list(self.migrations)
            bad[-1] = (bad[-1][0], bad[-1][1], "f" * 64)
            create_ledger_db(
                path,
                bad,
                self.repairs,
                lineage="ostudy-public-v1",
                user_version=len(bad),
            )
            self.assertEqual(
                self.classify(path)["state"], "public-unsupported-or-damaged"
            )

    def test_command_result_fails_for_unknown_or_damaged_state(self):
        unsafe = [
            {"state": "release-unsupported-or-damaged"},
            {"state": "public-unsupported-or-damaged"},
            {"state": "unsupported-lineage"},
            {"state": "unknown-pre-public"},
            {"state": "integrity-failed"},
        ]
        safe = [
            {"state": "absent"},
            {"state": "release-current"},
            {"state": "release-supported-older"},
            {"state": "legacy-public-current"},
            {"state": "public-supported-older"},
            {"state": "pre-public-v26-exact"},
        ]
        self.assertEqual(status.status_exit_code(unsafe), 1)
        self.assertEqual(status.status_exit_code(safe), 0)


if __name__ == "__main__":
    unittest.main()
