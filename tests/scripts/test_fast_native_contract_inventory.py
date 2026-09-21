"""Verify the one-pass fast native inventory without requiring Rust."""
from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "verify-fast-native-contracts.sh"
FILTERS = [
    "database::entry_tests::",
    "current_format_tests::",
    "external_file_",
    "document_",
    "documents::image_sets::",
    "library_backup::image_set_backup_tests::",
    "tutor::",
    "knowledge::",
    "task_activity::",
    "task_cancellation::",
    "device_sync::scope::",
    "device_sync::package::",
    "library_backup::media_backup_tests",
    "library_backup::tutor_backup_tests",
    "cards::content::tests::accepts_versioned_tables_tasks_and_subscripts",
    "documents::lifecycle::tests::startup_interruption_preserves_ready_state_for_a_guide_only_document",
    "cards::",
    "authoring::card_commands::interactive::",
    "database::interactive_migration_tests::",
    "practice::question_attempts::",
    "asset_lifecycle::question_attempt_assets_keep_images_referenced",
    "asset_lifecycle::occlusion::",
    "pdf_",
    "database::tests::latest_schema_is_idempotent_and_public_baseline_fixture_is_immutable",
    "database::tests::anki_apkg_migration_preserves_existing_staging_rows_and_accepts_apkg",
    "database::tests::public_migration_checksum_and_future_version_are_enforced",
]


class FastNativeInventoryTests(unittest.TestCase):
    def invoke(self, missing: str | None = None, exit_code: int = 0):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            calls = root / "calls.jsonl"
            fake = root / "cargo"
            fake.write_text(
                """#!/usr/bin/env python3
import json, os, sys
from pathlib import Path
with Path(os.environ["FAKE_CALLS"]).open("a") as output:
    output.write(json.dumps(sys.argv[1:]) + "\\n")
code = int(os.environ.get("FAKE_EXIT", "0"))
if code:
    raise SystemExit(code)
for value in json.loads(os.environ["FAKE_FILTERS"]):
    if value != os.environ.get("FAKE_MISSING", ""):
        print(f"{value}fixture: test")
"""
            )
            fake.chmod(0o700)
            env = dict(
                os.environ,
                PATH=f"{root}:{os.environ['PATH']}",
                FAKE_CALLS=str(calls),
                FAKE_FILTERS=json.dumps(FILTERS),
                FAKE_MISSING=missing or "",
                FAKE_EXIT=str(exit_code),
            )
            result = subprocess.run(
                ["bash", str(SCRIPT)],
                cwd=ROOT,
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            recorded = [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else []
            return result, recorded

    def test_lists_once_and_accepts_every_required_family(self):
        result, calls = self.invoke()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(calls), 1)
        self.assertIn("--lib", calls[0])
        self.assertIn("--all-features", calls[0])
        self.assertIn("--list", calls[0])
        self.assertIn("PASS Fast native inventory", result.stdout)

    def test_missing_family_fails_closed(self):
        result, calls = self.invoke(missing="knowledge::")
        self.assertEqual(result.returncode, 1)
        self.assertEqual(len(calls), 1)
        self.assertIn("knowledge::", result.stderr)
        self.assertIn("matched no tests", result.stderr)

    def test_cargo_listing_failure_is_preserved(self):
        result, calls = self.invoke(exit_code=7)
        self.assertEqual(result.returncode, 7)
        self.assertEqual(len(calls), 1)


if __name__ == "__main__":
    unittest.main()
