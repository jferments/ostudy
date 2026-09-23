"""Behavioral coverage for recovery usefulness without fabricated completion."""
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
SPEC = importlib.util.spec_from_file_location("generation_usage_outcomes", ROOT / "scripts/generation-usage.py")
USAGE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(USAGE)


def snap(accepted=1, rejected=0, sources=0, regions=0):
    return {"acceptedUnits": accepted, "rejectedProposals": rejected,
            "unaccountedSources": sources, "unresolvedRegions": regions,
            "protocolComplete": rejected == sources == regions == 0,
            "rejectionRules": {"knowledge_annotation_selection_absent": rejected}}


def row(identity, attempt=1, state="returned", **native):
    return {"requestId": identity, "operationId": "a" * 32,
            "requestKey": "c" * 64, "stage": "knowledge_extraction",
            "attempt": attempt, "status": "response", "inputTokens": 10,
            "outputTokens": 2, "totalTokens": 12,
            "payloadBytesJson": {"serializedRequestBytes": 200,
                                 "nativeTask": {"schemaVersion": 1, "state": state}, **native}}


def recovery(previous, before=None, after=None, **extra):
    result = {"nativeLineage": {
        "schemaVersion": 1, "previousRequestId": previous,
        "linkageIncomplete": False, "recovery": {
            "schemaVersion": 1, "reason": "contract_repair", "allowed": True,
            "triggerRequestId": previous, "triggerCode": "partial_coverage"}}}
    if after is not None:
        result["nativeCheckpoint"] = {"schemaVersion": 1, "before": before, "after": after}
    result.update(extra)
    return result


def summary(rows):
    return USAGE.summarize({"requests": rows})["nativeOutcomes"]


class GenerationOutcomeTests(unittest.TestCase):
    def test_legacy_responses_are_unobserved_not_successful_repairs(self):
        rows = [row("a"), row("b", 2)]
        for value in rows:
            value["payloadBytesJson"] = "{}"
        result = summary(rows)
        self.assertEqual(result["observedRequests"], 0)
        self.assertEqual(result["unobservedRequests"], 2)
        self.assertEqual(result["unclassifiedRepeatedRequests"], 1)
        self.assertEqual(result["recoveryRequests"], [])
        self.assertEqual(result["nativeFailures"], [])

    def test_committed_progress_links_once_and_preserves_missing_usage(self):
        first = row("a", state="continued")
        second = row("b", 2, **recovery("a", snap(rejected=1), snap(accepted=2)))
        second["inputTokens"] = None
        second["totalTokens"] = None
        result = summary([second, first])
        group, = result["recoveryRequests"]
        self.assertEqual(group["checkpointOutcome"], "committed_progress")
        self.assertEqual(group["requests"], 1)
        self.assertIsNone(group["inputTokens"])
        self.assertEqual(group["inputTokensMissingRequests"], 1)
        self.assertEqual(group["outputTokens"], 2)
        self.assertIsNone(group["totalTokens"])

    def test_no_progress_regression_and_mixed_progress_are_separate(self):
        for before, after, expected in [
            (snap(rejected=1), snap(rejected=1), "no_measured_progress"),
            (snap(accepted=2), snap(accepted=1, rejected=1), "native_regression"),
            (snap(), snap(accepted=2, rejected=1), "mixed_progress"),
        ]:
            with self.subTest(expected=expected):
                result = summary([row("a"), row("b", 2, **recovery("a", before, after))])
                self.assertEqual(result["recoveryRequests"][0]["checkpointOutcome"], expected)

    def test_no_baseline_or_invalid_counts_do_not_manufacture_progress(self):
        bad = snap()
        bad["acceptedUnits"] = True
        for before in (None, bad, {**snap(), "unresolvedRegions": -1}):
            result = summary([row("b", **recovery(None, before, snap(accepted=20)))])
            self.assertEqual(result["recoveryRequests"][0]["checkpointOutcome"], "unobserved")
        result = summary([row("b", **recovery(None, snap(regions=None), snap(regions=None)))])
        self.assertEqual(result["recoveryRequests"][0]["checkpointOutcome"], "no_measured_progress")

    def test_resumed_recovery_on_attempt_one_is_included(self):
        result = summary([row("b", **recovery(None, snap(rejected=1), snap(accepted=2)))])
        self.assertEqual(result["recoveryRequests"][0]["requests"], 1)
        self.assertEqual(result["unclassifiedRepeatedRequests"], 0)

    def test_cross_operation_stage_key_and_self_predecessors_are_not_guessed(self):
        for field, value in (("operationId", "b" * 32), ("stage", "guide_authoring"),
                             ("requestKey", "d" * 64)):
            a = row("a")
            b = row("b", 2, **recovery("a", snap(), snap(accepted=2)))
            b[field] = value
            result = summary([a, b])
            self.assertEqual(result["recoveryRequests"], [])
            self.assertEqual(result["linkageOrRecordIssues"]["cross_task_predecessor"], 1)
        result = summary([row("b", 2, **recovery("b", snap(), snap(accepted=2)))])
        self.assertEqual(result["recoveryRequests"], [])

    def test_missing_duplicate_and_lost_links_are_explicit(self):
        b = row("b", 2, **recovery("a", snap(), snap(accepted=2)))
        for rows in ([b], [row("a"), row("a"), b]):
            result = summary(rows)
            self.assertEqual(result["linkageOrRecordIssues"]["missing_or_duplicate_predecessor"], 1)
            self.assertEqual(result["recoveryRequests"], [])
        b["payloadBytesJson"]["nativeLineage"]["linkageIncomplete"] = True
        result = summary([row("a"), b])
        self.assertEqual(result["linkageOrRecordIssues"]["incomplete_linkage"], 1)
        self.assertEqual(result["unclassifiedRepeatedRequests"], 1)

    def test_authoring_returns_and_empty_selection_do_not_claim_persistence(self):
        rows = []
        for i, items in enumerate((0, 4)):
            value = row(str(i), nativeAuthoring={
                "schemaVersion": 1, "state": "returned", "returnedItems": items,
                "itemKind": "curation_selections", "persistence": "not_observed"})
            value["stage"] = "flashcard_curation"
            rows.append(value)
        result = summary(rows)
        returned, = result["authoringReturns"]
        self.assertEqual(returned["returnedItems"], 4)
        self.assertEqual(returned["emptyReturns"], 1)
        self.assertEqual(returned["persistence"], "not_observed")
        self.assertEqual(result["checkpointRequests"], 0)
        text = USAGE.render(USAGE.summarize({"requests": rows}))
        self.assertIn("persistence not observed", text)

    def test_native_failures_and_decisions_do_not_replace_provider_status(self):
        a = row("a", state="failed")
        a["payloadBytesJson"]["nativeTask"]["errorCode"] = "stale_source"
        a["payloadBytesJson"]["nativeRecoveryDecisions"] = {
            "contract_repair_allowed": {"schemaVersion": 1, "reason": "contract_repair", "allowed": True},
            "contract_repair_denied": {"schemaVersion": 1, "reason": "contract_repair", "allowed": False}}
        result = USAGE.summarize({"requests": [a]})
        self.assertEqual(result["totals"]["failedOrInterruptedRequests"], 0)
        self.assertEqual(result["nativeOutcomes"]["nativeFailures"][0]["errorCode"], "stale_source")
        self.assertEqual(len(result["nativeOutcomes"]["recordedDecisions"]), 2)
        self.assertEqual(a["status"], "response")

    def test_malformed_unknown_version_and_private_text_are_not_rendered_as_causes(self):
        values = [row("a"), row("b"), row("c"), row("d", state="failed")]
        values[0]["payloadBytesJson"] = "invalid json"
        values[1]["payloadBytesJson"]["nativeTask"]["schemaVersion"] = 2
        values[2]["payloadBytesJson"]["nativeTask"]["state"] = ["returned"]
        values[3]["payloadBytesJson"]["nativeTask"]["errorCode"] = "PRIVATE SOURCE TEXT"
        values[3]["payloadBytesJson"]["nativeTask"]["message"] = "PRIVATE SOURCE TEXT"
        report = USAGE.summarize({"requests": values})
        self.assertEqual(report["nativeOutcomes"]["unobservedRequests"], 3)
        self.assertNotIn("PRIVATE SOURCE TEXT", json.dumps(report["nativeOutcomes"]))
        self.assertNotIn("PRIVATE SOURCE TEXT", USAGE.render(report))

    def test_repeated_snapshots_are_labeled_observations_and_inputs_unchanged(self):
        rows = [row(str(i), nativeCheckpoint={"schemaVersion": 1, "before": None,
                                              "after": snap(rejected=1)}) for i in range(2)]
        original = copy.deepcopy(rows)
        report = USAGE.summarize({"requests": rows})
        self.assertEqual(rows, original)
        self.assertEqual(report["nativeOutcomes"]["checkpointRejectionObservations"][0]["observations"], 2)
        self.assertEqual(report, USAGE.summarize(report))
        self.assertEqual(report["totals"]["totalTokens"], 24)
        self.assertEqual(report["payloadByteDiagnostics"]["serializedRequestBytes"], 400)

    def test_read_only_database_and_cli_export_native_outcomes_from_other_directory(self):
        with tempfile.TemporaryDirectory() as temp:
            db = Path(temp) / "library.sqlite3"
            with sqlite3.connect(db) as c:
                c.executescript("""
                    CREATE TABLE generation_operations(id,purpose,state,started_at,finished_at);
                    INSERT INTO generation_operations VALUES('op','document','failed',1,100);
                    CREATE TABLE generation_reuse_events(operation_id,stage);
                    CREATE TABLE generation_requests(
                        id,operation_id,stage,attempt,provider,model,status,input_tokens,output_tokens,
                        reasoning_tokens,cached_input_tokens,cache_write_tokens,total_tokens,cost,
                        response_id,finish_reason,error_code,request_key,payload_bytes_json,
                        max_output_tokens,started_at,finished_at);
                """)
                data = row("a", state="failed")["payloadBytesJson"]
                data["nativeTask"]["errorCode"] = "stale_source"
                c.execute("INSERT INTO generation_requests VALUES(" + ",".join("?" for _ in range(22)) + ")",
                          ("a", "op", "knowledge_extraction", 1, "fixture", "fixture", "response",
                           10, 2, 0, 0, 0, 12, None, None, "stop", None, "c" * 64,
                           json.dumps(data), 100, 1, 10))
            before = hashlib.sha256(db.read_bytes()).hexdigest()
            data = USAGE.read_database(db, None, False)
            self.assertEqual(data["operation"]["state"], "failed")
            process = subprocess.run([sys.executable, str(ROOT / "scripts/generation-usage.py"),
                                      "--database", str(db), "--json"], cwd=temp,
                                     check=True, capture_output=True, text=True)
            report = json.loads(process.stdout)
            self.assertEqual(report["nativeOutcomes"]["nativeFailures"][0]["errorCode"], "stale_source")
            self.assertEqual(report["totals"]["totalTokens"], 12)
            self.assertEqual(hashlib.sha256(db.read_bytes()).hexdigest(), before)


if __name__ == "__main__":
    unittest.main()
