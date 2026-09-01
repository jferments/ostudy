import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "render-agent-handoff.mjs"


class AgentHandoffTests(unittest.TestCase):
    def run_handoff(
        self,
        *,
        qualification_uses_base: bool = False,
        ledger_uses_base: bool = False,
        artifact_valid: bool = True,
        failure: dict | None = None,
        include_contract: bool = True,
    ) -> tuple[subprocess.CompletedProcess[str], str, str, str]:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            repository = directory / "repo"
            repository.mkdir()
            self.git(repository, "init")
            self.git(repository, "config", "user.name", "Test Runner")
            self.git(repository, "config", "user.email", "runner@example.invalid")
            (repository / "README.md").write_text("base\n", encoding="utf-8")
            self.git(repository, "add", "README.md")
            self.git(repository, "commit", "-m", "base")
            base_sha = self.git(repository, "rev-parse", "HEAD").stdout.strip()
            (repository / "docs").mkdir()
            (repository / "docs" / "example.md").write_text("change\n", encoding="utf-8")
            self.git(repository, "add", "docs/example.md")
            self.git(repository, "commit", "-m", "change")
            head_sha = self.git(repository, "rev-parse", "HEAD").stdout.strip()

            ledger_head = base_sha if ledger_uses_base else head_sha
            tested_head = base_sha if qualification_uses_base else head_sha
            contract_artifact = {
                "schemaVersion": 1,
                "valid": artifact_valid,
                "strict": False,
                "actual": {
                    "draft": True,
                    "baseSha": base_sha,
                    "headSha": head_sha,
                    "branch": "agent/example",
                    "number": 60,
                },
                "contract": {
                    "schemaVersion": 1,
                    "baseSha": base_sha,
                    "headSha": ledger_head,
                    "branch": "agent/example",
                    "executionMode": "githubConnector",
                    "currentState": "validation",
                    "nonGoals": ["No product behavior changes."],
                    "compatibilityNotes": ["No durable data format changes."],
                    "rollbackNotes": ["Revert the source-only commit."],
                    "residualRisks": ["Human review remains required."],
                    "mergeMethod": "squash",
                    "acceptance": {
                        "AC-01": {
                            "status": "implemented",
                            "evidence": ["scripts/render-agent-handoff.mjs"],
                            "tests": ["tests/scripts/test_agent_handoff.py"],
                        }
                    },
                    "validation": {
                        "static": {
                            "status": "passed",
                            "sha": ledger_head,
                            "evidence": ["run-1"],
                        },
                        "fast": {"status": "pending"},
                        "full": {"status": "pending"},
                        "android": {
                            "status": "notRequired",
                            "evidence": ["No Android surface changed."],
                        },
                    },
                    "manualAcceptance": {
                        "artifactReview": {
                            "status": "passed",
                            "evidence": ["Inspected the generated handoff."],
                        }
                    },
                    "knownFailures": [],
                },
                "warnings": (
                    ["Draft ledger head differs from the current GitHub head."]
                    if ledger_uses_base
                    else []
                ),
                "errors": [],
            }
            qualification = {
                "schemaVersion": 1,
                "generatedAt": "2026-08-20T00:00:00.000Z",
                "headSha": tested_head,
                "gate": "all-full",
                "status": "failure" if failure else "success",
                "runner": "test-runner",
                "workflow": {
                    "name": "Check",
                    "event": "pull_request",
                    "runId": "123",
                    "runAttempt": "1",
                },
                "steps": [
                    {"name": "pr-contract", "status": "success"},
                    {"name": "static", "status": "failure" if failure else "success"},
                    {"name": "fast", "status": "skipped"},
                    {"name": "linux-full", "status": "skipped" if failure else "success"},
                    {"name": "android-compile", "status": "skipped" if failure else "success"},
                    {"name": "android-smoke", "status": "skipped" if failure else "success"},
                ],
                "failure": failure,
            }
            contract_path = directory / "contract.json"
            qualification_path = directory / "qualification.json"
            output_path = directory / "handoff.md"
            contract_path.write_text(json.dumps(contract_artifact), encoding="utf-8")
            qualification_path.write_text(json.dumps(qualification), encoding="utf-8")
            arguments = ["node", str(SCRIPT)]
            if include_contract:
                arguments.extend(["--contract", str(contract_path)])
            arguments.extend(
                [
                    "--qualification",
                    str(qualification_path),
                    "--repository-root",
                    str(repository),
                    "--output",
                    str(output_path),
                ]
            )
            completed = subprocess.run(
                arguments,
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertTrue(output_path.exists(), completed.stderr)
            return completed, output_path.read_text(encoding="utf-8"), base_sha, head_sha

    def test_valid_handoff_combines_contract_qualification_and_git_evidence(self) -> None:
        completed, handoff, _, head_sha = self.run_handoff()
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("Pull request: #60", handoff)
        self.assertIn(head_sha, handoff)
        self.assertIn("Terminal criteria: 1/1", handoff)
        self.assertIn("docs/example", handoff)
        self.assertIn("No product behavior changes", handoff)
        self.assertIn("included by successful", handoff)
        self.assertIn("Requested method: squash", handoff)
        self.assertIn("Integrity status: valid", handoff)

    def test_draft_handoff_without_contract_remains_valid(self) -> None:
        completed, handoff, _, head_sha = self.run_handoff(include_contract=False)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn(head_sha, handoff)
        self.assertIn("Contract artifact: not supplied", handoff)
        self.assertIn("Integrity status: valid", handoff)

    def test_mismatched_qualification_head_is_reported_and_fails(self) -> None:
        completed, handoff, base_sha, head_sha = self.run_handoff(qualification_uses_base=True)
        self.assertEqual(completed.returncode, 2)
        self.assertNotEqual(base_sha, head_sha)
        self.assertIn("Integrity status: invalid", handoff)
        self.assertIn("does not match GitHub pull-request head", handoff)

    def test_stale_draft_ledger_is_visible_without_replacing_actual_head(self) -> None:
        completed, handoff, base_sha, head_sha = self.run_handoff(ledger_uses_base=True)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertNotEqual(base_sha, head_sha)
        self.assertIn("Integrity status: valid", handoff)
        self.assertIn("Draft ledger head", handoff)
        self.assertIn(head_sha, handoff)

    def test_invalid_contract_artifact_is_reported_and_fails(self) -> None:
        completed, handoff, _, _ = self.run_handoff(artifact_valid=False)
        self.assertEqual(completed.returncode, 2)
        self.assertIn("contract verification reported an invalid ledger", handoff)

    def test_failure_handoff_includes_fingerprint_and_focused_reproduction(self) -> None:
        failure = {
            "fingerprint": "qf-0123456789abcdef",
            "boundary": "static",
            "summary": "Static qualification failed.",
            "classificationHint": "productOrTest",
            "failedTests": [],
            "reproduction": "./scripts/check-static.sh",
            "logFilter": "warn,ostudy=debug",
            "transientEvidence": False,
            "logFiles": ["ostudy-static-check.log"],
        }
        completed, handoff, _, _ = self.run_handoff(failure=failure)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("qf-0123456789abcdef", handoff)
        self.assertIn("Focused reproduction", handoff)
        self.assertIn("check-static", handoff)

    @staticmethod
    def git(repository: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["git", *arguments],
            cwd=repository,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
        )


if __name__ == "__main__":
    unittest.main()
