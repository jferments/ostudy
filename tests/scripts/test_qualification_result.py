import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "write-qualification-result.mjs"


class QualificationResultTests(unittest.TestCase):
    def run_result(
        self,
        log_text: str,
        *,
        status: str = "failure",
        sha: str = "1" * 40,
        step_name: str = "react-tests",
        step_status: str | None = None,
        gate: str = "fast",
    ) -> tuple[subprocess.CompletedProcess[str], dict]:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            log_path = directory / "check.log"
            output_path = directory / "qualification.json"
            log_path.write_text(log_text, encoding="utf-8")
            environment = os.environ.copy()
            environment.update(
                {
                    "QUALIFIED_SHA": sha,
                    "RUNNER_NAME": "test-runner",
                    "GITHUB_WORKFLOW": "Check",
                    "GITHUB_EVENT_NAME": "pull_request",
                    "GITHUB_RUN_ID": "123",
                    "GITHUB_RUN_ATTEMPT": "1",
                }
            )
            completed = subprocess.run(
                [
                    "node",
                    str(SCRIPT),
                    "--output",
                    str(output_path),
                    "--status",
                    status,
                    "--gate",
                    gate,
                    "--step",
                    f"{step_name}={step_status or ('failure' if status == 'failure' else 'success')}",
                    str(log_path),
                ],
                cwd=ROOT,
                env=environment,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertTrue(output_path.exists(), completed.stderr)
            return completed, json.loads(output_path.read_text(encoding="utf-8"))

    def test_success_record_is_exact_head_and_has_no_failure(self) -> None:
        completed, result = self.run_result("All ostudy checks passed.\n", status="success")
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(result["headSha"], "1" * 40)
        self.assertEqual(result["status"], "success")
        self.assertIsNone(result["failure"])
        self.assertEqual(result["steps"], [{"name": "react-tests", "status": "success"}])

    def test_vitest_failure_has_stable_fingerprint_and_focused_reproduction(self) -> None:
        first_log = """==> React tests
 FAIL  src/screens/DecksScreen.test.tsx
AssertionError: expected true to be false after 12 ms
FAST CHECK FAILED during: React tests (exit 1)
"""
        second_log = first_log.replace("12 ms", "987 ms")
        first_completed, first = self.run_result(first_log, sha="2" * 40)
        second_completed, second = self.run_result(second_log, sha="3" * 40)
        self.assertEqual(first_completed.returncode, 0, first_completed.stderr)
        self.assertEqual(second_completed.returncode, 0, second_completed.stderr)
        self.assertEqual(first["failure"]["fingerprint"], second["failure"]["fingerprint"])
        self.assertEqual(first["failure"]["boundary"], "React tests")
        self.assertIn("DecksScreen.test.tsx", first["failure"]["reproduction"])
        self.assertEqual(first["failure"]["classificationHint"], "productOrTest")

    def test_pr_contract_failure_has_contract_focused_reproduction(self) -> None:
        completed, result = self.run_result(
            "Pull-request execution contract verification failed:\n  - validation.full is pending\n",
            step_name="pr-contract",
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(result["failure"]["boundary"], "pr-contract")
        self.assertIn("verify-pr-contract.mjs", result["failure"]["reproduction"])
        self.assertIn("GITHUB_EVENT_PATH", result["failure"]["reproduction"])

    def test_unrecognized_failure_still_produces_a_bounded_record(self) -> None:
        completed, result = self.run_result("command exited unexpectedly\n")
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertRegex(result["failure"]["fingerprint"], r"^qf-[0-9a-f]{16}$")
        self.assertEqual(result["failure"]["reproduction"], "./scripts/check.sh")

    def test_linux_full_success_does_not_require_advisory_ui_performance(self) -> None:
        completed, result = self.run_result(
            "Linux qualification passed without UI performance evidence.\n",
            status="success",
            gate="linux-full",
            step_name="linux-full",
        )
        self.assertEqual(0, completed.returncode, completed.stderr)
        self.assertEqual("success", result["status"])
        self.assertIsNone(result["failure"])

    def test_all_full_success_keeps_failed_ui_performance_advisory(self) -> None:
        completed, result = self.run_result(
            "ERROR: Advisory UI performance candidate comparison failed.\n",
            status="success",
            gate="all-full",
            step_name="ui-performance",
            step_status="failure",
        )
        self.assertEqual(0, completed.returncode, completed.stderr)
        self.assertEqual("success", result["status"])
        self.assertIsNone(result["failure"])

    def test_success_status_cannot_hide_failed_android_correctness(self) -> None:
        completed, result = self.run_result(
            "ERROR: Android compile and package qualification failed.\n",
            status="success",
            gate="all-full",
            step_name="android-compile",
            step_status="failure",
        )
        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual("failure", result["status"])
        self.assertEqual("android-compile", result["failure"]["boundary"])
        self.assertIn("reported success", result["failure"]["summary"])
        self.assertIn("android-test.sh", result["failure"]["reproduction"])


if __name__ == "__main__":
    unittest.main()
