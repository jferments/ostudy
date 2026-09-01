import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WRAPPER = ROOT / "scripts" / "check-ui-performance-candidate.sh"


class UiPerformanceCandidateWrapperTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.scripts = self.root / "scripts"
        self.scripts.mkdir()
        shutil.copy2(WRAPPER, self.scripts / WRAPPER.name)
        (self.scripts / "ui-performance-baselines.sh").write_text(
            """#!/usr/bin/env bash
set -eu
printf '%s %s\n' "$1" "$2" >> "$CALL_LOG"
if [[ "$1" == "capture-candidate" ]]; then
  if (( ${CAPTURE_STATUS:-0} != 0 )); then
    exit "$CAPTURE_STATUS"
  fi
  mkdir -p "$2/measurements"
  printf '{"kind":"manifest"}\n' > "$2/manifest.json"
  printf '{"kind":"measurement"}\n' > "$2/measurements/deck-drag-v1.json"
  exit 0
fi
if [[ "$1" == "compare-candidate" ]]; then
  test -f "$2/manifest.json"
  test -f "$2/measurements/deck-drag-v1.json"
  exit "${COMPARE_STATUS:-0}"
fi
exit 64
""",
            encoding="utf-8",
        )
        self.call_log = self.root / "calls.log"

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_wrapper(
        self,
        output: Path | str,
        *,
        capture_status: int = 0,
        compare_status: int = 0,
    ) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment.update({
            "CALL_LOG": str(self.call_log),
            "CAPTURE_STATUS": str(capture_status),
            "COMPARE_STATUS": str(compare_status),
        })
        return subprocess.run(
            ["bash", str(self.scripts / WRAPPER.name), str(output)],
            cwd=self.root,
            env=environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )

    def calls(self) -> list[str]:
        if not self.call_log.exists():
            return []
        return self.call_log.read_text(encoding="utf-8").splitlines()

    def test_requires_one_explicit_absolute_new_output_directory(self) -> None:
        completed = self.run_wrapper("relative-output")
        self.assertEqual(2, completed.returncode)
        self.assertIn("explicit absolute path", completed.stderr)
        self.assertEqual([], self.calls())

        completed = subprocess.run(
            ["bash", str(self.scripts / WRAPPER.name)],
            cwd=self.root,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
        self.assertEqual(2, completed.returncode)
        self.assertIn("Usage:", completed.stderr)

    def test_runs_capture_then_compare_and_retains_canonical_evidence(self) -> None:
        output = self.root / "candidate"
        completed = self.run_wrapper(output)
        self.assertEqual(0, completed.returncode, completed.stderr)
        self.assertEqual(
            [f"capture-candidate {output}", f"compare-candidate {output}"],
            self.calls(),
        )
        self.assertTrue((output / "manifest.json").is_file())
        self.assertTrue((output / "measurements" / "deck-drag-v1.json").is_file())
        self.assertIn("evidence retained", completed.stdout)

    def test_capture_failure_propagates_without_running_compare(self) -> None:
        output = self.root / "candidate"
        completed = self.run_wrapper(output, capture_status=23)
        self.assertEqual(23, completed.returncode)
        self.assertEqual([f"capture-candidate {output}"], self.calls())
        self.assertFalse(output.exists())

    def test_compare_failure_propagates_and_keeps_installed_evidence(self) -> None:
        output = self.root / "candidate"
        completed = self.run_wrapper(output, compare_status=29)
        self.assertEqual(29, completed.returncode)
        self.assertEqual(
            [f"capture-candidate {output}", f"compare-candidate {output}"],
            self.calls(),
        )
        self.assertEqual(
            '{"kind":"manifest"}\n',
            (output / "manifest.json").read_text(encoding="utf-8"),
        )
        self.assertIn("evidence retained", completed.stderr)

    def test_existing_directory_or_symlink_is_never_overwritten(self) -> None:
        output = self.root / "candidate"
        output.mkdir()
        sentinel = output / "sentinel"
        sentinel.write_text("keep", encoding="utf-8")
        completed = self.run_wrapper(output)
        self.assertEqual(1, completed.returncode)
        self.assertEqual("keep", sentinel.read_text(encoding="utf-8"))
        self.assertEqual([], self.calls())

        alias = self.root / "candidate-alias"
        alias.symlink_to(output, target_is_directory=True)
        completed = self.run_wrapper(alias)
        self.assertEqual(1, completed.returncode)
        self.assertTrue(alias.is_symlink())
        self.assertEqual([], self.calls())


if __name__ == "__main__":
    unittest.main()

