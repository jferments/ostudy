import json
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "select-check-gate.mjs"
WORKFLOW = ROOT / ".github" / "workflows" / "check.yml"
PR_CONTRACT_WORKFLOW = ROOT / ".github" / "workflows" / "pr-contract.yml"


class CheckGateSelectionTests(unittest.TestCase):
    def select(self, **environment: str) -> subprocess.CompletedProcess[str]:
        env = os.environ.copy()
        # Tests that model an unlabeled PR must not inherit the real Actions
        # event payload from the self-hosted runner. Label-specific cases pass
        # an explicit temporary GITHUB_EVENT_PATH below.
        env.pop("GITHUB_EVENT_PATH", None)
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

    def select_with_labels(self, labels: list[str]) -> subprocess.CompletedProcess[str]:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".json") as handle:
            json.dump(
                {"pull_request": {"labels": [{"name": label} for label in labels]}},
                handle,
            )
            handle.flush()
            return self.select(
                EVENT_NAME="pull_request",
                PR_DRAFT="true",
                GITHUB_EVENT_PATH=handle.name,
            )

    def test_draft_pull_request_uses_fast_by_default(self) -> None:
        completed = self.select(
            EVENT_NAME="pull_request",
            PR_DRAFT="true",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(completed.stdout.strip(), "fast")

    def test_full_qualification_label_runs_all_full_while_pr_remains_draft(self) -> None:
        completed = self.select_with_labels(["qualification:full"])
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(completed.stdout.strip(), "all-full")

    def test_unrelated_labels_leave_draft_on_fast_gate(self) -> None:
        completed = self.select_with_labels(["documentation", "ui"])
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

    def test_product_ci_does_not_run_for_pr_body_edits_or_validate_metadata(self) -> None:
        workflow = WORKFLOW.read_text(encoding="utf-8")
        event_types = self.pull_request_event_types(workflow)
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
        self.assertNotIn("edited", event_types)
        self.assertNotIn("PR_BODY:", workflow)
        self.assertNotIn("ostudy-check:all-full", workflow)
        self.assertNotIn("verify-pr-contract.mjs", workflow)
        self.assertNotIn("ostudy-pr-contract.json", workflow)
        self.assertIn(
            "group: ${{ github.workflow }}-${{ github.event_name == 'workflow_dispatch' && 'manual' || 'automatic' }}-${{ github.event.pull_request.head.repo.full_name || github.repository }}-${{ github.event.pull_request.head.ref || github.ref_name }}",
            workflow,
        )
        self.assertIn("cancel-in-progress: true", workflow)
        self.assertIn(
            "name: ${{ github.event_name == 'workflow_dispatch' && 'manual-check' || 'check' }}",
            workflow,
        )
        self.assertIn("if: github.event.action != 'closed'", workflow)

    def test_pr_contract_workflow_runs_on_metadata_and_source_lifecycle_events(self) -> None:
        workflow = PR_CONTRACT_WORKFLOW.read_text(encoding="utf-8")
        event_types = self.pull_request_event_types(workflow)
        self.assertEqual(
            event_types,
            {
                "opened",
                "edited",
                "synchronize",
                "reopened",
                "ready_for_review",
                "converted_to_draft",
            },
        )
        self.assertIn("name: PR Contract", workflow)
        self.assertIn("permissions:\n  contents: read", workflow)
        self.assertIn("persist-credentials: false", workflow)
        self.assertIn("verify-pr-contract.mjs", workflow)
        self.assertIn("Product qualification: **independent; not run by this workflow**", workflow)
        for expensive_command in [
            "check-fast.sh",
            "check.sh",
            "android-test.sh",
            "android-emulator-smoke.sh",
        ]:
            self.assertNotIn(expensive_command, workflow)

    @staticmethod
    def pull_request_event_types(workflow: str) -> set[str]:
        match = re.search(
            r"pull_request:\n\s+branches:\s*\[main\]\n\s+types:\s*\[([^]]+)\]",
            workflow,
        )
        if match is None:
            raise AssertionError("pull_request event types were not found")
        return {value.strip() for value in match.group(1).split(",")}


if __name__ == "__main__":
    unittest.main()
