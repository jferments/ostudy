#!/usr/bin/env python3
"""Bundled native libraries must land where the app looks for them."""

from __future__ import annotations

import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONFIGS = sorted((ROOT / "src-tauri").glob("tauri*.conf.json"))


class BundleResourceContractTests(unittest.TestCase):
    def test_single_file_resources_name_their_destination_file(self) -> None:
        # Tauri installs a single file mapped to "dir/" as a file named "dir",
        # so Windows found no native/pdfium/pdfium.dll and startup stopped.
        for config in CONFIGS:
            resources = json.loads(config.read_text()).get("bundle", {}).get("resources")
            if not isinstance(resources, dict):
                continue
            for source, dest in resources.items():
                if "*" in source or (ROOT / "src-tauri" / source).is_dir():
                    continue
                with self.subTest(config=config.name, source=source):
                    self.assertFalse(dest.endswith("/"), f"{source} -> {dest}")

    def test_windows_installs_pdfium_where_startup_loads_it(self) -> None:
        config = json.loads((ROOT / "src-tauri/tauri.windows.conf.json").read_text())
        self.assertIn("native/pdfium/pdfium.dll", config["bundle"]["resources"].values())


if __name__ == "__main__":
    unittest.main()
