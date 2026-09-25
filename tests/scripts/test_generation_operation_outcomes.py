"""Operation failures cannot disappear merely because no request row exists."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("generation_operations", ROOT / "scripts/generation_operations.py")
OPERATIONS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(OPERATIONS)


def outcome(state="failed", error="document_source_missing"):
    return {"schemaVersion": 1, "state": state, "errorCode": error,
            "resultObserved": state != "interrupted", "artifactPublication": "not_observed"}


def record(identity="a" * 32, value=None, state="failed"):
    return {"id": identity, "purpose": "document_generation", "state": state,
            "startedAt": 1, "finishedAt": 2, "outcomeJson": value}


class OperationOutcomeTests(unittest.TestCase):
    def test_zero_request_failure_retains_cause_without_fabricating_usage(self):
        records = [record(value=json.dumps(outcome()))]
        before = copy.deepcopy(records)
        report = OPERATIONS.summarize(records, [])
        row = report["operations"][0]
        self.assertTrue(row["outcomeObserved"])
        self.assertEqual(row["recordedRequests"], 0)
        self.assertEqual(row["errorCode"], "document_source_missing")
        self.assertEqual(row["artifactPublication"], "not_observed")
        self.assertIn("document_source_missing", "\n".join(OPERATIONS.render(report)))
        self.assertEqual(records, before)

    def test_legacy_absent_and_unknown_version_records_are_not_guessed(self):
        self.assertFalse(OPERATIONS.summarize(None, [])["recordsAvailable"])
        unknown = outcome()
        unknown["schemaVersion"] = 2
        report = OPERATIONS.summarize([record(), record("b" * 32, unknown)], [])
        self.assertEqual(report["observedOperations"], 0)
        self.assertEqual(report["unobservedOperations"], 2)
        self.assertEqual(report["recordIssues"], 1)
        self.assertTrue(all(row["errorCode"] is None for row in report["operations"]))

    def test_provider_success_does_not_erase_native_failure_or_include_other_operation_requests(self):
        requests = [{"operationId": "a" * 32, "status": "response"},
                    {"operationId": "b" * 32, "status": "failed"}]
        report = OPERATIONS.summarize([record(value=outcome())], requests)
        self.assertEqual(report["operations"][0]["state"], "failed")
        self.assertEqual(report["operations"][0]["recordedRequests"], 1)

    def test_interruption_is_not_an_observed_return_and_completion_is_not_publication(self):
        records = [record(value=outcome("complete", None), state="complete"),
                   record("b" * 32, outcome("interrupted", "native_process_interrupted"), "interrupted")]
        report = OPERATIONS.summarize(records, [])
        self.assertTrue(report["operations"][0]["resultObserved"])
        self.assertFalse(report["operations"][1]["resultObserved"])
        self.assertTrue(all(row["artifactPublication"] == "not_observed" for row in report["operations"]))

    def test_conflicts_malformed_private_fields_and_duplicate_identity_stay_unobserved(self):
        cases = ["not json", "[]", {**outcome(), "schemaVersion": True},
                 {**outcome(), "state": "complete"}, {**outcome(), "resultObserved": 1},
                 {**outcome(), "errorCode": "PRIVATE SOURCE PATH"}]
        for value in cases:
            with self.subTest(value=value):
                report = OPERATIONS.summarize([record(value=value)], [])
                self.assertEqual(report["observedOperations"], 0)
                self.assertNotIn("PRIVATE", json.dumps(report))
        report = OPERATIONS.summarize([record(value=outcome()), record(value=outcome())], [])
        self.assertEqual(report["observedOperations"], 0)
        self.assertEqual(report["recordIssues"], 2)
        missing = outcome("complete", None)
        del missing["errorCode"]
        report = OPERATIONS.summarize([record(value=missing, state="complete")], [])
        self.assertEqual(report["observedOperations"], 0)
        self.assertEqual(report["recordIssues"], 1)

    def test_read_records_supports_old_and_new_sqlite_without_mutating_rows(self):
        with sqlite3.connect(":memory:") as connection:
            connection.execute("CREATE TABLE generation_operations(id TEXT,purpose TEXT,state TEXT,started_at INTEGER,finished_at INTEGER)")
            connection.execute("INSERT INTO generation_operations VALUES (?,?,?,?,?)", ("a" * 32, "fixture", "failed", 1, 2))
            original = OPERATIONS.read_records(connection, None, True)
            self.assertIsNone(original[0]["outcomeJson"])
            connection.executescript((ROOT / "src-tauri/migrations/0030_generation_operation_outcomes.sql").read_text())
            self.assertEqual(OPERATIONS.read_records(connection, None, True), original)
            connection.execute("UPDATE generation_operations SET outcome_json=?", (json.dumps(outcome()),))
            before = connection.total_changes
            records = OPERATIONS.read_records(connection, "a" * 32, False)
            self.assertEqual(connection.total_changes, before)
            self.assertEqual(len(records), 1)
            self.assertTrue(OPERATIONS.summarize(records, [])["operations"][0]["outcomeObserved"])
            self.assertEqual(OPERATIONS.read_records(connection, "missing", False), [])
            self.assertEqual(OPERATIONS.read_records(connection, None, False), [])

    def test_cli_exports_zero_request_operations_and_preserves_json_reimport(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "ledger.sqlite3"
            with sqlite3.connect(path) as connection:
                connection.executescript("""
                    CREATE TABLE generation_operations(id TEXT,purpose TEXT,state TEXT,started_at INTEGER,finished_at INTEGER);
                    CREATE TABLE generation_requests(id TEXT,operation_id TEXT,stage TEXT,attempt INTEGER,provider TEXT,model TEXT,status TEXT,
                        input_tokens INTEGER,output_tokens INTEGER,reasoning_tokens INTEGER,cached_input_tokens INTEGER,
                        cache_write_tokens INTEGER,total_tokens INTEGER,cost REAL,response_id TEXT,finish_reason TEXT,error_code TEXT,
                        request_key TEXT,payload_bytes_json TEXT,max_output_tokens INTEGER,started_at INTEGER,finished_at INTEGER);
                    CREATE TABLE generation_reuse_events(operation_id TEXT,stage TEXT);
                """)
                connection.executescript((ROOT / "src-tauri/migrations/0030_generation_operation_outcomes.sql").read_text())
                connection.execute("INSERT INTO generation_operations VALUES (?,?,?,?,?,?)",
                                   ("a" * 32, "document_generation", "failed", 1, 2, json.dumps(outcome())))
                connection.execute("INSERT INTO generation_operations VALUES (?,?,?,?,?,?)",
                                   ("b" * 32, "guide_generation", "failed", 3, 4, None))
            before = hashlib.sha256(path.read_bytes()).hexdigest()
            command = [sys.executable, str(ROOT / "scripts/generation-usage.py"), "--database", str(path)]
            report = json.loads(subprocess.check_output(command + ["--all", "--json"], cwd=root, text=True))
            self.assertEqual(len(report["operationRecords"]), 2)
            self.assertEqual(report["operationOutcomes"]["observedOperations"], 1)
            self.assertEqual(report["totals"]["requests"], 0)
            exported = root / "report.json"
            exported.write_text(json.dumps(report))
            imported = json.loads(subprocess.check_output(
                [sys.executable, str(ROOT / "scripts/generation-usage.py"), "--input-json", str(exported), "--json"],
                cwd=root, text=True))
            self.assertEqual(imported["operationOutcomes"], report["operationOutcomes"])
            rendered = subprocess.check_output(command + ["--operation", "a" * 32], cwd=root, text=True)
            self.assertIn("document_source_missing", rendered)
            latest = json.loads(subprocess.check_output(command + ["--json"], cwd=root, text=True))
            self.assertEqual(latest["operationId"], "b" * 32)
            self.assertEqual(len(latest["operationRecords"]), 1)
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), before)


if __name__ == "__main__":
    unittest.main()
