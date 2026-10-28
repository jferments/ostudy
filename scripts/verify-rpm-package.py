#!/usr/bin/env python3
"""Read-only inspection of the production oStudy .rpm, with the same policy as the .deb.

Reads the RPM headers and cpio payload directly, so the Ubuntu runner needs no
rpm tooling. Checks the package name, the libusb and ca-certificates requirements, the packaged GUI
bytes and desktop entry, the reviewed udev rule, the verified PDFium runtime,
and that neither files nor scriptlets touch user-owned learner data.
"""

from __future__ import annotations

import argparse
import gzip
import importlib.util
import lzma
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("packaged_launcher", ROOT / "scripts/verify-packaged-launcher.py")
assert SPEC is not None and SPEC.loader is not None
LAUNCHER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LAUNCHER)

HEADER_MAGIC = b"\x8e\xad\xe8\x01"
TAG_NAME = 1000
TAG_PREIN, TAG_POSTIN, TAG_PREUN, TAG_POSTUN = 1023, 1024, 1025, 1026
TAG_REQUIRENAME = 1049
TAG_DIRINDEXES, TAG_BASENAMES, TAG_DIRNAMES = 1116, 1117, 1118
TAG_PAYLOADFORMAT, TAG_PAYLOADCOMPRESSOR = 1124, 1125
LIBUSB_REQUIREMENT = re.compile(r"^(libusb-1\.0\.so\.0(\(\)\(64bit\))?|libusb1|libusb-1_0-0)$")
USER_PATH = re.compile(r"^/(home|root)/|\.local/share", re.IGNORECASE)
USER_DATA_SCRIPT = re.compile(r"XDG_DATA_HOME|\.local/share|/home/|\$HOME|\$\{HOME", re.IGNORECASE)


def read_header(data: bytes, offset: int) -> tuple[dict[int, object], int]:
    if data[offset:offset + 4] != HEADER_MAGIC:
        raise ValueError(f"RPM header magic missing at byte {offset}.")
    count, size = struct.unpack(">II", data[offset + 8:offset + 16])
    index = offset + 16
    store = index + 16 * count
    tags: dict[int, object] = {}
    for entry in range(count):
        tag, kind, start, items = struct.unpack(">IIII", data[index + 16 * entry:index + 16 * entry + 16])
        position = store + start
        if kind in (6, 8, 9):  # string, string array, i18n string
            values = []
            for _ in range(items if kind != 6 else 1):
                end = data.index(b"\0", position)
                values.append(data[position:end].decode("utf-8"))
                position = end + 1
            tags[tag] = values[0] if kind == 6 else values
        elif kind == 4:  # int32
            tags[tag] = list(struct.unpack(f">{items}I", data[position:position + 4 * items]))
    return tags, store + size


def decompress(payload: bytes, compressor: str) -> bytes:
    if compressor == "gzip":
        return gzip.decompress(payload)
    if compressor in ("xz", "lzma"):
        return lzma.decompress(payload)
    if compressor == "zstd":
        try:
            from compression import zstd  # Python 3.14+
            return zstd.decompress(payload)
        except ImportError:
            tool = shutil.which("zstd")
            if tool is None:
                raise ValueError("The .rpm payload is zstd-compressed; install zstd to inspect it.") from None
            return subprocess.run([tool, "-dc"], input=payload, capture_output=True, check=True).stdout
    raise ValueError(f"Unsupported RPM payload compressor: {compressor}")


def extract_cpio(archive: bytes, destination: Path) -> None:
    position = 0
    while True:
        header = archive[position:position + 110]
        if header[:6] not in (b"070701", b"070702"):
            raise ValueError("RPM payload is not a newc cpio archive.")
        fields = [int(header[6 + 8 * i:14 + 8 * i], 16) for i in range(13)]
        mode, file_size, name_size = fields[1], fields[6], fields[11]
        name_start = position + 110
        name = archive[name_start:name_start + name_size - 1].decode("utf-8")
        data_start = (name_start + name_size + 3) & ~3
        if name == "TRAILER!!!":
            return
        relative = Path(name.removeprefix("./").lstrip("/"))
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError(f"RPM payload path escapes the package root: {name}")
        target = destination / relative
        kind = mode & 0o170000
        if kind == 0o040000:
            target.mkdir(parents=True, exist_ok=True)
        elif kind == 0o100000:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(archive[data_start:data_start + file_size])
            target.chmod(mode & 0o7777)
        position = (data_start + file_size + 3) & ~3


def verify(rpm: Path, built_gui: Path, cargo_manifest: Path) -> None:
    data = rpm.read_bytes()
    if data[:4] != b"\xed\xab\xee\xdb":
        raise ValueError("Not an RPM package (lead magic missing).")
    _, end = read_header(data, 96)  # signature header
    end = (end + 7) & ~7
    tags, payload_start = read_header(data, end)

    if tags.get(TAG_NAME) != "ostudy":
        raise ValueError(f"Expected RPM package name ostudy, found {tags.get(TAG_NAME)}.")
    requires = list(tags.get(TAG_REQUIRENAME, []))
    if not any(LIBUSB_REQUIREMENT.match(name) for name in requires):
        raise ValueError(f"RPM package is missing the libusb requirement. Requires: {requires}")
    # Startup builds the AI HTTP client, which needs the system certificate store.
    if "ca-certificates" not in requires:
        raise ValueError(f"RPM package is missing the ca-certificates requirement. Requires: {requires}")

    dirnames = list(tags.get(TAG_DIRNAMES, []))
    files = [dirnames[d] + b for d, b in zip(tags.get(TAG_DIRINDEXES, []), tags.get(TAG_BASENAMES, []))]
    owned = [path for path in files if USER_PATH.search(path)]
    if owned:
        raise ValueError(f"RPM package owns user/home application-data paths: {owned}")
    for tag, label in ((TAG_PREIN, "%pre"), (TAG_POSTIN, "%post"), (TAG_PREUN, "%preun"), (TAG_POSTUN, "%postun")):
        script = tags.get(tag)
        if isinstance(script, str) and USER_DATA_SCRIPT.search(script):
            raise ValueError(f"RPM scriptlet {label} references user learner-data locations.")

    if tags.get(TAG_PAYLOADFORMAT, "cpio") != "cpio":
        raise ValueError(f"Unsupported RPM payload format: {tags.get(TAG_PAYLOADFORMAT)}")
    archive = decompress(data[payload_start:], str(tags.get(TAG_PAYLOADCOMPRESSOR, "gzip")))

    with tempfile.TemporaryDirectory(prefix="ostudy-rpm-policy.") as temporary:
        root = Path(temporary)
        extract_cpio(archive, root)
        LAUNCHER.verify_manifest(cargo_manifest)
        LAUNCHER.verify_launcher(root, built_gui, "rpm")
        for packaged, source, label in (
            ("usr/lib/udev/rules.d/70-ostudy-android-usb.rules",
             ROOT / "src-tauri/packaging/linux/70-ostudy-android-usb.rules", "reviewed Android USB udev rule"),
            ("usr/lib/ostudy/native/pdfium/libpdfium.so",
             ROOT / "src-tauri/resources/native/pdfium/libpdfium.so", "verified PDFium runtime"),
        ):
            installed = root / packaged
            if not installed.is_file():
                raise ValueError(f"RPM package is missing the {label} ({packaged}).")
            if installed.read_bytes() != source.read_bytes():
                raise ValueError(f"Packaged {label} does not match {source.relative_to(ROOT)}.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rpm", type=Path)
    parser.add_argument("built_gui", type=Path)
    parser.add_argument("--cargo-manifest", type=Path, default=ROOT / "src-tauri/Cargo.toml")
    args = parser.parse_args()
    try:
        verify(args.rpm, args.built_gui, args.cargo_manifest)
    except (OSError, ValueError, KeyError, struct.error, subprocess.CalledProcessError) as error:
        print(f"ERROR: RPM package policy: {error}", file=sys.stderr)
        return 1
    print("OK   RPM package contains the reviewed udev rule, verified PDFium runtime, libusb requirement, and no user-owned learner-data paths.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
