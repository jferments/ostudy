import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "verify-agent-workflow-policy.mjs"


class AgentWorkflowContinuationPolicyTests(unittest.TestCase):
    def run_policy(self, workflow: str) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temporary:
            fixture = Path(temporary)
            workflow_directory = fixture / ".github" / "workflows"
            (fixture / "scripts").mkdir(parents=True)
            workflow_directory.mkdir(parents=True)
            (workflow_directory / "validate.yml").write_text(workflow, encoding="utf-8")
            return subprocess.run(
                ["node", str(SCRIPT), "--root", str(fixture)],
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )

    def test_multiline_cargo_fmt_check_is_read_only(self) -> None:
        completed = self.run_policy(
            """name: Validate
on:
  workflow_dispatch:
permissions:
  contents: read
jobs:
  validate:
    runs-on: [self-hosted]
    steps:
      - run: |
          cargo fmt \\
            --manifest-path src-tauri/Cargo.toml \\
            --all \\
            -- \\
            --check
          git diff --exit-code
"""
        )

        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_multiline_cargo_fmt_without_check_is_rejected(self) -> None:
        completed = self.run_policy(
            """name: Rewrite
on:
  push:
permissions:
  contents: read
jobs:
  rewrite:
    runs-on: [self-hosted]
    steps:
      - run: |
          cargo fmt \\
            --manifest-path src-tauri/Cargo.toml \\
            --all
"""
        )

        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("check-only formatter modes", completed.stderr)


if __name__ == "__main__":
    unittest.main()
