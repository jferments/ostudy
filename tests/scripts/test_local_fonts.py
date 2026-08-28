import subprocess
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
VERIFIER = ROOT / "scripts" / "verify-local-fonts.mjs"
LICENSE = ROOT / "public" / "licenses" / "IBM-Plex-Sans-OFL-1.1.txt"
FONT_FILENAMES = (
    "ibm-plex-sans-latin-variable.woff2",
    "albert-sans-latin-variable.woff2",
    "space-grotesk-latin-variable.woff2",
    "spline-sans-latin-variable.woff2",
    "sora-latin-variable.woff2",
    "roboto-latin-variable.woff2",
    "roboto-slab-latin-variable.woff2",
    "sofia-sans-latin-variable.woff2",
    "lato-latin-300.woff2",
    "lato-latin-400.woff2",
    "lato-latin-700.woff2",
    "ubuntu-latin-300.woff2",
    "ubuntu-latin-400.woff2",
    "ubuntu-latin-500.woff2",
    "ubuntu-latin-700.woff2",
)


class LocalFontVerifierTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="ostudy-font-package-")
        self.package = Path(self.temporary.name)
        self.assets = self.package / "assets"
        self.assets.mkdir(parents=True)
        packaged_names = []
        for filename in FONT_FILENAMES:
            stem = Path(filename).stem
            packaged_name = f"{stem}-fixtureHash.woff2"
            packaged_names.append(packaged_name)
            (self.assets / packaged_name).write_bytes(b"wOF2fixture")
        font_faces = "\n".join(
            f'@font-face{{font-family:"Fixture";src:url("./{name}") format("woff2")}}'
            for name in packaged_names
        )
        (self.assets / "app.css").write_text(
            font_faces
            + '\n:root{--ui-font-family: "IBM Plex Sans", var(--ostudy-font-family-system);'
            + '--card-font-family: "IBM Plex Sans", var(--ostudy-font-family-system)}',
            encoding="utf-8",
        )
        (self.package / "index.html").write_text(
            '<link rel="stylesheet" href="/assets/app.css">', encoding="utf-8"
        )
        license_directory = self.package / "licenses"
        license_directory.mkdir()
        (license_directory / LICENSE.name).write_text(
            LICENSE.read_text(encoding="utf-8"), encoding="utf-8"
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def verify(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["node", str(VERIFIER), "--packaged-dir", str(self.package)],
            cwd=ROOT,
            check=False,
            capture_output=True,
            text=True,
        )

    def test_accepts_hashed_offline_assets_and_matching_license(self) -> None:
        result = self.verify()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("hashed packaged WOFF2 assets and OFL notice", result.stdout)

    def test_rejects_a_missing_packaged_font(self) -> None:
        missing = self.assets / "ibm-plex-sans-latin-variable-fixtureHash.woff2"
        missing.unlink()

        result = self.verify()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Expected one hashed packaged asset", result.stderr)

    def test_rejects_a_missing_or_changed_packaged_license(self) -> None:
        packaged_license = self.package / "licenses" / LICENSE.name
        packaged_license.write_text("incomplete notice\n", encoding="utf-8")

        result = self.verify()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("does not match the audited source notice", result.stderr)

    def test_rejects_a_runtime_remote_font_reference(self) -> None:
        css = self.assets / "app.css"
        css.write_text(
            css.read_text(encoding="utf-8")
            + '\n@import url("https://fonts.googleapis.com/css2?family=IBM+Plex+Sans");',
            encoding="utf-8",
        )

        result = self.verify()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("packaged runtime remote-font reference", result.stderr)


if __name__ == "__main__":
    unittest.main()
