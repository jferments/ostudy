import os
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "select-check-gate.mjs"


class CheckGateSelectionTests(unittest.TestCase):
    def select(self, **environment: str) -> subprocess.CompletedProcess[str]:
        env = os.environ.copy()
        env.update(environment)
        return subprocess.run(
            ["node", str(SCRIPT)],
            cwd=ROOT,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def test_draft_pull_request_uses_fast_by_default(self) -> None:
        completed = self.select(
            EVENT_NAME="pull_request",
            PR_DRAFT="true",
            PR_BODY="ordinary draft",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(completed.stdout.strip(), "fast")

    def test_exact_marker_requests_all_full_while_draft(self) -> None:
        completed = self.select(
            EVENT_NAME="pull_request",
            PR_DRAFT="true",
            PR_BODY="before\n<!-- ostudy-check:all-full -->\nafter",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(completed.stdout.strip(), "all-full")

    def test_similar_text_does_not_trigger_expensive_gate(self) -> None:
        completed = self.select(
            EVENT_NAME="pull_request",
            PR_DRAFT="true",
            PR_BODY="Please run ostudy-check all-full eventually.",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(completed.stdout.strip(), "fast")

    def test_ready_pull_request_uses_all_full(self) -> None:
        completed = self.select(
            EVENT_NAME="pull_request",
            PR_DRAFT="false",
            PR_BODY="",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(completed.stdout.strip(), "all-full")

    def test_manual_dispatch_uses_selected_gate(self) -> None:
        completed = self.select(
            EVENT_NAME="workflow_dispatch",
            DISPATCH_GATE="linux-full",
            PR_DRAFT="true",
            PR_BODY="<!-- ostudy-check:all-full -->",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(completed.stdout.strip(), "linux-full")

    def test_invalid_manual_gate_fails_closed(self) -> None:
        completed = self.select(
            EVENT_NAME="workflow_dispatch",
            DISPATCH_GATE="write-source",
        )
        self.assertEqual(completed.returncode, 2)
        self.assertIn("Invalid qualification gate", completed.stderr)


if __name__ == "__main__":
    unittest.main()
