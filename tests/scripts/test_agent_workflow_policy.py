import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "verify-agent-workflow-policy.mjs"


class AgentWorkflowPolicyTests(unittest.TestCase):
    def run_policy(self, workflows: dict[str, str], scripts: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            workflow_directory = fixture / ".github" / "workflows"
            scripts_directory = fixture / "scripts"
            workflow_directory.mkdir(parents=True)
            scripts_directory.mkdir(parents=True)
            for name, content in workflows.items():
                (workflow_directory / name).write_text(content, encoding="utf-8")
            for name, content in (scripts or {}).items():
                path = scripts_directory / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
            return subprocess.run(
                ["node", str(SCRIPT), "--root", str(fixture)],
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )

    def test_read_only_pull_request_qualification_passes(self) -> None:
        completed = self.run_policy(
            {
                "check.yml": """name: Check
on:
  pull_request:
permissions:
  contents: read
jobs:
  check:
    runs-on: [self-hosted, linux]
    steps:
      - uses: actions/checkout@1111111111111111111111111111111111111111 # v1
        with:
          persist-credentials: false
      - run: ./scripts/check.sh
"""
            }
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_pull_request_workflow_cannot_write_or_push(self) -> None:
        completed = self.run_policy(
            {
                "check.yml": """name: Check
on:
  pull_request:
permissions:
  contents: write
jobs:
  check:
    runs-on: [self-hosted]
    steps:
      - run: |
          git add -A
          git commit -m generated
          git push
"""
            }
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("write permissions", completed.stderr)
        self.assertIn("may not push", completed.stderr)
        self.assertIn("may not author commits", completed.stderr)
        self.assertIn("may not stage", completed.stderr)

    def test_inline_write_permissions_are_rejected(self) -> None:
        completed = self.run_policy(
            {
                "check.yml": """name: Check
on:
  pull_request:
permissions: {contents: write, pull-requests: write}
jobs:
  check:
    runs-on: [self-hosted]
    steps:
      - run: ./scripts/check.sh
"""
            }
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("write permissions", completed.stderr)

    def test_inline_pull_request_metadata_write_is_rejected(self) -> None:
        completed = self.run_policy(
            {
                "check.yml": """name: Check
on:
  pull_request:
permissions: {contents: read, pull-requests: write}
jobs:
  check:
    runs-on: [self-hosted]
    steps:
      - run: ./scripts/check.sh
"""
            }
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("write permissions", completed.stderr)

    def test_write_all_is_rejected(self) -> None:
        completed = self.run_policy(
            {
                "check.yml": """name: Check
on:
  pull_request:
permissions: write-all
jobs:
  check:
    runs-on: [self-hosted]
    steps:
      - run: ./scripts/check.sh
"""
            }
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("write permissions", completed.stderr)

    def test_pull_request_workflow_cannot_mutate_readiness(self) -> None:
        completed = self.run_policy(
            {
                "check.yml": """name: Check
on:
  pull_request:
jobs:
  check:
    runs-on: [self-hosted]
    steps:
      - uses: actions/checkout@1111111111111111111111111111111111111111 # v1
        with:
          persist-credentials: true
      - run: gh pr ready 12
"""
            }
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("persist write credentials", completed.stderr)
        self.assertIn("draft/readiness", completed.stderr)

    def test_pull_request_metadata_write_is_rejected(self) -> None:
        completed = self.run_policy(
            {
                "check.yml": """name: Check
on:
  pull_request:
permissions:
  contents: read
  pull-requests: write
jobs:
  check:
    runs-on: [self-hosted]
    steps:
      - run: ./scripts/check.sh
"""
            }
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("write permissions", completed.stderr)

    def test_manual_maintenance_workflow_may_use_narrow_write_boundary(self) -> None:
        completed = self.run_policy(
            {
                "maintain-generated-candidates.yml": """name: Maintain generated candidates
on:
  workflow_dispatch:
permissions:
  contents: write
jobs:
  maintain:
    runs-on: [self-hosted]
    steps:
      - run: git push origin HEAD:agent/example
"""
            }
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_task_specific_applicator_is_rejected(self) -> None:
        completed = self.run_policy(
            {"check.yml": "name: Check\non:\n  workflow_dispatch:\n"},
            {"agent-pr61-applicator.py": "print('rewrite source')\n"},
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("task-specific applicator", completed.stderr)

    def test_required_check_workflow_is_source_driven(self) -> None:
        workflow = (ROOT / ".github" / "workflows" / "check.yml").read_text(encoding="utf-8")
        selector = (ROOT / "scripts" / "select-check-gate.mjs").read_text(encoding="utf-8")

        self.assertIn(
            "types: [opened, synchronize, reopened, ready_for_review, converted_to_draft, closed]",
            workflow,
        )
        self.assertNotIn("edited", workflow)
        self.assertNotIn("PR_BODY", workflow)
        self.assertNotIn("ostudy-check:all-full", workflow)
        self.assertIn("DISPATCH_GATE: ${{ inputs.gate || 'all-full' }}", workflow)

        self.assertIn("if (eventName === \"workflow_dispatch\")", selector)
        self.assertIn("gate = dispatchGate", selector)
        self.assertIn('eventName === "pull_request" && pullRequestDraft', selector)
        self.assertIn('gate = "fast"', selector)
        self.assertIn('gate = "all-full"', selector)

        self.assertIn(
            "github.event_name == 'workflow_dispatch' && 'manual' || 'automatic'",
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
        self.assertIn("permissions:\n  contents: read", workflow)
        self.assertIn("persist-credentials: false", workflow)

    def test_required_check_workflow_keeps_performance_advisory(self) -> None:
        workflow = (ROOT / ".github" / "workflows" / "check.yml").read_text(encoding="utf-8")

        self.assertNotIn("id: ui_performance", workflow)
        self.assertNotIn("check-ui-performance-candidate.sh", workflow)
        self.assertTrue((ROOT / "scripts" / "check-ui-performance-candidate.sh").is_file())
        self.assertTrue((ROOT / "tests" / "performance-baselines").is_dir())


if __name__ == "__main__":
    unittest.main()
