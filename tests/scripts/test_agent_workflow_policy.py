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

    def test_check_workflow_cannot_omit_native_performance_after_linux_full(self) -> None:
        workflow = (ROOT / ".github" / "workflows" / "check.yml").read_text(encoding="utf-8")
        linux_offset = workflow.index("id: linux_full")
        performance_offset = workflow.index("id: ui_performance")
        android_offset = workflow.index("id: android_compile")
        self.assertLess(linux_offset, performance_offset)
        self.assertLess(performance_offset, android_offset)

        performance_step = workflow[performance_offset:android_offset]
        self.assertIn(
            "if: success() && steps.linux_full.outcome == 'success' && "
            "(steps.gate.outputs.gate == 'linux-full' || "
            "steps.gate.outputs.gate == 'all-full')",
            performance_step,
        )
        self.assertIn("bash ./scripts/run-ci-captured.sh", performance_step)
        self.assertIn("$RUNNER_TEMP/ostudy-ui-performance-check.log", performance_step)
        self.assertIn("bash ./scripts/check-ui-performance-candidate.sh", performance_step)
        self.assertIn(
            "$RUNNER_TEMP/ostudy-ui-performance-candidate-$QUALIFIED_SHA",
            performance_step,
        )
        self.assertIn("ostudy-ui-performance-check-tail.log", performance_step)

        evidence = workflow[workflow.index("id: qualification_evidence"):]
        self.assertIn("UI_PERFORMANCE_STATUS: ${{ steps.ui_performance.outcome }}", evidence)
        self.assertIn('--step "ui-performance=${UI_PERFORMANCE_STATUS:-skipped}"', evidence)
        self.assertGreaterEqual(evidence.count("ostudy-ui-performance-check.log"), 3)
        self.assertIn(
            "ostudy-ui-performance-candidate-${{ env.QUALIFIED_SHA }}/manifest.json",
            evidence,
        )
        self.assertIn(
            "ostudy-ui-performance-candidate-${{ env.QUALIFIED_SHA }}/measurements/deck-drag-v1.json",
            evidence,
        )


if __name__ == "__main__":
    unittest.main()

