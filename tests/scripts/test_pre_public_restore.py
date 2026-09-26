from __future__ import annotations

import hashlib
import importlib.util
from pathlib import Path
import sqlite3
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT_PATH = ROOT / "scripts" / "restore_pre_public_backup.py"
SPEC = importlib.util.spec_from_file_location("restore_pre_public_backup", SCRIPT_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("Could not load restore_pre_public_backup.py")
restore = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(restore)


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def create_final_pre_public_backup(
    directory: Path,
    sqlite_user_version: int = restore.HISTORICAL_SQLITE_USER_VERSION,
) -> Path:
    directory.mkdir(parents=True)
    database = directory / "library.sqlite3"
    with sqlite3.connect(database) as connection:
        connection.executescript(
            """
            CREATE TABLE schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                checksum TEXT NOT NULL,
                applied_at INTEGER NOT NULL
            ) STRICT;
            CREATE TABLE data_repairs (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                checksum TEXT NOT NULL,
                applied_at INTEGER NOT NULL,
                changed_rows INTEGER NOT NULL
            ) STRICT;
            CREATE TABLE deck_folders (id TEXT PRIMARY KEY) STRICT;
            CREATE TABLE decks (id TEXT PRIMARY KEY, name TEXT NOT NULL) STRICT;
            CREATE TABLE notes (id TEXT PRIMARY KEY, deck_id TEXT) STRICT;
            CREATE TABLE cards (id TEXT PRIMARY KEY, note_id TEXT, deck_id TEXT) STRICT;
            CREATE TABLE review_events (id TEXT PRIMARY KEY, card_id TEXT) STRICT;
            CREATE TABLE review_event_attributions (
                review_event_id TEXT PRIMARY KEY,
                captured_deck_name TEXT NOT NULL
            ) STRICT;
            CREATE TABLE ai_provider_profiles (
                id TEXT PRIMARY KEY CHECK(length(id) = 32),
                display_name TEXT NOT NULL,
                normalized_name TEXT NOT NULL,
                provider_kind TEXT NOT NULL,
                base_url TEXT NOT NULL,
                model_id TEXT NOT NULL DEFAULT '',
                auth_mode TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                allow_insecure_http INTEGER NOT NULL DEFAULT 0,
                request_timeout_ms INTEGER NOT NULL DEFAULT 60000,
                credential_storage TEXT NOT NULL DEFAULT 'none',
                credential_epoch INTEGER NOT NULL DEFAULT 0,
                revision INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                deleted_at INTEGER
            ) STRICT;
            INSERT INTO ai_provider_profiles VALUES (
                'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
                'Local model', 'local model', 'llama_cpp',
                'http://127.0.0.1:8080', 'local', 'none', 1, 1, 60000,
                'none', 0, 1, 1000, 1000, NULL
            );
            INSERT INTO decks VALUES ('deck-1', 'Preserved deck');
            INSERT INTO notes VALUES ('note-1', 'deck-1');
            INSERT INTO cards VALUES ('card-1', 'note-1', 'deck-1');
            INSERT INTO review_events VALUES ('review-1', 'card-1');
            INSERT INTO review_event_attributions VALUES ('review-1', 'Preserved deck');
            """
        )
        connection.executemany(
            "INSERT INTO schema_migrations(version, name, checksum, applied_at) "
            "VALUES (?, ?, ?, 0)",
            restore.EXPECTED_MIGRATIONS,
        )
        connection.executemany(
            "INSERT INTO data_repairs(version, name, checksum, applied_at, changed_rows) "
            "VALUES (?, ?, ?, 0, 0)",
            restore.EXPECTED_REPAIRS,
        )
        connection.execute(f"PRAGMA user_version = {sqlite_user_version}")

    image = directory / "assets" / "images" / "preserved.png"
    image.parent.mkdir(parents=True)
    image.write_bytes(b"preserved-image")
    return database


class PrePublicRestoreTests(unittest.TestCase):
    def test_historical_zero_marker_is_copied_and_active_directory_is_archived(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-pre-public-restore-") as root_value:
            root = Path(root_value)
            backup = root / "io.github.jferments.ostudy.pre-public-20260817T172808Z"
            database = create_final_pre_public_backup(backup)
            backup_hash = file_sha256(database)

            active = root / "io.github.jferments.ostudy"
            active.mkdir()
            (active / "new-public-attempt.txt").write_text("preserve this too")

            archived_active, restored, preserved_source = restore.restore_backup(backup, active)

            self.assertEqual(restored, active)
            self.assertEqual(preserved_source, backup)
            self.assertIsNotNone(archived_active)
            assert archived_active is not None
            self.assertTrue((archived_active / "new-public-attempt.txt").is_file())
            self.assertEqual(
                (active / "assets" / "images" / "preserved.png").read_bytes(),
                b"preserved-image",
            )
            self.assertNotEqual(file_sha256(active / "library.sqlite3"), backup_hash)
            self.assertEqual(file_sha256(database), backup_hash)
            self.assertTrue(backup.is_dir(), "the original backup must remain intact")
            with sqlite3.connect(active / "library.sqlite3") as connection:
                self.assertEqual(
                    connection.execute("SELECT value FROM schema_metadata WHERE key='schema_lineage'").fetchone()[0],
                    restore.PUBLIC_SCHEMA_LINEAGE,
                )
                self.assertEqual(connection.execute("PRAGMA user_version").fetchone()[0], 1)
                self.assertEqual(
                    connection.execute("SELECT version, name, checksum FROM schema_migrations").fetchall(),
                    [(1, restore.PUBLIC_BASELINE_NAME, restore.PUBLIC_BASELINE_CHECKSUM)],
                )
                self.assertEqual(connection.execute("SELECT COUNT(*) FROM data_repairs").fetchone()[0], 0)
                self.assertIn(
                    "require_zero_data_retention",
                    restore.column_names(connection, "ai_provider_profiles"),
                )

    def test_explicitly_stamped_v26_marker_is_also_accepted(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-pre-public-stamped-") as root_value:
            root = Path(root_value)
            backup = root / "legacy"
            database = create_final_pre_public_backup(
                backup, restore.FINAL_PRE_PUBLIC_VERSION
            )
            backup_hash = file_sha256(database)
            active = root / "active"

            archived_active, restored, preserved_source = restore.restore_backup(backup, active)

            self.assertIsNone(archived_active)
            self.assertEqual(restored, active)
            self.assertEqual(preserved_source, backup)
            self.assertNotEqual(file_sha256(active / "library.sqlite3"), backup_hash)
            self.assertEqual(file_sha256(database), backup_hash)

    def test_in_place_recovery_creates_immutable_source_archive_first(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-pre-public-in-place-") as root_value:
            root = Path(root_value)
            active = root / "io.github.jferments.ostudy"
            database = create_final_pre_public_backup(active)
            source_hash = file_sha256(database)

            archived_active, restored, preserved_source = restore.restore_backup(active, active)

            self.assertEqual(restored, active)
            self.assertIsNotNone(archived_active)
            self.assertNotEqual(preserved_source, active)
            self.assertTrue(preserved_source.is_dir())
            self.assertEqual(file_sha256(preserved_source / "library.sqlite3"), source_hash)
            with sqlite3.connect(active / "library.sqlite3") as connection:
                self.assertEqual(
                    connection.execute("SELECT value FROM schema_metadata WHERE key='schema_lineage'").fetchone()[0],
                    restore.PUBLIC_SCHEMA_LINEAGE,
                )

    def test_unknown_sqlite_marker_is_rejected_without_moving_active_data(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-pre-public-marker-") as root_value:
            root = Path(root_value)
            backup = root / "legacy"
            create_final_pre_public_backup(backup, 7)
            active = root / "active"
            active.mkdir()
            marker = active / "marker.txt"
            marker.write_text("active")

            with self.assertRaisesRegex(ValueError, "user_version"):
                restore.restore_backup(backup, active)
            self.assertEqual(marker.read_text(), "active")

    def test_wrong_migration_ledger_is_rejected_without_moving_active_data(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-pre-public-invalid-") as root_value:
            root = Path(root_value)
            backup = root / "legacy"
            database = create_final_pre_public_backup(backup)
            with sqlite3.connect(database) as connection:
                connection.execute(
                    "UPDATE schema_migrations SET checksum = 'wrong' WHERE version = 26"
                )

            active = root / "active"
            active.mkdir()
            marker = active / "marker.txt"
            marker.write_text("active")

            with self.assertRaisesRegex(ValueError, "migration ledger"):
                restore.restore_backup(backup, active)
            self.assertEqual(marker.read_text(), "active")

    def test_public_database_is_not_misclassified_as_pre_public(self) -> None:
        with tempfile.TemporaryDirectory(prefix="ostudy-public-restore-") as root_value:
            root = Path(root_value)
            backup = root / "public"
            database = create_final_pre_public_backup(backup)
            with sqlite3.connect(database) as connection:
                connection.execute(
                    "CREATE TABLE schema_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL) STRICT"
                )
                connection.execute(
                    "INSERT INTO schema_metadata VALUES ('schema_lineage', 'ostudy-public-v1')"
                )
            active = root / "active"
            active.mkdir()
            marker = active / "marker.txt"
            marker.write_text("active")

            with self.assertRaisesRegex(ValueError, "public schema lineage"):
                restore.restore_backup(backup, active)
            self.assertEqual(marker.read_text(), "active")


if __name__ == "__main__":
    unittest.main()
