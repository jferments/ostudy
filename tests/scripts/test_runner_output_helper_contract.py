from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class RunnerOutputHelperContractTests(unittest.TestCase):
    def test_runner_output_helpers_are_present_and_bounded(self) -> None:
        persistence = (ROOT / "scripts" / "persist-runner-output.sh").read_text()
        pruner = (ROOT / "scripts" / "prune-runner-output.py").read_text()
        self.assertIn("OSTUDY_RUNNER_OUTPUT_RETENTION_DAYS:-7", persistence)
        self.assertIn("OSTUDY_RUNNER_OUTPUT_MAX_BYTES:-21474836480", persistence)
        self.assertIn("prune-runner-output.py", persistence)
        self.assertIn('RUN_DIR = re.compile(r"^[0-9]+-[0-9]+$")', pruner)
        self.assertIn("item.name != current", pruner)
        self.assertIn("resolved.parent != resolved_root", pruner)


if __name__ == "__main__":
    unittest.main()
