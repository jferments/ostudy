from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class QualificationTimingTests(unittest.TestCase):
    def test_timing_helper_emits_log_and_step_summary_rows(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            summary = Path(temp) / "summary.md"
            env = os.environ.copy()
            env["GITHUB_STEP_SUMMARY"] = str(summary)
            command = """
source scripts/lib/qualification-timing.bash
ostudy_timing_init fast
ostudy_timing_start_step "React tests"
ostudy_timing_finish_step passed
ostudy_timing_finish_total passed
"""
            result = subprocess.run(
                ["bash", "-c", command],
                cwd=ROOT,
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("TIMING scope=fast status=passed", result.stdout)
            self.assertIn("phase=React tests", result.stdout)
            self.assertIn("phase=TOTAL", result.stdout)
            summary_text = summary.read_text()
            self.assertIn("### fast qualification timings", summary_text)
            self.assertIn("| React tests | passed |", summary_text)
            self.assertIn("| TOTAL | passed |", summary_text)


if __name__ == "__main__":
    unittest.main()
