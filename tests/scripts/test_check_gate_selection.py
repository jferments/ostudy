import os
import re
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "select-check-gate.mjs"
WORKFLOW = ROOT / ".github" / "workflows" / "check.yml"


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
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(completed.stdout.strip(), "fast")

    def test_pr_body_marker_cannot_force_full_while_draft(self) -> None:
        completed = self.select(
            EVENT_NAME="pull_request",
            PR_DRAFT="true",
            PR_BODY="before\n<!-- ostudy-check:all-full -->\nafter",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(completed.stdout.strip(), "fast")

    def test_ready_pull_request_uses_all_full(self) -> None:
        completed = self.select(
            EVENT_NAME="pull_request",
            PR_DRAFT="false",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(completed.stdout.strip(), "all-full")

    def test_manual_dispatch_uses_selected_gate(self) -> None:
        completed = self.select(
            EVENT_NAME="workflow_dispatch",
            DISPATCH_GATE="linux-full",
            PR_DRAFT="true",
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

    def test_product_ci_does_not_run_for_pr_body_edits(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        match = re.search(
            r"pull_request:\n\s+branches:\s*\[main\]\n\s+types:\s*\[([^]]+)\]",
            workflow,
        )
        self.assertIsNotNone(match)
        assert match is not None
        event_types = {value.strip() for value in match.group(1).split(",")}
        self.assertEqual(
            event_types,
            {
                "opened",
                "synchronize",
                "reopened",
                "ready_for_review",
                "converted_to_draft",
                "closed",
            },
        )
        self.assertNotIn("PR_BODY:", workflow)
        self.assertNotIn("ostudy-check:all-full", workflow)
        self.assertIn(
            "group: ${{ github.workflow }}-${{ github.event.pull_request.head.ref || github.ref_name }}",
            workflow,
        )
        self.assertIn(
            "cancel-in-progress: ${{ github.event_name != 'workflow_dispatch' }}",
            workflow,
        )
        self.assertIn(
            "name: ${{ github.event_name == 'workflow_dispatch' && 'manual-check' || 'check' }}",
            workflow,
        )
        self.assertIn("if: github.event.action != 'closed'", workflow)
        self.assertIn(
            "if: github.event_name == 'pull_request' && github.event.pull_request.draft == false",
            workflow,
        )


if __name__ == "__main__":
    unittest.main()
