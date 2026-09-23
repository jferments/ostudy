"""Regression tests for zero-placeholder local crash diagnostics."""

import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "ostudy_crash_diagnostics", ROOT / "scripts/diagnose-last-crash.py"
)
CRASH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CRASH)


class CrashDiagnosticsTests(unittest.TestCase):
    def test_latest_run_pairs_launcher_start_and_exit(self):
        records = [
            {"event": "process_start", "timestamp": "2026-01-01T00:00:00Z", "pid": 7},
            {"event": "process_exit", "timestamp": "2026-01-01T00:01:00Z", "exit_code": 0},
            {"event": "process_start", "timestamp": "2026-01-01T00:02:00Z", "pid": 8},
            {"stream": "stderr", "event": "process_output"},
            {"event": "process_exit", "timestamp": "2026-01-01T00:03:00Z", "exit_code": -11},
        ]
        latest = CRASH.latest_process(records)
        self.assertEqual(latest["start"]["pid"], 8)
        self.assertEqual(latest["exit"]["exit_code"], -11)

    def test_legacy_negative_exit_code_recovers_signal_name(self):
        details = CRASH.signal_details({"exit_code": -6})
        self.assertEqual(details["signalNumber"], 6)
        self.assertEqual(details["signalName"], "SIGABRT")


if __name__ == "__main__":
    unittest.main()
