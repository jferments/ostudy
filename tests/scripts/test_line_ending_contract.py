#!/usr/bin/env python3
"""Text files must check out with LF line endings on every platform."""

from __future__ import annotations

import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class LineEndingContractTests(unittest.TestCase):
    def test_text_files_check_out_with_lf(self) -> None:
        # Windows Git turns LF into CRLF unless the repository says otherwise,
        # and the bundled migrations' checksums then no longer match.
        migration = next((ROOT / "src-tauri/migrations").glob("*.sql"))
        path = migration.relative_to(ROOT).as_posix()
        attributes = subprocess.run(
            ["git", "check-attr", "text", "eol", "--", path],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        self.assertIn(f"{path}: text: auto", attributes)
        self.assertIn(f"{path}: eol: lf", attributes)


if __name__ == "__main__":
    unittest.main()
