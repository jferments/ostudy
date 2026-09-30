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
        self.installed.write_bytes(b"synthetic-built-GUI")
        self.installed.chmod(0o755)
        self.built = self.root / "ostudy"
        self.built.write_bytes(self.installed.read_bytes())
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
        LAUNCHER.verify_launcher(self.package, self.built)
        self.write_desktop("/usr/bin/ostudy %U")
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
