import hashlib
import struct
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MASCOT = ROOT / "assets" / "images" / "racoon mascot (logo).png"
BRAND = ROOT / "src" / "components" / "Brand.tsx"
EXPECTED_SHA256 = "64979ff390c2a13a19e8f346c0f610b74fe83e14faf12497e902ac8a76f35137"
EXPECTED_IMPORT = 'import mascotUrl from "../../assets/images/racoon mascot (logo).png";'
REMOVED_MOTTO = "Learn with intent"


class BrandAssetContractTest(unittest.TestCase):
    def test_supplied_mascot_bytes_and_rgba_dimensions_are_exact(self) -> None:
        image = MASCOT.read_bytes()

        self.assertEqual(hashlib.sha256(image).hexdigest(), EXPECTED_SHA256)
        self.assertEqual(image[:8], b"\x89PNG\r\n\x1a\n")
        self.assertEqual(image[12:16], b"IHDR")
        self.assertEqual(struct.unpack(">I", image[8:12])[0], 13)

        width, height, bit_depth, color_type, compression, filtering, interlace = (
            struct.unpack(">IIBBBBB", image[16:29])
        )
        self.assertEqual((width, height), (1254, 1254))
        self.assertEqual(bit_depth, 8)
        self.assertEqual(color_type, 6, "The supplied mascot must remain an RGBA PNG")
        self.assertEqual((compression, filtering, interlace), (0, 0, 0))

    def test_brand_imports_the_exact_supplied_mascot(self) -> None:
        source = BRAND.read_text(encoding="utf-8")
        png_imports = [
            line for line in source.splitlines() if line.startswith("import ") and ".png" in line
        ]

        self.assertEqual(png_imports, [EXPECTED_IMPORT])
        self.assertEqual(source.count(EXPECTED_IMPORT), 1)
        self.assertIn("src={mascotUrl}", source)

    def test_removed_motto_stays_out_of_production_frontend_sources(self) -> None:
        offenders = []
        for path in (ROOT / "src").rglob("*"):
            if path.suffix not in {".css", ".ts", ".tsx"}:
                continue
            if ".test." in path.name or ".spec." in path.name:
                continue
            if REMOVED_MOTTO in path.read_text(encoding="utf-8"):
                offenders.append(path.relative_to(ROOT).as_posix())

        self.assertEqual(offenders, [])


if __name__ == "__main__":
    unittest.main()
