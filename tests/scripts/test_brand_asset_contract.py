import hashlib
import re
import struct
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MASCOT = ROOT / "assets" / "images" / "racoon mascot (logo).png"
BRAND = ROOT / "src" / "components" / "Brand.tsx"
ARTWORK = ROOT / "src" / "ui" / "artwork"
MANIFEST = ARTWORK / "artworkManifest.ts"
HEADS = {64: ARTWORK / "images" / "raccoon-head-64.webp", 128: ARTWORK / "images" / "raccoon-head-128.webp"}
EXPECTED_SHA256 = "64979ff390c2a13a19e8f346c0f610b74fe83e14faf12497e902ac8a76f35137"
REMOVED_MOTTO = "Learn with intent"
IMAGE_IMPORT = re.compile(r"""from\s+["'][^"']+\.(?:png|jpe?g|webp|svg)["']""", re.IGNORECASE)


def webp_canvas(data: bytes) -> tuple[int, int, bool]:
    """Return the width, height and alpha flag of an extended (VP8X) WebP file."""
    if data[:4] != b"RIFF" or data[8:12] != b"WEBP":
        raise AssertionError("not a RIFF WebP file")
    if data[12:16] != b"VP8X":
        raise AssertionError("expected an extended WebP with an alpha channel")
    flags = data[20]
    width = int.from_bytes(data[24:27], "little") + 1
    height = int.from_bytes(data[27:30], "little") + 1
    return width, height, bool(flags & 0x10)


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

    def test_brand_mark_ships_the_head_crop_as_small_transparent_webp(self) -> None:
        for size, path in HEADS.items():
            with self.subTest(size=size):
                data = path.read_bytes()
                self.assertEqual(webp_canvas(data), (size, size, True))
                self.assertLess(len(data), 16 * 1024)

    def test_brand_mark_comes_from_the_artwork_registry(self) -> None:
        manifest = MANIFEST.read_text(encoding="utf-8")
        brand = BRAND.read_text(encoding="utf-8")

        for path in HEADS.values():
            self.assertIn(f'from "./images/{path.name}";', manifest)
        self.assertIsNone(IMAGE_IMPORT.search(brand), "Brand.tsx must read its image from the registry")
        self.assertIn('artworkManifest["brand.logo.compact"].image', brand)
        self.assertIn("src={raccoonHead?.src}", brand)
        self.assertIn("srcSet={raccoonHead?.srcSet}", brand)

    def test_full_resolution_mascot_stays_out_of_the_frontend_bundle(self) -> None:
        offenders = []
        for path in (ROOT / "src").rglob("*"):
            if path.suffix in {".css", ".ts", ".tsx"} and MASCOT.name in path.read_text(encoding="utf-8"):
                offenders.append(path.relative_to(ROOT).as_posix())

        self.assertEqual(offenders, [])

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
