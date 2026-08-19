from __future__ import annotations

from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]
ANDROID_BUILD = ROOT / "scripts" / "android-build.sh"


class AndroidBuildHygieneTests(unittest.TestCase):
    def test_tauri_generated_tracked_sources_are_restored_after_build(self) -> None:
        source = ANDROID_BUILD.read_text(encoding="utf-8")
        self.assertIn(
            "src-tauri/gen/android/app/src/main/AndroidManifest.xml",
            source,
        )
        self.assertIn(
            "src-tauri/gen/android/buildSrc/src/main/java/io/github/jferments/ostudy/kotlin/BuildTask.kt",
            source,
        )
        self.assertIn("generated_sources=(", source)
        self.assertIn("restore_generated_sources", source)
        self.assertIn("trap restore_generated_sources EXIT", source)
        self.assertIn('cp -- "$generated_backup_root/$relative" "$ROOT_DIR/$relative"', source)


if __name__ == "__main__":
    unittest.main()
