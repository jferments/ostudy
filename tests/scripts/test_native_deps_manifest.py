import json
import re
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MANIFEST_PATH = ROOT / "scripts" / "native-deps-manifest.json"
SHA256 = re.compile(r"^[0-9a-f]{64}$")


class NativeDependencyManifestTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))

    def test_pdfium_release_and_desktop_artifacts_are_immutable_and_hashed(self):
        pdfium = self.manifest["pdfium"]
        self.assertEqual(self.manifest["schemaVersion"], 1)
        self.assertEqual(pdfium["apiBuild"], 7881)
        self.assertEqual(pdfium["bindingCrateVersion"], "0.9.3")
        self.assertEqual(pdfium["binarySource"]["release"], "chromium/7881")
        self.assertEqual(
            pdfium["binarySource"]["releaseBaseUrl"],
            "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F7881",
        )

        required = {
            "linux-x64": ("pdfium-linux-x64.tgz", "lib/libpdfium.so"),
            "linux-arm64": ("pdfium-linux-arm64.tgz", "lib/libpdfium.so"),
            "macos-x64": ("pdfium-mac-x64.tgz", "lib/libpdfium.dylib"),
            "macos-arm64": ("pdfium-mac-arm64.tgz", "lib/libpdfium.dylib"),
            "windows-x64": ("pdfium-win-x64.tgz", "bin/pdfium.dll"),
            "windows-arm64": ("pdfium-win-arm64.tgz", "bin/pdfium.dll"),
        }
        for target, (archive, library) in required.items():
            with self.subTest(target=target):
                artifact = pdfium["artifacts"][target]
                self.assertEqual(artifact["archive"], archive)
                self.assertEqual(artifact["library"], library)
                self.assertRegex(artifact["sha256"], SHA256)

    def test_every_pdfium_artifact_has_a_unique_archive_and_sha256(self):
        artifacts = self.manifest["pdfium"]["artifacts"]
        archives = [entry["archive"] for entry in artifacts.values()]
        hashes = [entry["sha256"] for entry in artifacts.values()]
        self.assertEqual(len(archives), len(set(archives)))
        self.assertEqual(len(hashes), len(set(hashes)))
        for digest in hashes:
            self.assertRegex(digest, SHA256)

    def test_tesseract_policy_uses_established_platform_install_channels(self):
        tesseract = self.manifest["tesseract"]
        self.assertGreaterEqual(tesseract["minimumMajor"], 5)
        self.assertEqual(tesseract["language"], "eng")
        self.assertIn("tesseract-ocr", tesseract["linuxDebianPackages"])
        self.assertEqual(tesseract["macosHomebrewFormula"], "tesseract")
        self.assertEqual(tesseract["windowsWingetId"], "UB-Mannheim.TesseractOCR")


if __name__ == "__main__":
    unittest.main()
