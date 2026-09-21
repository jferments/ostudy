from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / "scripts" / "test-native-contracts.sh"


class NativeContractHelperTests(unittest.TestCase):
    def run_helper(self, *filters: str) -> tuple[subprocess.CompletedProcess[str], list[str]]:
        with tempfile.TemporaryDirectory() as temp:
            temp_path = Path(temp)
            fake_bin = temp_path / "bin"
            fake_bin.mkdir()
            log = temp_path / "cargo.log"
            cargo = fake_bin / "cargo"
            cargo.write_text(
                """#!/usr/bin/env bash
set -eu
printf '%s\\n' "$*" >> "$CARGO_LOG"
case "$*" in
  *missing::*)
    printf 'running 0 tests\\n\\n'
    printf 'test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 999 filtered out; finished in 0.00s\\n'
    ;;
  *ignored::*)
    printf 'running 1 test\\n'
    printf 'test ignored::case ... ignored\\n\\n'
    printf 'test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 998 filtered out; finished in 0.00s\\n'
    ;;
  *)
    printf 'running 1 test\\n'
    printf 'test selected::case ... ok\\n\\n'
    printf 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 998 filtered out; finished in 0.00s\\n'
    ;;
esac
"""
            )
            cargo.chmod(0o755)
            env = os.environ.copy()
            env["PATH"] = f"{fake_bin}{os.pathsep}{env['PATH']}"
            env["CARGO_LOG"] = str(log)
            result = subprocess.run(
                [str(HELPER), *filters],
                cwd=ROOT,
                env=env,
                text=True,
                capture_output=True,
                check=False,
            )
            calls = log.read_text().splitlines() if log.exists() else []
            return result, calls

    def test_each_filter_invokes_cargo_once(self) -> None:
        result, calls = self.run_helper("alpha::", "beta::")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(calls), 2)
        self.assertTrue(all("-- --color never" in call for call in calls))

    def test_zero_executed_tests_are_rejected_without_a_list_pass(self) -> None:
        result, calls = self.run_helper("missing::")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(calls), 1)
        self.assertIn("matched no executed tests", result.stderr)

    def test_ignored_only_selection_is_rejected(self) -> None:
        result, calls = self.run_helper("ignored::")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(calls), 1)
        self.assertIn("did not execute every selected test", result.stderr)


if __name__ == "__main__":
    unittest.main()
