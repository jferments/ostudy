from __future__ import annotations

import importlib.util
import json
import shutil
import struct
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
WEBDRIVER_DIR = ROOT / "tests" / "webdriver"
VERIFIER_PATH = WEBDRIVER_DIR / "ui_visual_baseline.py"
BASELINE_DIR = ROOT / "tests" / "visual-baselines" / "ui-workflow-redesign-stage-0"


def load_verifier_module():
    sys.path.insert(0, str(WEBDRIVER_DIR))
    try:
        spec = importlib.util.spec_from_file_location("ostudy_ui_visual_baseline", VERIFIER_PATH)
        if spec is None or spec.loader is None:
            raise RuntimeError("Could not load UI visual-baseline verifier")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module
    finally:
        sys.path.remove(str(WEBDRIVER_DIR))


class UiVisualBaselineVerifierTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.verifier = load_verifier_module()

    def copy_baseline(self, root: Path) -> Path:
        destination = root / "baseline"
        shutil.copytree(BASELINE_DIR, destination)
        return destination

    def load_manifest(self, baseline: Path) -> dict:
        return json.loads((baseline / "manifest.json").read_text(encoding="utf-8"))

    def write_manifest(self, baseline: Path, manifest: dict) -> None:
        (baseline / "manifest.json").write_text(
            json.dumps(manifest, indent=2) + "\n",
            encoding="utf-8",
        )

    def assert_invalid(self, baseline: Path, pattern: str) -> None:
        with self.assertRaisesRegex(self.verifier.BaselineValidationError, pattern):
            self.verifier.verify_manifest(baseline)

    def test_committed_matrix_passes_read_only_verification(self) -> None:
        before = (BASELINE_DIR / "manifest.json").read_bytes()
        manifest = self.verifier.verify_manifest(BASELINE_DIR)
        after = (BASELINE_DIR / "manifest.json").read_bytes()

        self.assertEqual(len(manifest["captures"]), 12)
        self.assertEqual(before, after)

    def test_rejects_missing_capture_entry(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            baseline = self.copy_baseline(Path(temporary))
            manifest = self.load_manifest(baseline)
            manifest["captures"].pop()
            self.write_manifest(baseline, manifest)
            self.assert_invalid(baseline, "exactly 12 captures")

    def test_rejects_missing_png(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            baseline = self.copy_baseline(Path(temporary))
            manifest = self.load_manifest(baseline)
            (baseline / manifest["captures"][0]["path"]).unlink()
            self.assert_invalid(baseline, "Missing baseline PNG")

    def test_rejects_duplicate_json_key(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            baseline = self.copy_baseline(Path(temporary))
            manifest_path = baseline / "manifest.json"
            source = manifest_path.read_text(encoding="utf-8")
            source = source.replace(
                '  "schemaVersion": 1,',
                '  "schemaVersion": 1,\n  "schemaVersion": 1,',
                1,
            )
            manifest_path.write_text(source, encoding="utf-8")
            self.assert_invalid(baseline, "duplicate JSON key")

    def test_rejects_duplicate_matrix_coordinate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            baseline = self.copy_baseline(Path(temporary))
            manifest = self.load_manifest(baseline)
            manifest["captures"][1]["theme"] = manifest["captures"][0]["theme"]
            manifest["captures"][1]["layoutClass"] = manifest["captures"][0]["layoutClass"]
            self.write_manifest(baseline, manifest)
            self.assert_invalid(baseline, "Duplicate matrix coordinate")

    def test_rejects_duplicate_image_hash(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            baseline = self.copy_baseline(Path(temporary))
            manifest = self.load_manifest(baseline)
            manifest["captures"][1]["sha256"] = manifest["captures"][0]["sha256"]
            self.write_manifest(baseline, manifest)
            self.assert_invalid(baseline, "Duplicate capture image hash")

    def test_rejects_noncanonical_or_traversing_name(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            baseline = self.copy_baseline(Path(temporary))
            manifest = self.load_manifest(baseline)
            manifest["captures"][0]["path"] = "../wrong.png"
            self.write_manifest(baseline, manifest)
            self.assert_invalid(baseline, "path is not canonical")

    def test_rejects_wrong_theme_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            baseline = self.copy_baseline(Path(temporary))
            manifest = self.load_manifest(baseline)
            manifest["captures"][0]["resolvedTheme"] = "dark"
            self.write_manifest(baseline, manifest)
            self.assert_invalid(baseline, "wrong theme metadata")

    def test_rejects_wrong_layout_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            baseline = self.copy_baseline(Path(temporary))
            manifest = self.load_manifest(baseline)
            manifest["captures"][0]["layoutClass"] = "medium"
            self.write_manifest(baseline, manifest)
            self.assert_invalid(baseline, "wrong layout class")

    def test_rejects_stale_source_or_scenario(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            baseline = self.copy_baseline(Path(temporary))
            manifest = self.load_manifest(baseline)
            manifest["baselineSourceSha"] = "0" * 40
            self.write_manifest(baseline, manifest)
            self.assert_invalid(baseline, "Stale baseline source SHA")

            manifest["baselineSourceSha"] = self.verifier.BASELINE_SOURCE_SHA
            manifest["scenario"]["version"] = "flashcards-library-v0"
            self.write_manifest(baseline, manifest)
            self.assert_invalid(baseline, "Stale or unexpected scenario fixture")

    def test_rejects_stale_hash_or_png_dimensions(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            baseline = self.copy_baseline(Path(temporary))
            manifest = self.load_manifest(baseline)
            first = manifest["captures"][0]
            image_path = baseline / first["path"]
            image = bytearray(image_path.read_bytes())
            image[-1] ^= 1
            image_path.write_bytes(image)
            self.assert_invalid(baseline, "SHA-256 mismatch")

            shutil.copyfile(BASELINE_DIR / first["path"], image_path)
            image = bytearray(image_path.read_bytes())
            image[16:24] = struct.pack(">II", 1, 1)
            image_path.write_bytes(image)
            self.assert_invalid(baseline, "PNG dimensions")

    def test_rejects_unexpected_baseline_file(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            baseline = self.copy_baseline(Path(temporary))
            (baseline / "unexpected.png").write_bytes(b"unexpected")
            self.assert_invalid(baseline, "unexpected=.*unexpected.png")


if __name__ == "__main__":
    unittest.main()

