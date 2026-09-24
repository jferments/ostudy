"""Deferred annotations remain visible without becoming failed core knowledge."""
import copy
import importlib.util
import json
from pathlib import Path
import sqlite3
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "generation_annotation_outcomes", ROOT / "scripts/generation_outcomes.py")
OUTCOMES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(OUTCOMES)
RULE = "knowledge_annotation_selection_absent"


def snapshot(deferred=None, accepted=2):
    result = {"acceptedUnits": accepted, "rejectedProposals": 0,
              "unaccountedSources": 0, "unresolvedRegions": 0,
              "protocolComplete": False, "rejectionRules": {}}
    if deferred is not None:
        result["deferredAnnotations"] = {
            "count": deferred, "rules": {RULE: deferred} if deferred else {}}
    return result


def request(after, before=None, request_id="a", predecessor=None):
    return {"requestId": request_id, "operationId": "operation",
            "requestKey": "window", "stage": "knowledge_extraction",
            "attempt": 1 if predecessor is None else 2,
            "status": "response", "inputTokens": 10, "outputTokens": 2,
            "totalTokens": 12, "cost": None,
            "payloadBytesJson": {"nativeTask": {"schemaVersion": 1, "state": "returned"},
                "nativeCheckpoint": {"schemaVersion": 1, "before": before, "after": after},
                "nativeLineage": {"schemaVersion": 1, "previousRequestId": predecessor,
                    "linkageIncomplete": False,
                    "recovery": None if predecessor is None else {
                        "schemaVersion": 1, "allowed": True, "reason": "contract_repair",
                        "triggerRequestId": predecessor}}}}


class GenerationAnnotationOutcomesTests(unittest.TestCase):
    def test_deferred_counts_do_not_become_core_rejections_or_provider_failures(self):
        report = OUTCOMES.summarize([request(snapshot(1))])
        observed = report["deferredAnnotations"]
        self.assertEqual(observed["observedCheckpointRequests"], 1)
        self.assertEqual(observed["observations"], 1)
        self.assertEqual(observed["ruleObservations"], [
            {"stage": "knowledge_extraction", "rule": RULE, "observations": 1}])
        self.assertEqual(report["checkpointRejectionObservations"], [])
        self.assertEqual(report["nativeFailures"], [])
        self.assertEqual(report["recoveryRequests"], [])
        self.assertEqual(report["requestStates"], {"returned": 1})
        text = "\n".join(OUTCOMES.render(report))
        self.assertIn("Deferred annotations: 1 observations", text)
        self.assertIn(RULE, text)

    def test_absent_null_and_measured_zero_are_distinct(self):
        old = snapshot()
        null = snapshot()
        null["deferredAnnotations"] = None
        for value in (old, null):
            report = OUTCOMES.summarize([request(value)])["deferredAnnotations"]
            self.assertEqual(report["observedCheckpointRequests"], 0)
            self.assertEqual(report["unobservedCheckpointRequests"], 1)
            self.assertIsNone(report["observations"])
        measured = OUTCOMES.summarize([request(snapshot(0))])["deferredAnnotations"]
        self.assertEqual(measured["observedCheckpointRequests"], 1)
        self.assertEqual(measured["observations"], 0)
        self.assertEqual(measured["ruleObservations"], [])

    def test_annotation_resolution_and_regression_are_measured_independently(self):
        for before, after, expected in [(1, 0, "committed_progress"),
                                        (0, 1, "native_regression"),
                                        (1, 1, "no_measured_progress")]:
            with self.subTest(before=before, after=after):
                self.assertEqual(OUTCOMES.checkpoint_progress(
                    {"before": snapshot(before), "after": snapshot(after)}), expected)
        self.assertEqual(OUTCOMES.checkpoint_progress(
            {"before": snapshot(0, 1), "after": snapshot(1, 2)}), "mixed_progress")

    def test_unknown_prior_measurements_do_not_invent_annotation_gains(self):
        for before, after in [(snapshot(), snapshot(1)), (snapshot(1), snapshot())]:
            self.assertEqual(OUTCOMES.checkpoint_progress(
                {"before": before, "after": after}), "no_measured_progress")
        self.assertEqual(OUTCOMES.checkpoint_progress(
            {"before": None, "after": snapshot(1)}), "unobserved")

    def test_impossible_or_malformed_annotation_fields_do_not_replace_core_counts(self):
        damaged = [True, [], {}, {"count": True, "rules": {}},
                   {"count": -1, "rules": {}}, {"count": 1, "rules": {RULE: 0}},
                   {"count": 3, "rules": {RULE: 3}}, {"count": 1, "rules": {}},
                   {"count": 1, "rules": {RULE: "1"}},
                   {"count": 1, "rules": {"PRIVATE DRAFT": 1}}]
        for value in damaged:
            with self.subTest(value=value):
                after = snapshot()
                after["deferredAnnotations"] = value
                report = OUTCOMES.summarize([request(after)])
                self.assertEqual(report["checkpointRequests"], 1)
                self.assertEqual(report["requestStates"], {"returned": 1})
                self.assertIsNone(report["deferredAnnotations"]["observations"])
                self.assertEqual(report["linkageOrRecordIssues"], {
                    "invalid_deferred_annotation_observation": 1})
                self.assertNotIn("PRIVATE DRAFT", json.dumps(report))
        complete = snapshot(1)
        complete["protocolComplete"] = True
        self.assertIsNone(OUTCOMES.deferred_annotations(complete))

    def test_recovery_usage_and_repeated_snapshots_are_not_double_counted_as_unique_debt(self):
        rows = [request(snapshot(1)), request(snapshot(0), snapshot(1), "b", "a")]
        report = OUTCOMES.summarize(rows)
        self.assertEqual(report["deferredAnnotations"]["observations"], 1)
        recovery = report["recoveryRequests"][0]
        self.assertEqual(recovery["checkpointOutcome"], "committed_progress")
        self.assertEqual(recovery["inputTokens"], 10)
        self.assertIsNone(recovery["cost"])
        self.assertEqual(recovery["costMissingRequests"], 1)
        report = OUTCOMES.summarize([request(snapshot(1)), request(snapshot(1), request_id="b")])
        self.assertEqual(report["deferredAnnotations"]["observations"], 2)
        self.assertIn("not unique outstanding tasks", " ".join(report["notes"]))

    def test_unknown_checkpoint_version_and_missing_checkpoint_stay_unobserved(self):
        row = request(snapshot(1))
        row["payloadBytesJson"]["nativeCheckpoint"]["schemaVersion"] = 2
        report = OUTCOMES.summarize([row])
        self.assertEqual(report["checkpointRequests"], 0)
        self.assertIsNone(report["deferredAnnotations"]["observations"])
        row["payloadBytesJson"].pop("nativeCheckpoint")
        row["payloadBytesJson"]["nativeTask"]["lastCheckpoint"] = snapshot(1)
        report = OUTCOMES.summarize([row])
        self.assertEqual(report["checkpointRequests"], 0)
        self.assertIsNone(report["deferredAnnotations"]["observations"])

    def test_json_sqlite_read_only_reopen_and_input_preservation(self):
        row = request(snapshot(1))
        original = copy.deepcopy(row)
        expected = OUTCOMES.summarize([row])
        self.assertEqual(row, original)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "diagnostics.sqlite3"
            with sqlite3.connect(path) as connection:
                connection.execute("CREATE TABLE diagnostic_request (row_json TEXT NOT NULL)")
                connection.execute("INSERT INTO diagnostic_request VALUES (?)", (json.dumps(row),))
            original_bytes = path.read_bytes()
            with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True) as connection:
                connection.execute("PRAGMA query_only=ON")
                restored = json.loads(connection.execute(
                    "SELECT row_json FROM diagnostic_request").fetchone()[0])
                restored["payloadBytesJson"] = json.dumps(restored["payloadBytesJson"])
                self.assertEqual(OUTCOMES.summarize([restored]), expected)
            self.assertEqual(path.read_bytes(), original_bytes)
        self.assertEqual(OUTCOMES.summarize([json.loads(json.dumps(row))]), expected)


if __name__ == "__main__":
    unittest.main()
