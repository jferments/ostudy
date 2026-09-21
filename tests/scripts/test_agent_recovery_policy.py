import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "verify-agent-recovery-policy.mjs"

FILES = {
    "AGENTS.md": "ANCHORLINE RESET\nnever means\nDo not ask the human to repeat\ndocs/agents/RECOVERY-PROTOCOL.md\n",
    "docs/agents/RECOVERY-PROTOCOL.md": (
        "One bounded resumption read\nKeep an active checkpoint\nTool and access discipline\n"
        "direct PR and branch-ref reads\nambiguous acknowledgement\n"
        "Progress and response discipline\nWorkflow success and gate coverage are separate facts\n"
    ),
    "docs/agents/GITHUB-DIRECT-DEVELOPMENT.md": (
        "Interruption-resilient execution\nEfficient GitHub reads and publication\nKnown branch\n"
        "No mid-composition polling\nBlob proportionality\nAmbiguous mutation recovery\n"
        "Publication checkpointing\nANCHORLINE RESET\nDo not ask the human to repeat\nsubstantive response\n"
    ),
    "docs/agents/GITHUB-CONNECTOR.md": (
        "session-local action exposure\ndoes not revoke repository permission\n"
        "Do not rediscover a working action surface\n"
    ),
    "docs/agents/COMMAND-RUNBOOK.md": "verify-agent-recovery-policy.mjs\n",
    "scripts/check-static.sh": "verify-agent-recovery-policy.mjs\n",
}


class AgentRecoveryPolicyTests(unittest.TestCase):
    def write_fixture(self, root: Path) -> None:
        for relative, content in FILES.items():
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content, encoding="utf-8")

    def run_verifier(self, root: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["node", str(SCRIPT), "--root", str(root)],
            text=True,
            capture_output=True,
            check=False,
        )

    def test_repository_contract_passes(self) -> None:
        completed = subprocess.run(
            ["node", str(SCRIPT)],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_complete_fixture_passes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            completed = self.run_verifier(root)
            self.assertEqual(completed.returncode, 0, completed.stderr)

    def test_missing_recovery_anchor_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            (root / "AGENTS.md").write_text("ANCHORLINE RESET\n", encoding="utf-8")
            completed = self.run_verifier(root)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("Do not ask the human to repeat", completed.stderr)

    def test_missing_ambiguous_mutation_recovery_contract_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            direct = root / "docs" / "agents" / "GITHUB-DIRECT-DEVELOPMENT.md"
            direct.write_text(
                direct.read_text(encoding="utf-8").replace("Ambiguous mutation recovery\n", ""),
                encoding="utf-8",
            )
            completed = self.run_verifier(root)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("Ambiguous mutation recovery", completed.stderr)

    def test_missing_direct_read_efficiency_contract_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.write_fixture(root)
            direct = root / "docs" / "agents" / "GITHUB-DIRECT-DEVELOPMENT.md"
            direct.write_text("Interruption-resilient execution\nANCHORLINE RESET\nDo not ask the human to repeat\nsubstantive response\n", encoding="utf-8")
            completed = self.run_verifier(root)
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("Efficient GitHub reads and publication", completed.stderr)


if __name__ == "__main__":
    unittest.main()
