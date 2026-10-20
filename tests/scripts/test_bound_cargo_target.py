from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "runner" / "bound-cargo-target.sh"


class BoundCargoTargetTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.target = Path(self.temporary.name) / "cargo-target"
        (self.target / "debug" / "incremental" / "ostudy-abc").mkdir(parents=True)
        (self.target / "debug" / "incremental" / "ostudy-abc" / "query.bin").write_bytes(b"x")
        (self.target / "x86_64-linux-android" / "debug" / "incremental").mkdir(parents=True)
        (self.target / "debug" / "deps").mkdir(parents=True)
        (self.target / "debug" / "deps" / "libostudy.rlib").write_bytes(b"x")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_script(self, **settings: str) -> subprocess.CompletedProcess[str]:
        env = {
            **os.environ,
            "OSTUDY_CARGO_TARGET_DIR": str(self.target),
            "OSTUDY_CARGO_TARGET_MAX_GIB": "100000",
            "OSTUDY_CARGO_TARGET_MIN_FREE_GIB": "0",
            **settings,
        }
        return subprocess.run(
            ["bash", str(SCRIPT)], env=env, text=True, capture_output=True, check=False
        )

    def test_removes_incremental_state_and_keeps_build_artifacts_within_bounds(self) -> None:
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("removed 2 incremental directories", result.stdout)
        self.assertFalse((self.target / "debug" / "incremental").exists())
        self.assertFalse((self.target / "x86_64-linux-android" / "debug" / "incremental").exists())
        self.assertTrue((self.target / "debug" / "deps" / "libostudy.rlib").exists())
        self.assertNotIn("clearing", result.stdout)

    def test_clears_a_target_above_its_size_cap(self) -> None:
        result = self.run_script(OSTUDY_CARGO_TARGET_MAX_GIB="0")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("above the 0 GiB cap", result.stdout)
        self.assertTrue(self.target.is_dir())
        self.assertEqual(list(self.target.iterdir()), [])

    def test_clears_the_target_when_the_disk_lacks_the_build_margin(self) -> None:
        result = self.run_script(OSTUDY_CARGO_TARGET_MIN_FREE_GIB="1000000")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("below the 1000000 GiB build margin", result.stdout)
        self.assertEqual(list(self.target.iterdir()), [])

    def test_refuses_a_directory_that_is_not_a_dedicated_cargo_target(self) -> None:
        other = Path(self.temporary.name) / "home"
        other.mkdir()
        (other / "keep").write_text("keep")
        for path in (str(other), "relative/cargo-target", ""):
            result = self.run_script(OSTUDY_CARGO_TARGET_DIR=path, OSTUDY_CARGO_TARGET_MAX_GIB="0")
            self.assertEqual(result.returncode, 2, path)
            self.assertIn("refusing", result.stderr)
        self.assertTrue((other / "keep").exists())

    def test_rejects_malformed_limits(self) -> None:
        result = self.run_script(OSTUDY_CARGO_TARGET_MAX_GIB="forty")
        self.assertEqual(result.returncode, 2)
        self.assertTrue((self.target / "debug" / "deps" / "libostudy.rlib").exists())


if __name__ == "__main__":
    unittest.main()
