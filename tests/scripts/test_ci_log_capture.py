import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "run-ci-captured.sh"


class CiLogCaptureTests(unittest.TestCase):
    def run_capture(
        self,
        command: str,
        *,
        environment_updates: dict[str, str] | None = None,
        expect_log: bool = True,
    ) -> tuple[subprocess.CompletedProcess[str], str]:
        with tempfile.TemporaryDirectory() as temporary:
            log_path = Path(temporary) / "qualification.log"
            environment = os.environ.copy()
            environment.update(
                {
                    "OSTUDY_CI_SUCCESS_TAIL_BYTES": "256",
                    "OSTUDY_CI_FAILURE_TAIL_BYTES": "384",
                }
            )
            environment.update(environment_updates or {})
            completed = subprocess.run(
                ["bash", str(SCRIPT), str(log_path), "bash", "-c", command],
                cwd=ROOT,
                env=environment,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
                timeout=10,
            )
            self.assertEqual(log_path.exists(), expect_log)
            raw_log = log_path.read_text(encoding="utf-8") if log_path.exists() else ""
            return completed, raw_log

    def test_success_retains_full_log_but_bounds_live_output(self) -> None:
        completed, raw_log = self.run_capture(
            "printf 'EARLY_SUCCESS_MARKER\\n'; "
            "for i in $(seq 1 200); do printf 'success-line-%03d-xxxxxxxxxxxxxxxxxxxxxxxx\\n' \"$i\"; done; "
            "printf 'FINAL_SUCCESS_MARKER\\n'"
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("EARLY_SUCCESS_MARKER", raw_log)
        self.assertIn("FINAL_SUCCESS_MARKER", raw_log)
        self.assertNotIn("EARLY_SUCCESS_MARKER", completed.stdout)
        self.assertIn("FINAL_SUCCESS_MARKER", completed.stdout)
        self.assertLess(len(completed.stdout.encode("utf-8")), 1024)

    def test_long_running_command_emits_metadata_only_heartbeats(self) -> None:
        completed, raw_log = self.run_capture(
            "printf 'EARLY_HEARTBEAT_MARKER\\n'; "
            "for i in $(seq 1 200); do printf 'heartbeat-line-%03d-zzzzzzzzzzzzzzzzzzzzzzzz\\n' \"$i\"; done; "
            "sleep 2; printf 'FINAL_HEARTBEAT_MARKER\\n'",
            environment_updates={"OSTUDY_CI_HEARTBEAT_SECONDS": "1"},
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn("EARLY_HEARTBEAT_MARKER", raw_log)
        self.assertIn("FINAL_HEARTBEAT_MARKER", raw_log)
        self.assertNotIn("EARLY_HEARTBEAT_MARKER", completed.stdout)
        self.assertIn("WAIT Qualification still running", completed.stdout)
        self.assertIn("phase=qualification", completed.stdout)
        self.assertIn("log_bytes=", completed.stdout)
        self.assertIn("delta_bytes=", completed.stdout)
        self.assertIn("FINAL_HEARTBEAT_MARKER", completed.stdout)
        self.assertLess(len(completed.stdout.encode("utf-8")), 1536)

    def test_invalid_heartbeat_interval_fails_closed(self) -> None:
        completed, raw_log = self.run_capture(
            "exit 0",
            environment_updates={"OSTUDY_CI_HEARTBEAT_SECONDS": "0"},
            expect_log=False,
        )
        self.assertEqual(raw_log, "")
        self.assertEqual(completed.returncode, 2)
        self.assertIn("OSTUDY_CI_HEARTBEAT_SECONDS", completed.stderr)

    def test_failure_preserves_exit_code_and_emits_only_bounded_tail(self) -> None:
        completed, raw_log = self.run_capture(
            "printf 'EARLY_FAILURE_MARKER\\n'; "
            "for i in $(seq 1 250); do printf 'failure-line-%03d-yyyyyyyyyyyyyyyyyyyyyyyy\\n' \"$i\"; done; "
            "printf 'FINAL_FAILURE_MARKER\\n' >&2; exit 7"
        )
        live_output = completed.stdout + completed.stderr
        self.assertEqual(completed.returncode, 7)
        self.assertIn("EARLY_FAILURE_MARKER", raw_log)
        self.assertIn("FINAL_FAILURE_MARKER", raw_log)
        self.assertNotIn("EARLY_FAILURE_MARKER", live_output)
        self.assertIn("FINAL_FAILURE_MARKER", live_output)
        self.assertIn("full output retained", live_output)
        self.assertLess(len(live_output.encode("utf-8")), 1536)


if __name__ == "__main__":
    unittest.main()
