"""Exercise test selection and failure propagation without a Rust toolchain."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/test-native-contracts.sh"
FAKE_CARGO = r'''#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys
with Path(os.environ["FAKE_CALLS"]).open("a") as output:
    output.write(json.dumps(sys.argv[1:]) + "\n")
mode = os.environ.get("FAKE_MODE", "pass")
listing = "--list" in sys.argv
if listing:
    if mode == "list_failure":
        sys.exit(7)
    if mode != "empty":
        print("module::one: test\nmodule::two: test")
    print("0 benchmarks")
else:
    if mode == "test_failure":
        print("test result: FAILED. 1 passed; 1 failed; 0 ignored;")
        sys.exit(9)
    elif mode == "ignored":
        print("test result: ok. 1 passed; 0 failed; 1 ignored;")
    elif mode == "partial":
        print("test result: ok. 1 passed; 0 failed; 0 ignored;")
    else:
        print("test result: ok. 2 passed; 0 failed; 0 ignored;")
'''


class NativeContractSelectionTests(unittest.TestCase):
    def invoke(self, mode="pass", filters=("database::entry_tests::",)):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cargo = root / "cargo"
            cargo.write_text(FAKE_CARGO)
            cargo.chmod(0o700)
            calls = root / "calls.jsonl"
            environment = dict(os.environ, PATH=f"{root}:{os.environ['PATH']}",
                               FAKE_CALLS=str(calls), FAKE_MODE=mode)
            result = subprocess.run(["bash", str(SCRIPT), *filters], env=environment,
                                    capture_output=True, text=True, check=False)
            recorded = [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else []
            return result, recorded

    def test_executes_every_selected_family_and_reports_nonzero_counts(self):
        result, calls = self.invoke(filters=("database::entry_tests::", "current_format_tests::"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(calls), 4)
        self.assertEqual(result.stdout.count("2 executed, none ignored"), 2)
        self.assertTrue(all("--locked" in call and "--all-features" in call and "--lib" in call for call in calls))
        self.assertIn("--list", calls[0])
        self.assertNotIn("--list", calls[1])

    def test_empty_selection_fails_before_execution(self):
        result, calls = self.invoke("empty")
        self.assertEqual(result.returncode, 1)
        self.assertEqual(len(calls), 1)
        self.assertIn("matched no tests", result.stderr)

    def test_listing_failure_preserves_cargo_failure(self):
        result, calls = self.invoke("list_failure")
        self.assertEqual(result.returncode, 7)
        self.assertEqual(len(calls), 1)

    def test_test_failure_is_not_hidden_by_tee(self):
        result, calls = self.invoke("test_failure")
        self.assertEqual(result.returncode, 9)
        self.assertEqual(len(calls), 2)

    def test_ignored_or_partial_execution_is_not_a_pass(self):
        for mode in ("ignored", "partial"):
            with self.subTest(mode=mode):
                result, _ = self.invoke(mode)
                self.assertEqual(result.returncode, 1)
                self.assertIn("did not execute every selected test", result.stderr)

    def test_missing_or_option_shaped_filter_is_rejected(self):
        for filters in ((), ("",), ("--ignored",)):
            with self.subTest(filters=filters):
                result, calls = self.invoke(filters=filters)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
