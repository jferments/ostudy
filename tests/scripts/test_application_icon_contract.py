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
    def test_linux_creates_the_window_with_the_actual_mascot_before_mapping(self) -> None:
        source = ROOT / "assets/images/racoon mascot (logo).png"
        desktop_icon = TAURI / "icons/icon.png"
        startup = (TAURI / "src/startup.rs").read_text(encoding="utf-8")
        config = json.loads((TAURI / "tauri.conf.json").read_text(encoding="utf-8"))
        linux_config = json.loads(
            (TAURI / "tauri.linux.conf.json").read_text(encoding="utf-8")
        )

        width, height = png_dimensions(source)
        self.assertEqual(width, height)
        self.assertGreaterEqual(width, 512)
        self.assertEqual(desktop_icon.read_bytes(), source.read_bytes())
        self.assertIn('tauri::include_image!("./icons/icon.png")', startup)
        self.assertIn("create_linux_main_window(app)?", startup)
        self.assertIn("WebviewWindowBuilder::from_config", startup)
        self.assertIn(".icon(MAIN_WINDOW_ICON.clone())?", startup)
        self.assertIn("window.set_icon(MAIN_WINDOW_ICON.clone())", startup)
        self.assertIn("main_window_icon_applied", startup)
        self.assertTrue(config["app"]["enableGTKAppId"])
        self.assertEqual(config["identifier"], "io.github.jferments.ostudy")
        self.assertEqual(config["bundle"]["icon"][0], "icons/icon.png")

        windows = linux_config["app"]["windows"]
        self.assertEqual(len(windows), 1)
        self.assertEqual(windows[0]["label"], "main")
        self.assertFalse(windows[0]["create"])
        self.assertEqual(windows[0]["url"], "index.html")

    def test_android_launcher_uses_the_same_actual_mascot(self) -> None:
        source = ROOT / "assets/images/racoon mascot (logo).png"
        android_source = ANDROID_RES / "drawable-nodpi/ostudy_raccoon_mascot.png"
        foreground = ANDROID_RES / "drawable/ostudy_raccoon_foreground.xml"

        self.assertEqual(android_source.read_bytes(), source.read_bytes())
        foreground_text = foreground.read_text(encoding="utf-8")
        self.assertIn("@drawable/ostudy_raccoon_mascot", foreground_text)
        self.assertIn('android:gravity="fill"', foreground_text)

    def test_manifest_and_every_launcher_variant_resolve_to_mascot_resources(self) -> None:
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
            legacy = (ANDROID_RES / "mipmap-anydpi" / name).read_text(encoding="utf-8")
            self.assertIn("@drawable/ostudy_raccoon_mascot", legacy)

            adaptive = ET.parse(ANDROID_RES / "mipmap-anydpi-v26" / name).getroot()
            self.assertEqual(adaptive.tag, "adaptive-icon")
            background = adaptive.find("background")
            foreground = adaptive.find("foreground")
            self.assertIsNotNone(background)
            self.assertIsNotNone(foreground)
            assert background is not None and foreground is not None
            self.assertEqual(
                background.attrib[f"{ANDROID_NS}drawable"],
                "@color/ic_launcher_background",
            )
            self.assertEqual(
                foreground.attrib[f"{ANDROID_NS}drawable"],
                "@drawable/ostudy_raccoon_foreground",
            )

            themed = ET.parse(ANDROID_RES / "mipmap-anydpi-v33" / name).getroot()
            monochrome = themed.find("monochrome")
            self.assertIsNotNone(monochrome)
            assert monochrome is not None
            self.assertEqual(
                monochrome.attrib[f"{ANDROID_NS}drawable"],
                "@drawable/ostudy_raccoon_foreground",
            )

    def test_stale_generic_android_launcher_assets_are_absent(self) -> None:
        self.assertFalse(
            (ANDROID_RES / "drawable-v24/ic_launcher_foreground.xml").exists()
        )
        for density in ("mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi"):
            directory = ANDROID_RES / f"mipmap-{density}"
            for name in (
                "ic_launcher.png",
                "ic_launcher_round.png",
                "ic_launcher_foreground.png",
            ):
                self.assertFalse((directory / name).exists(), f"stale asset: {density}/{name}")


if __name__ == "__main__":
    unittest.main()