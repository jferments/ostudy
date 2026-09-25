"""Regression tests for baseline/current generation acceptance evaluation."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "generation_acceptance", ROOT / "scripts/generation-acceptance.py")
ACCEPTANCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ACCEPTANCE)


def report(wall_ms: int, input_tokens: int, output_tokens: int,
           requests: int, retries: int, cache_rate: float = 0.0):
    return {
        "schemaVersion": 1,
        "operationId": "a" * 32,
        "operation": {
            "id": "a" * 32,
            "purpose": "document_generation",
            "state": "succeeded",
            "startedAt": 1_000,
            "finishedAt": 1_000 + wall_ms,
        },
        "wallClockMs": wall_ms,
        "usageComplete": True,
        "totals": {
            "inputTokens": input_tokens,
            "outputTokens": output_tokens,
            "requests": requests,
            "repeatedLogicalRequests": retries,
            "cacheReadPercentOfCoveredInput": cache_rate,
        },
    }


def quality(current_override=None):
    fields = {
        "sourceFidelity": 0.95,
        "importantTopicCoverage": 0.90,
        "qualificationsAndUnits": 0.95,
        "answerKeyCorrectness": 0.98,
        "questionTypeDiversity": 0.85,
        "visualTableInterpretation": 0.90,
    }
    current = dict(fields)
    if current_override:
        current.update(current_override)
    return {
        "applicable": list(fields),
        "baseline": fields,
        "current": current,
    }


class GenerationAcceptanceTests(unittest.TestCase):
    def test_acceptance_separates_quality_floor_from_engineering_targets(self):
        baseline = report(1_000_000, 1_800_000, 500_000, 100, 30, 5)
        current = report(450_000, 700_000, 300_000, 58, 8, 35)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "quality.json"
            path.write_text(json.dumps(quality()), encoding="utf-8")
            result = ACCEPTANCE.compare(
                baseline, current, "cold", ACCEPTANCE.load_quality(path))

        self.assertTrue(result["acceptanceReady"])
        self.assertTrue(result["engineeringTargetsMet"])
        self.assertGreaterEqual(
            result["performance"]["change"]["wallClockReductionPercent"], 50)
        self.assertGreaterEqual(
            result["performance"]["change"]["reportedTokenReductionPercent"], 50)
        self.assertEqual(result["quality"]["regressions"], [])

    def test_quality_regression_blocks_acceptance_even_when_faster(self):
        baseline = report(1_000_000, 1_800_000, 500_000, 100, 30)
        current = report(300_000, 500_000, 200_000, 40, 4)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "quality.json"
            path.write_text(json.dumps(
                quality({"importantTopicCoverage": 0.80})), encoding="utf-8")
            result = ACCEPTANCE.compare(
                baseline, current, "cold", ACCEPTANCE.load_quality(path))

        self.assertFalse(result["acceptanceReady"])
        self.assertIn("importantTopicCoverage", result["quality"]["regressions"])
        self.assertTrue(result["engineeringTargetsMet"])

    def test_incomplete_provider_usage_is_not_accepted(self):
        baseline = report(1000, 100, 50, 3, 0)
        current = report(500, 50, 25, 2, 0)
        current["usageComplete"] = False
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "quality.json"
            path.write_text(json.dumps(quality()), encoding="utf-8")
            result = ACCEPTANCE.compare(
                baseline, current, "resume", ACCEPTANCE.load_quality(path))
        self.assertFalse(result["performance"]["comparable"])
        self.assertFalse(result["acceptanceReady"])

    def test_text_only_benchmark_can_mark_visual_rubric_not_applicable(self):
        value = quality()
        value["applicable"].remove("visualTableInterpretation")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "quality.json"
            path.write_text(json.dumps(value), encoding="utf-8")
            loaded = ACCEPTANCE.load_quality(path)
        self.assertTrue(loaded["complete"])
        self.assertNotIn("visualTableInterpretation", loaded["fields"])


if __name__ == "__main__":
    unittest.main()
