from __future__ import annotations

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class CiTmpdirTests(unittest.TestCase):
    def test_ci_tmpdir_is_unique_exported_and_cleaned(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            script = f"""
set -Eeuo pipefail
source scripts/lib/ci-test-tmpdir.bash
export CI=true
export OSTUDY_CI_TMPFS_ROOT={str(root)!r}
ostudy_prepare_ci_test_tmpdir fast
printf 'TMP=%s\\n' "$TMPDIR"
test -d "$TMPDIR"
touch "$TMPDIR/probe"
owned="$TMPDIR"
ostudy_cleanup_ci_test_tmpdir
test ! -e "$owned"
"""
            result = subprocess.run(
                ["bash", "-c", script],
                cwd=ROOT,
                text=True,
                capture_output=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(f"TMP={root}/ostudy-fast-", result.stdout)

    def test_local_execution_does_not_override_tmpdir(self) -> None:
        env = os.environ.copy()
        env["CI"] = "false"
        env["TMPDIR"] = "/tmp/ostudy-existing"
        result = subprocess.run(
            [
                "bash",
                "-c",
                "source scripts/lib/ci-test-tmpdir.bash; "
                "ostudy_prepare_ci_test_tmpdir fast; printf '%s' \"$TMPDIR\"",
            ],
            cwd=ROOT,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "/tmp/ostudy-existing")


if __name__ == "__main__":
    unittest.main()
