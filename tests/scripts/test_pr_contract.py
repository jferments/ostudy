import json
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "verify-pr-contract.mjs"
BASE_SHA = "1" * 40
HEAD_SHA = "2" * 40


def ledger(contract: dict) -> str:
    return (
        "## State\n\n"
        "<!-- ostudy-agent-state:start -->\n"
        "```json\n"
        f"{json.dumps(contract, indent=2)}\n"
        "```\n"
        "<!-- ostudy-agent-state:end -->\n"
    )


def base_contract() -> dict:
    return {
        "schemaVersion": 1,
        "baseSha": BASE_SHA,
        "headSha": HEAD_SHA,
        "branch": "agent/test-contract",
        "executionMode": "githubConnector",
        "currentState": "implementation",
        "acceptance": {
            "AC-01": {
                "status": "implemented",
                "evidence": ["scripts/verify-pr-contract.mjs"],
                "tests": ["tests/scripts/test_pr_contract.py"],
            },
            "AC-02": {"status": "inProgress", "evidence": [], "tests": []},
        },
        "validation": {
            "static": {"status": "pending"},
            "fast": {"status": "pending"},
            "full": {"status": "pending"},
            "android": {"status": "notRequired"},
        },
        "manualAcceptance": {"review": {"status": "pending"}},
        "knownFailures": [],
    }


def event_for(contract: dict, *, draft: bool) -> dict:
    return {
        "pull_request": {
            "number": 99,
            "draft": draft,
            "body": ledger(contract),
            "base": {"sha": BASE_SHA, "ref": "main"},
            "head": {"sha": HEAD_SHA, "ref": "agent/test-contract"},
        }
    }


class PullRequestContractTests(unittest.TestCase):
    def run_contract(self, contract: dict, *, draft: bool = True) -> tuple[subprocess.CompletedProcess[str], dict]:
        with tempfile.TemporaryDirectory() as temporary:
            event_path = Path(temporary) / "event.json"
            output_path = Path(temporary) / "result.json"
            event_path.write_text(json.dumps(event_for(contract, draft=draft)), encoding="utf-8")
            completed = subprocess.run(
                [
                    "node",
                    str(SCRIPT),
                    "--event",
                    str(event_path),
                    "--output",
                    str(output_path),
                ],
                cwd=ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertTrue(output_path.exists())
            result = json.loads(output_path.read_text(encoding="utf-8"))
            return completed, result

    def test_valid_draft_contract_passes(self) -> None:
        completed, result = self.run_contract(base_contract())
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertTrue(result["valid"])

    def test_draft_identity_lag_warns_without_claiming_current_validation(self) -> None:
        contract = base_contract()
        contract["headSha"] = "3" * 40
        completed, result = self.run_contract(contract)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("does not match GitHub", completed.stderr)
        self.assertTrue(result["warnings"])

    def test_terminal_acceptance_requires_evidence(self) -> None:
        contract = base_contract()
        contract["acceptance"]["AC-01"]["evidence"] = []
        completed, _ = self.run_contract(contract)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("terminal but has no", completed.stderr)

    def test_passed_validation_must_match_ledger_head(self) -> None:
        contract = base_contract()
        contract["validation"]["static"] = {
            "status": "passed",
            "sha": "4" * 40,
            "evidence": ["run-1"],
        }
        completed, _ = self.run_contract(contract)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("must match the ledger headSha", completed.stderr)

    def test_ready_pull_request_rejects_incomplete_state(self) -> None:
        completed, _ = self.run_contract(base_contract(), draft=False)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("readyForReview", completed.stderr)
        self.assertIn("AC-02 is not terminal", completed.stderr)

    def test_ready_pull_request_rejects_not_run_full_gate(self) -> None:
        contract = self.ready_contract()
        contract["validation"]["full"] = {
            "status": "notRun",
            "evidence": ["Full qualification was not executed."],
        }
        completed, _ = self.run_contract(contract, draft=False)
        self.assertNotEqual(completed.returncode, 0)
        self.assertIn("validation.full must pass", completed.stderr)
        self.assertIn("validation.full is not terminal", completed.stderr)

    def test_ready_pull_request_accepts_exact_head_evidence(self) -> None:
        completed, result = self.run_contract(self.ready_contract(), draft=False)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertTrue(result["valid"])

    @staticmethod
    def ready_contract() -> dict:
        contract = base_contract()
        contract["currentState"] = "readyForReview"
        contract["acceptance"]["AC-02"] = {
            "status": "notApplicable",
            "evidence": ["No Android product behavior changed."],
            "tests": [],
        }
        for gate in ["static", "fast", "full"]:
            contract["validation"][gate] = {
                "status": "passed",
                "sha": HEAD_SHA,
                "evidence": [f"workflow-{gate}"],
            }
        contract["manualAcceptance"]["review"] = {
            "status": "passed",
            "evidence": ["Inspected the rendered PR state and qualification record."],
        }
        return contract


if __name__ == "__main__":
    unittest.main()
