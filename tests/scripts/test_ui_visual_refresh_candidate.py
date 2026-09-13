from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WEBDRIVER_DIR = ROOT / "tests" / "webdriver"
CANDIDATE_PATH = WEBDRIVER_DIR / "ui_visual_refresh_candidate.py"


def load_candidate_module():
    sys.path.insert(0, str(WEBDRIVER_DIR))
    try:
        spec = importlib.util.spec_from_file_location("ostudy_ui_visual_refresh_candidate", CANDIDATE_PATH)
        if spec is None or spec.loader is None:
            raise RuntimeError("Could not load UI visual-refresh candidate module")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module
    finally:
        sys.path.remove(str(WEBDRIVER_DIR))


class UiVisualRefreshCandidateTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.candidate = load_candidate_module()

    def test_candidate_configuration_uses_current_refresh_identity(self) -> None:
        source_sha = "a" * 40
        self.candidate.configure_candidate(source_sha)
        verifier = self.candidate.baseline
        self.assertEqual(verifier.BASELINE_ID, "ui-visual-refresh-candidate-v1")
        self.assertEqual(verifier.BASELINE_SOURCE_SHA, source_sha)
        self.assertEqual(verifier.SCENARIO_VERSION, "flashcards-library-visual-refresh-v1")
        self.assertIn(
            "flashcards-library-visual-refresh-v1",
            verifier.canonical_filename("light", "wide", 1440, 900),
        )

    def test_default_output_is_ignored_candidate_evidence(self) -> None:
        source_sha = "b" * 40
        destination = self.candidate.default_destination(source_sha)
        self.assertEqual(destination.parent, ROOT / ".artifacts")
        self.assertIn(source_sha[:12], destination.name)

    def test_historical_stage_zero_destination_is_rejected(self) -> None:
        with self.assertRaisesRegex(ValueError, "must not overwrite"):
            self.candidate.ensure_safe_destination(self.candidate.baseline.BASELINE_DIR)

    def test_rejects_abbreviated_source_sha(self) -> None:
        with self.assertRaisesRegex(ValueError, "full lowercase Git SHA"):
            self.candidate.configure_candidate("deadbeef")


if __name__ == "__main__":
    unittest.main()
