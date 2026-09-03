#!/usr/bin/env python3
"""Cross-platform application-icon contract tests."""

from __future__ import annotations

import json
import struct
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TAURI = ROOT / "src-tauri"
ANDROID_RES = TAURI / "gen/android/app/src/main/res"
ANDROID_NS = "{http://schemas.android.com/apk/res/android}"


def png_dimensions(path: Path) -> tuple[int, int]:
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
        raise AssertionError(f"{path} is not a PNG with an IHDR header")
    return struct.unpack(">II", data[16:24])


class ApplicationIconContractTests(unittest.TestCase):
    def test_desktop_uses_the_embedded_raccoon_icon_and_stable_gtk_identity(self) -> None:
        startup = (TAURI / "src/startup.rs").read_text(encoding="utf-8")
        config = json.loads((TAURI / "tauri.conf.json").read_text(encoding="utf-8"))
        icon = TAURI / "icons/128x128.png"

        self.assertEqual(png_dimensions(icon), (128, 128))
        self.assertIn('tauri::include_image!("./icons/128x128.png")', startup)
        self.assertIn("window.set_icon(MAIN_WINDOW_ICON.clone())", startup)
        self.assertNotIn("default_window_icon()", startup)
        self.assertTrue(config["app"]["enableGTKAppId"])
        self.assertEqual(config["identifier"], "io.github.jferments.ostudy")
        self.assertIn("icons/icon.png", config["bundle"]["icon"])

    def test_android_manifest_and_adaptive_icons_use_launcher_resources(self) -> None:
        manifest = ET.parse(
            TAURI / "gen/android/app/src/main/AndroidManifest.xml"
        ).getroot()
        application = manifest.find("application")
        self.assertIsNotNone(application)
        assert application is not None
        self.assertEqual(application.attrib[f"{ANDROID_NS}icon"], "@mipmap/ic_launcher")
        self.assertEqual(
            application.attrib[f"{ANDROID_NS}roundIcon"],
            "@mipmap/ic_launcher_round",
        )

        for name in ("ic_launcher.xml", "ic_launcher_round.xml"):
            root = ET.parse(ANDROID_RES / "mipmap-anydpi-v26" / name).getroot()
            self.assertEqual(root.tag, "adaptive-icon")
            background = root.find("background")
            foreground = root.find("foreground")
            self.assertIsNotNone(background)
            self.assertIsNotNone(foreground)
            assert background is not None and foreground is not None
            self.assertEqual(
                background.attrib[f"{ANDROID_NS}drawable"],
                "@color/ic_launcher_background",
            )
            self.assertEqual(
                foreground.attrib[f"{ANDROID_NS}drawable"],
                "@mipmap/ic_launcher_foreground",
            )

        foreground_xml = (
            ANDROID_RES / "drawable-v24/ic_launcher_foreground.xml"
        ).read_text(encoding="utf-8")
        self.assertIn("@mipmap/ic_launcher_foreground", foreground_xml)
        self.assertNotIn("android:pathData", foreground_xml)

    def test_every_android_launcher_bitmap_is_derived_from_the_raccoon_assets(self) -> None:
        raccoon_128 = (TAURI / "icons/128x128.png").read_bytes()
        raccoon_256 = (TAURI / "icons/128x128@2x.png").read_bytes()

        for density in ("mdpi", "hdpi", "xhdpi"):
            directory = ANDROID_RES / f"mipmap-{density}"
            self.assertEqual((directory / "ic_launcher.png").read_bytes(), raccoon_128)
            self.assertEqual((directory / "ic_launcher_round.png").read_bytes(), raccoon_128)
            self.assertEqual(
                (directory / "ic_launcher_foreground.png").read_bytes(),
                raccoon_256,
            )

        for density in ("xxhdpi", "xxxhdpi"):
            directory = ANDROID_RES / f"mipmap-{density}"
            for name in (
                "ic_launcher.png",
                "ic_launcher_round.png",
                "ic_launcher_foreground.png",
            ):
                self.assertEqual((directory / name).read_bytes(), raccoon_256)


if __name__ == "__main__":
    unittest.main()