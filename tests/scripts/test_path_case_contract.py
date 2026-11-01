#!/usr/bin/env python3
"""Tracked paths and modules must stay distinct on case-insensitive file systems."""

from __future__ import annotations

import subprocess
import unittest
from collections import defaultdict
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[2]
# An import names a module without its extension, so `./Foo` finds `foo.ts`
# as readily as `Foo.tsx` where case is ignored.
MODULE_SUFFIXES = {".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"}


def tracked_paths() -> list[str]:
    listed = subprocess.run(
        ["git", "ls-files", "-z"],
        cwd=ROOT,
        check=True,
        capture_output=True,
    ).stdout.decode("utf-8")
    return [path for path in listed.split("\0") if path and (ROOT / path).exists()]


def clashes(keys: dict[str, list[str]]) -> list[list[str]]:
    return sorted(sorted(set(names)) for names in keys.values() if len(set(names)) > 1)


class PathCaseContractTests(unittest.TestCase):
    def test_no_two_tracked_paths_differ_only_in_case(self) -> None:
        # Windows and macOS checkouts keep only one of such a pair.
        groups: dict[str, list[str]] = defaultdict(list)
        for path in tracked_paths():
            groups[path.casefold()].append(path)
        self.assertEqual(clashes(groups), [])

    def test_no_two_modules_differ_only_in_case(self) -> None:
        groups: dict[str, list[str]] = defaultdict(list)
        for path in tracked_paths():
            pure = PurePosixPath(path)
            if pure.suffix in MODULE_SUFFIXES:
                stem = str(pure.with_suffix(""))
                groups[stem.casefold()].append(stem)
        self.assertEqual(clashes(groups), [])

if __name__ == "__main__":
    unittest.main()
