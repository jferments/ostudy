from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]
INSTALL = ROOT / "scripts" / "android-install.sh"


class AndroidInstallWrapperTests(unittest.TestCase):
    def test_physical_device_install_uses_non_streaming_adb(self) -> None:
        source = INSTALL.read_text(encoding="utf-8")

        self.assertIn('adb "${adb_args[@]}" install --no-streaming -r "$apk"', source)
        self.assertNotIn('adb "${adb_args[@]}" install -r "$apk"', source)
        self.assertIn('--serial <adb-serial>', source)
        self.assertIn('verify-android-package.sh "$apk" "$MODE"', source)


if __name__ == "__main__":
    unittest.main()
