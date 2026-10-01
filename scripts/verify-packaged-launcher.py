#!/usr/bin/env python3
"""Read-only verification of the installed GUI entrypoint and built binary bytes."""

from __future__ import annotations

import argparse
import configparser
import hashlib
import mmap
import shlex
import stat
import sys
import tomllib
from pathlib import Path


def regular_file(path: Path, *, root: Path | None = None, executable: bool = False) -> None:
    if root is not None and not path.resolve().is_relative_to(root.resolve()):
        raise ValueError("Packaged launcher path escapes the extracted package.")
    mode = path.lstat().st_mode
    if not stat.S_ISREG(mode) or (executable and mode & 0o111 == 0):
        raise ValueError(f"Expected a regular{' executable' if executable else ''} file: {path}")


def digest(path: Path) -> bytes:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").digest()


def expected_debian_digest(path: Path) -> bytes:
    """Hash the exact first-marker patch made by the pinned Tauri bundler.

    tauri-cli 2.11.4, commit 8909f221d1515955fc843808032bdc5d62209c96,
    crates/tauri-bundler/src/bundle.rs: patch_binary replaces the first UNK
    token with DEB, then bundle_project restores the original build file.
    Do not normalize arbitrary markers in the installed file or ignore bytes.
    """
    original = b"__TAURI_BUNDLE_TYPE_VAR_UNK"
    packaged = b"__TAURI_BUNDLE_TYPE_VAR_DEB"
    if path.stat().st_size == 0:
        raise ValueError("Built GUI is missing the unpatched bundle-type marker.")
    with path.open("rb") as stream:
        with mmap.mmap(stream.fileno(), 0, access=mmap.ACCESS_READ) as binary:
            offset = binary.find(original)
            if offset < 0:
                raise ValueError("Built GUI is missing the unpatched bundle-type marker.")
            result = hashlib.sha256()
            # Views avoid copying the potentially large executable into Python buffers.
            with memoryview(binary) as view:
                result.update(view[:offset])
                result.update(packaged)
                result.update(view[offset + len(original):])
            return result.digest()


def verify_manifest(manifest: Path) -> None:
    with manifest.open("rb") as stream:
        package = tomllib.load(stream).get("package", {})
    if package.get("name") != "ostudy" or package.get("default-run") != "ostudy":
        raise ValueError("Cargo must select ostudy explicitly as the default GUI binary.")


def verify_launcher(root: Path, built_gui: Path) -> None:
    installed = root / "usr/bin/ostudy"
    regular_file(installed, root=root, executable=True)
    regular_file(built_gui, executable=True)
    if digest(installed) != expected_debian_digest(built_gui):
        raise ValueError("Packaged ostudy bytes differ from the built GUI with its exact Debian marker patch.")

    desktop = root / "usr/share/applications/ostudy.desktop"
    regular_file(desktop, root=root)
    config = configparser.ConfigParser(interpolation=None, strict=True)
    config.optionxform = str
    with desktop.open(encoding="utf-8") as stream:
        config.read_file(stream)
    entry = config["Desktop Entry"]
    command = shlex.split(entry.get("Exec", ""))
    if entry.get("Type") != "Application" or not command:
        raise ValueError("The package is missing its application desktop entry.")
    if command[0] not in {"ostudy", "/usr/bin/ostudy"}:
        raise ValueError("The desktop entry selects a helper instead of the oStudy GUI.")
    if entry.get("TryExec", "ostudy") not in {"ostudy", "/usr/bin/ostudy"}:
        raise ValueError("The desktop entry TryExec selects a different executable.")
    if entry.get("Terminal", "false").lower() != "false":
        raise ValueError("The oStudy GUI must not be packaged as a terminal launcher.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package_root", type=Path)
    parser.add_argument("built_gui", type=Path)
    parser.add_argument("cargo_manifest", type=Path)
    args = parser.parse_args()
    try:
        verify_manifest(args.cargo_manifest)
        verify_launcher(args.package_root, args.built_gui)
    except (OSError, ValueError, KeyError, configparser.Error) as error:
        print(f"ERROR: GUI package identity: {error}", file=sys.stderr)
        return 1
    print("OK   Packaged GUI bytes match the exact Tauri Debian marker patch; desktop entry selects ostudy.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
