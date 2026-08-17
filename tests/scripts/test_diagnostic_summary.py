from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SUMMARY = ROOT / "scripts" / "summarize-check-failure.mjs"


class DiagnosticFailureSummaryTests(unittest.TestCase):
    def run_summary(
        self,
        gate_log: str,
        events: list[dict[str, object]],
        context: str,
        logcat_lines: list[str] | None = None,
    ) -> str:
        with tempfile.TemporaryDirectory() as raw_directory:
            directory = Path(raw_directory)
            log_path = directory / "gate.log"
            diagnostic_directory = directory / "diagnostics" / "failure"
            diagnostic_directory.mkdir(parents=True)
            log_path.write_text(gate_log, encoding="utf-8")
            (diagnostic_directory / "context.txt").write_text(context, encoding="utf-8")
            (diagnostic_directory / "application.jsonl").write_text(
                "\n".join(json.dumps(event) for event in events) + "\n",
                encoding="utf-8",
            )
            if logcat_lines is not None:
                (diagnostic_directory / "logcat.txt").write_text(
                    "\n".join(logcat_lines) + "\n",
                    encoding="utf-8",
                )
            environment = os.environ.copy()
            environment.update({"QUALIFIED_SHA": "abc123", "RUNNER_NAME": "test-runner"})
            completed = subprocess.run(
                [
                    "node",
                    str(SUMMARY),
                    "--diagnostics",
                    str(directory / "diagnostics"),
                    str(log_path),
                ],
                check=True,
                capture_output=True,
                text=True,
                env=environment,
            )
            return completed.stdout

    def test_vitest_failure_gets_file_scoped_replay_and_structured_error(self) -> None:
        output = self.run_summary(
            "\n".join(
                [
                    "FAIL  src/screens/SettingsScreen.test.tsx",
                    "AssertionError: expected true to be false",
                    "FULL CHECK FAILED during: React tests (exit 1)",
                ]
            ),
            [
                {
                    "application": "ostudy",
                    "timestampMs": 1,
                    "level": "info",
                    "target": "ostudy.app",
                    "event": "app_started",
                    "message": "Application started.",
                    "fields": {},
                },
                {
                    "application": "ostudy",
                    "timestampMs": 2,
                    "level": "error",
                    "target": "ostudy.frontend",
                    "event": "settings_render_failed",
                    "message": "Settings rendering failed.",
                    "fields": {"screen": "settings"},
                },
            ],
            "\n".join(
                [
                    "kind=failure",
                    "step=React tests",
                    "status=1",
                    "replayFilter=warn,ostudy.app=debug,ostudy.frontend=debug",
                    "replayCommand=pnpm test",
                ]
            ),
        )
        self.assertIn(
            "pnpm vitest run 'src/screens/SettingsScreen.test.tsx' --reporter=verbose",
            output,
        )
        self.assertIn("`ostudy.frontend` / `settings_render_failed`", output)
        self.assertNotIn("`ostudy.app` / `app_started`", output)

    def test_rust_failure_gets_exact_test_replay(self) -> None:
        output = self.run_summary(
            "\n".join(
                [
                    "test database::tests::public_baseline_is_idempotent_and_matches_manifest ... FAILED",
                    "test result: FAILED. 0 passed; 1 failed",
                    "FULL CHECK FAILED during: Rust unit, integration, and migration-fixture tests (exit 101)",
                ]
            ),
            [],
            "\n".join(
                [
                    "kind=failure",
                    "step=Rust unit, integration, and migration-fixture tests",
                    "status=101",
                    "replayFilter=warn,ostudy.database=debug",
                ]
            ),
        )
        self.assertIn(
            "'database::tests::public_baseline_is_idempotent_and_matches_manifest' -- --exact --nocapture",
            output,
        )
        self.assertIn("OSTUDY_LOG='warn,ostudy.database=debug'", output)

    def test_android_logcat_embedded_json_is_parsed(self) -> None:
        event = {
            "schemaVersion": 1,
            "application": "ostudy",
            "timestampMs": 5,
            "level": "error",
            "target": "ostudy.qr",
            "event": "scanner_unavailable",
            "message": "The QR scanner is unavailable.",
            "fields": {"reasonCode": "service_missing"},
        }
        output = self.run_summary(
            "Android smoke case failed: AndroidPlatformRoutingTest#route (instrumentation exit 1)",
            [],
            "kind=failure\nstep=Android smoke route\nstatus=1",
            [f"08-17 12:00:00.000 I/stdout: {json.dumps(event)}"],
        )
        self.assertIn("`ostudy.qr` / `scanner_unavailable`", output)
        self.assertIn("./scripts/android-emulator-smoke.sh", output)


if __name__ == "__main__":
    unittest.main()
