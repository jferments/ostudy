"""Candidate admission reporting preserves failures, unknowns and selection."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("candidate_outcomes", ROOT / "scripts/generation_outcomes.py")
OUTCOMES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(OUTCOMES)


def record(admitted=1, rejected=1, selected=1, state="returned", declared=2):
    return {
        "schemaVersion": 1, "state": state, "declaredCandidates": declared,
        "examinedCandidates": admitted + rejected, "admittedCandidates": admitted,
        "rejectedCandidates": rejected, "selectedCandidates": selected,
        "unselectedAdmittedCandidates": None if selected is None else admitted - selected,
        "unexaminedCandidates": None if declared is None else declared - admitted - rejected,
        "rejections": [{"candidateIndex": index, "rule": "knowledge_selection",
                        "errorCode": "ai_document_output_invalid", "retryable": True}
                       for index in range(rejected)],
        "failureBoundary": None if state == "returned" else "all_candidates_rejected",
        "errorCode": None if state == "returned" else "ai_document_output_invalid",
        "persistence": "not_observed",
    }


def request(admission, **extra):
    return {"stage": "flashcard_authoring", "status": "response", "attempt": 1,
            "payloadBytesJson": {"nativeAdmission": admission}, **extra}


class CandidateOutcomeTests(unittest.TestCase):
    def test_partial_admission_and_selection_have_separate_totals(self):
        rows = [request(record()), request(record(2, 0, 1))]
        before = copy.deepcopy(rows)
        report = OUTCOMES.summarize(rows)
        summary = report["candidateAdmission"]
        self.assertEqual(summary["observedRequests"], 2)
        self.assertEqual(summary["examinedCandidates"], 4)
        self.assertEqual(summary["admittedCandidates"], 3)
        self.assertEqual(summary["rejectedCandidates"], 1)
        self.assertEqual(summary["selectedCandidates"], 2)
        self.assertEqual(summary["unselectedAdmittedCandidates"], 1)
        self.assertEqual(summary["selectionObservedRequests"], 2)
        self.assertEqual(summary["persistence"], "not_observed")
        self.assertEqual(summary["rejectionObservations"][0]["rule"], "knowledge_selection")
        self.assertEqual(rows, before)
        self.assertEqual(report["checkpointRequests"], 0)
        self.assertEqual(report["observedRequests"], 0)  # No nativeTask was fabricated.
        rendered = "\n".join(OUTCOMES.render(report))
        self.assertIn("admitted but not selected 1", rendered)
        self.assertIn("Candidate rejection: knowledge_selection", rendered)
        self.assertIn("does not certify factual quality", rendered)

    def test_all_rejected_is_a_failure_not_a_valid_empty_selection(self):
        summary = OUTCOMES.summarize([request(record(0, 2, None, "failed"))])["candidateAdmission"]
        self.assertEqual(summary["responseStates"], {"failed": 1})
        self.assertEqual(summary["rejectedCandidates"], 2)
        self.assertIsNone(summary["selectedCandidates"])
        self.assertEqual(summary["selectionObservedRequests"], 0)
        self.assertEqual(summary["failureBoundaries"][0]["boundary"], "all_candidates_rejected")
        empty = OUTCOMES.summarize([request(record(0, 0, 0, declared=0))])["candidateAdmission"]
        self.assertEqual(empty["selectedCandidates"], 0)
        self.assertEqual(empty["selectionObservedRequests"], 1)
        self.assertEqual(empty["responseStates"], {"returned": 1})

    def test_malformed_envelope_and_size_limit_leave_unexamined_content_unknown(self):
        malformed = record(0, 0, None, "failed", None)
        malformed["failureBoundary"] = "response_envelope"
        oversize = record(0, 0, None, "failed", 129)
        oversize["failureBoundary"] = "response_bounds"
        summary = OUTCOMES.summarize([request(malformed), request(oversize)])["candidateAdmission"]
        self.assertEqual(summary["observedRequests"], 2)
        self.assertEqual(summary["examinedCandidates"], 0)
        self.assertEqual(summary["rejectedCandidates"], 0)
        self.assertIsNone(summary["selectedCandidates"])
        self.assertEqual(len(summary["failureBoundaries"]), 2)

    def test_legacy_and_unknown_versions_never_acquire_success(self):
        rows = [{"stage": "flashcard_authoring", "status": "response"},
                request({"schemaVersion": 999}), {"stage": "guide_authoring"}]
        summary = OUTCOMES.summarize(rows)["candidateAdmission"]
        self.assertEqual(summary["observedRequests"], 0)
        self.assertEqual(summary["unobservedFlashcardRequests"], 2)
        self.assertEqual(summary["invalidRecords"], 1)
        self.assertIsNone(summary["admittedCandidates"])
        self.assertIsNone(summary["selectedCandidates"])

    def test_invalid_counts_duplicate_indices_and_wrong_stages_are_not_aggregated(self):
        bad_records = []
        for key, value in [("admittedCandidates", True), ("selectedCandidates", 3),
                           ("examinedCandidates", 0), ("unexaminedCandidates", 3),
                           ("unexaminedCandidates", False), ("unselectedAdmittedCandidates", False),
                           ("persistence", "published"), ("rejectedCandidates", 129),
                           ("rejections", "PRIVATE_BAD")]:
            bad = record()
            bad[key] = value
            bad_records.append(request(bad))
        duplicate = record(0, 2, None, "failed")
        duplicate["rejections"][1]["candidateIndex"] = 0
        bad_records.extend([request(duplicate), request(record(), stage="guide_authoring")])
        summary = OUTCOMES.summarize(bad_records)["candidateAdmission"]
        self.assertEqual(summary["invalidRecords"], len(bad_records))
        self.assertEqual(summary["observedRequests"], 0)
        self.assertFalse(summary["rejectionObservations"])
        self.assertNotIn("PRIVATE_BAD", json.dumps(summary))

    def test_retry_reports_preserve_each_observation_and_never_count_cost_per_candidate(self):
        rows = [request(record(0, 2, None, "failed"), inputTokens=100),
                request(record(1, 0, 1, declared=1), attempt=2, inputTokens=100)]
        summary = OUTCOMES.summarize(rows)
        self.assertEqual(summary["candidateAdmission"]["rejectedCandidates"], 2)
        self.assertEqual(summary["candidateAdmission"]["admittedCandidates"], 1)
        self.assertEqual(summary["candidateAdmission"]["selectedCandidates"], 1)
        self.assertEqual(summary["unclassifiedRepeatedRequests"], 1)
        self.assertEqual(summary["recoveryRequests"], [])  # No invented recovery linkage.
        self.assertNotIn("inputTokens", summary["candidateAdmission"])

    def test_json_payload_reimport_and_unexpected_text_are_safe(self):
        admission = record()
        admission["rejections"][0]["message"] = "PRIVATE_CARD_TEXT"
        admission["rejections"][0]["rule"] = "PRIVATE GENERATED ERROR WITH SPACES"
        row = request(admission)
        expected = OUTCOMES.summarize([row])
        row["payloadBytesJson"] = json.dumps(row["payloadBytesJson"])
        self.assertEqual(OUTCOMES.summarize([row]), expected)
        self.assertNotIn("PRIVATE_", json.dumps(expected))
        self.assertEqual(expected["candidateAdmission"]["rejectionObservations"][0]["rule"], "unclassified")

    def test_every_previous_summary_field_is_unchanged_for_legacy_data(self):
        # Legacy inputs have neither a candidate report nor a native task. The
        # new admission projection must not alter provider/linkage semantics.
        rows = [{"stage": "flashcard_authoring", "status": "response", "attempt": 2,
                 "inputTokens": 100, "outputTokens": 20, "payloadBytesJson": {}}]
        summary = OUTCOMES.summarize(rows)
        self.assertEqual(summary["requestStates"], {"unobserved": 1})
        self.assertEqual(summary["unclassifiedRepeatedRequests"], 1)
        self.assertEqual(summary["nativeFailures"], [])
        self.assertEqual(summary["authoringReturns"], [])
        self.assertEqual(summary["candidateAdmission"]["unobservedFlashcardRequests"], 1)


if __name__ == "__main__":
    unittest.main()
