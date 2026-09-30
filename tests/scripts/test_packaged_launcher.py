"""The metadata exporter must never masquerade as the production GUI launcher."""

from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "packaged_launcher", ROOT / "scripts/verify-packaged-launcher.py"
)
assert SPEC is not None and SPEC.loader is not None
LAUNCHER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LAUNCHER)


class PackagedLauncherTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="ostudy-launcher-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.package = self.root / "package"
        self.installed = self.package / "usr/bin/ostudy"
        self.installed.parent.mkdir(parents=True)
        self.installed.write_bytes(b"GUI-prefix\0__TAURI_BUNDLE_TYPE_VAR_DEB\0GUI-suffix")
        self.installed.chmod(0o755)
        self.built = self.root / "ostudy"
        self.built.write_bytes(b"GUI-prefix\0__TAURI_BUNDLE_TYPE_VAR_UNK\0GUI-suffix")
        self.built.chmod(0o755)
        self.desktop = self.package / "usr/share/applications/ostudy.desktop"
        self.desktop.parent.mkdir(parents=True)
        self.write_desktop("ostudy %U")

    def write_desktop(self, command: str) -> None:
        self.desktop.write_text(
            f"[Desktop Entry]\nType=Application\nName=ostudy\nExec={command}\nTerminal=false\n",
            encoding="utf-8",
        )

    def test_exact_gui_and_desktop_entry_are_accepted(self) -> None:
        original = self.built.read_bytes()
        packaged = self.installed.read_bytes()
        LAUNCHER.verify_launcher(self.package, self.built)
        self.assertEqual(self.built.read_bytes(), original)
        self.assertEqual(self.installed.read_bytes(), packaged)
        self.write_desktop("/usr/bin/ostudy %U")
        LAUNCHER.verify_launcher(self.package, self.built)

    def test_unpatched_package_is_rejected_even_when_identical_to_build(self) -> None:
        self.installed.write_bytes(self.built.read_bytes())
        with self.assertRaisesRegex(ValueError, "bytes differ"):
            LAUNCHER.verify_launcher(self.package, self.built)

    def test_other_bundle_type_is_not_accepted_as_debian(self) -> None:
        self.installed.write_bytes(self.installed.read_bytes().replace(b"VAR_DEB", b"VAR_APP"))
        with self.assertRaisesRegex(ValueError, "bytes differ"):
            LAUNCHER.verify_launcher(self.package, self.built)

    def test_every_other_byte_stays_part_of_the_comparison(self) -> None:
        valid = self.installed.read_bytes()
        for offset in [0, len(valid) - 1]:
            with self.subTest(offset=offset):
                changed = bytearray(valid)
                changed[offset] ^= 1
                self.installed.write_bytes(changed)
                with self.assertRaisesRegex(ValueError, "bytes differ"):
                    LAUNCHER.verify_launcher(self.package, self.built)

    def test_truncated_and_appended_packages_are_rejected(self) -> None:
        valid = self.installed.read_bytes()
        for changed in [valid[:-1], valid + b"extra"]:
            with self.subTest(changed=changed):
                self.installed.write_bytes(changed)
                with self.assertRaisesRegex(ValueError, "bytes differ"):
                    LAUNCHER.verify_launcher(self.package, self.built)

    def test_only_first_unpatched_marker_is_transformed_like_tauri(self) -> None:
        token = b"__TAURI_BUNDLE_TYPE_VAR_UNK"
        deb = b"__TAURI_BUNDLE_TYPE_VAR_DEB"
        original = b"prefix" + token + b"between" + token + b"suffix"
        self.built.write_bytes(original)
        self.installed.write_bytes(original.replace(token, deb, 1))
        LAUNCHER.verify_launcher(self.package, self.built)
        for invalid in [original.replace(token, deb), original.rsplit(token, 1)[0] + deb + b"suffix"]:
            with self.subTest(invalid=invalid):
                self.installed.write_bytes(invalid)
                with self.assertRaisesRegex(ValueError, "bytes differ"):
                    LAUNCHER.verify_launcher(self.package, self.built)

    def test_missing_or_already_patched_build_marker_is_rejected(self) -> None:
        for original in [b"", b"helper without marker", b"__TAURI_BUNDLE_TYPE_VAR_DEB"]:
            with self.subTest(original=original):
                self.built.write_bytes(original)
                self.installed.write_bytes(original)
                with self.assertRaisesRegex(ValueError, "unpatched bundle-type marker"):
                    LAUNCHER.verify_launcher(self.package, self.built)

    def test_marker_at_file_boundaries_and_across_a_page_is_supported(self) -> None:
        token = b"__TAURI_BUNDLE_TYPE_VAR_UNK"
        deb = b"__TAURI_BUNDLE_TYPE_VAR_DEB"
        for prefix, suffix in [(b"", b"suffix"), (b"prefix", b""), (b"x" * 4090, b"tail")]:
            with self.subTest(prefix_length=len(prefix), suffix_length=len(suffix)):
                self.built.write_bytes(prefix + token + suffix)
                self.installed.write_bytes(prefix + deb + suffix)
                LAUNCHER.verify_launcher(self.package, self.built)

    def test_wrong_tryexec_and_terminal_launcher_are_rejected(self) -> None:
        self.desktop.write_text(
            "[Desktop Entry]\nType=Application\nExec=ostudy %U\nTryExec=helper\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ValueError, "TryExec"):
            LAUNCHER.verify_launcher(self.package, self.built)
        self.desktop.write_text(
            "[Desktop Entry]\nType=Application\nExec=ostudy %U\nTerminal=true\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(ValueError, "terminal"):
            LAUNCHER.verify_launcher(self.package, self.built)

    def test_helper_launcher_is_rejected_even_with_gui_present(self) -> None:
        self.write_desktop("export-migration-contract %U")
        with self.assertRaisesRegex(ValueError, "helper"):
            LAUNCHER.verify_launcher(self.package, self.built)

    def test_helper_bytes_renamed_to_ostudy_are_rejected(self) -> None:
        self.installed.write_bytes(b"synthetic-migration-exporter")
        with self.assertRaisesRegex(ValueError, "bytes differ"):
            LAUNCHER.verify_launcher(self.package, self.built)

    def test_missing_or_non_executable_gui_is_rejected(self) -> None:
        self.installed.chmod(0o644)
        with self.assertRaisesRegex(ValueError, "regular executable"):
            LAUNCHER.verify_launcher(self.package, self.built)
        self.installed.unlink()
        with self.assertRaises(OSError):
            LAUNCHER.verify_launcher(self.package, self.built)

    def test_symlink_and_escaping_package_paths_are_rejected(self) -> None:
        self.installed.unlink()
        self.installed.symlink_to(self.built)
        with self.assertRaisesRegex(ValueError, "escapes"):
            LAUNCHER.verify_launcher(self.package, self.built)

    def test_malformed_desktop_entry_is_rejected(self) -> None:
        self.write_desktop("'unterminated")
        with self.assertRaises(ValueError):
            LAUNCHER.verify_launcher(self.package, self.built)

    def test_manifest_requires_explicit_gui_not_implicit_or_helper_default(self) -> None:
        manifest = self.root / "Cargo.toml"
        for setting in ["", 'default-run = "export-migration-contract"\n']:
            manifest.write_text('[package]\nname = "ostudy"\n' + setting, encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "explicitly"):
                LAUNCHER.verify_manifest(manifest)
        manifest.write_text(
            '[package]\nname = "ostudy"\ndefault-run = "ostudy"\n', encoding="utf-8"
        )
        LAUNCHER.verify_manifest(manifest)

    def test_repository_manifest_selects_maintained_gui(self) -> None:
        LAUNCHER.verify_manifest(ROOT / "src-tauri/Cargo.toml")


if __name__ == "__main__":
    unittest.main()
