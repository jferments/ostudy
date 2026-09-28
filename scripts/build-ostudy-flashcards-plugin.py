#!/usr/bin/env python3
"""Build and validate the skills-only oStudy Flashcards submission ZIP.

The public ChatGPT-Web path is the OpenAI plugin submission portal. This builder
intentionally packages only the portable plugin root: plugin.json, assets/, and
skills/. Repository-local marketplace metadata and submission notes are not part
of the uploaded archive.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import stat
import struct
import sys
import unicodedata
import zipfile


ROOT = Path(__file__).resolve().parents[1]
PLUGIN = ROOT / "integrations" / "ostudy-flashcards"
MAX_ARCHIVE_BYTES = 100 * 1024 * 1024
MAX_ENTRY_BYTES = 100 * 1024 * 1024
MAX_UNCOMPRESSED_BYTES = 512 * 1024 * 1024
MAX_ENTRIES = 5_000
MAX_PATH_SEGMENTS = 20
VERSION = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.-]+)?$")


class PackageError(ValueError):
    pass


def fail(message: str) -> None:
    raise PackageError(message)


def load_manifest() -> dict:
    path = PLUGIN / "plugin.json"
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"plugin.json is not readable valid UTF-8 JSON: {error}")
    if not isinstance(value, dict):
        fail("plugin.json must contain a JSON object")
    if value.get("$schema") != "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json":
        fail("plugin.json must use the supported portable Agent Plugins schema")
    name = value.get("name")
    version = value.get("version")
    if not isinstance(name, str) or not name or len(name) > 64 or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]{0,63}", name):
        fail("plugin.json name must be 1–64 ASCII letters, digits, underscore, or hyphen and start alphanumeric")
    if not isinstance(version, str) or len(version) > 64 or not VERSION.fullmatch(version):
        fail("plugin.json version must be a semantic version of at most 64 characters")
    if "skills" in value or "mcpServers" in value:
        fail("portable root plugin.json must use fixed component discovery rather than legacy skills/mcpServers fields")

    extensions = value.get("extensions")
    if not isinstance(extensions, dict):
        fail("plugin.json must define extensions.com.openai.interface")
    openai = extensions.get("com.openai")
    if not isinstance(openai, dict):
        fail("plugin.json must define extensions.com.openai.interface")
    if "apps" in openai or "mcpServers" in openai:
        fail("skills-only web submission must not declare MCP/app configuration")
    interface = openai.get("interface")
    if not isinstance(interface, dict):
        fail("plugin.json must define extensions.com.openai.interface")
    if "screenshots" in interface:
        fail("skills-only submission must not declare interface.screenshots")
    for field in (
        "displayName",
        "shortDescription",
        "longDescription",
        "developerName",
        "websiteURL",
        "privacyPolicyURL",
        "termsOfServiceURL",
        "supportURL",
        "composerIcon",
        "logo",
    ):
        if not isinstance(interface.get(field), str) or not interface[field].strip():
            fail(f"plugin.json interface.{field} is required for the web publication package")
    if interface["logo"] != "./assets/logo.png":
        fail("plugin.json interface.logo must point to ./assets/logo.png")
    if interface["composerIcon"] != "./assets/logo.png":
        fail("plugin.json interface.composerIcon must point to ./assets/logo.png")
    if len(interface["displayName"]) > 30:
        fail("plugin.json interface.displayName must be 30 characters or fewer")
    if len(interface["longDescription"]) > 4000:
        fail("plugin.json interface.longDescription must be 4000 characters or fewer")
    if len(interface["developerName"]) > 80:
        fail("plugin.json interface.developerName must be 80 characters or fewer")
    author = value.get("author")
    if not isinstance(author, dict) or author.get("name") != interface["developerName"]:
        fail("plugin.json author.name must match interface.developerName")
    if interface.get("category") not in CATEGORIES:
        fail("plugin.json interface.category is not a supported Plugins Directory category")
    capabilities = interface.get("capabilities", [])
    if not isinstance(capabilities, list) or len(capabilities) > 20:
        fail("plugin.json interface.capabilities must contain at most 20 entries")
    if any(not isinstance(item, str) or not item.strip() or len(item) > 120 or "\n" in item for item in capabilities):
        fail("plugin.json capabilities must be non-empty single-line strings of at most 120 characters")
    if len(interface["shortDescription"]) > 30:
        fail("plugin.json interface.shortDescription must be 30 characters or fewer")
    prompts = interface.get("defaultPrompt", [])
    if not isinstance(prompts, list) or len(prompts) > 3:
        fail("plugin.json interface.defaultPrompt must contain at most three prompts")
    if any(not isinstance(prompt, str) or not prompt.strip() or len(prompt) > 128 or "\n" in prompt for prompt in prompts):
        fail("plugin.json starter prompts must be non-empty single-line strings of at most 128 characters")
    prompt_keys = {
        unicodedata.normalize("NFKC", " ".join(prompt.split())).casefold()
        for prompt in prompts
    }
    if len(prompt_keys) != len(prompts):
        fail("plugin.json starter prompts must remain unique after Unicode and whitespace normalization")
    for field in ("websiteURL", "privacyPolicyURL", "termsOfServiceURL", "supportURL"):
        if not interface[field].startswith("https://") or len(interface[field]) > 1024:
            fail(f"plugin.json interface.{field} must use HTTPS and be at most 1024 characters")
    validate_branding_png(PLUGIN / "assets" / "logo.png", "plugin branding image")
    return value


def archive_collision_key(path: str) -> str:
    """Match portal-style case/Unicode path normalization for collision checks."""
    return unicodedata.normalize("NFC", path).casefold()


def validate_branding_png(path: Path, label: str) -> None:
    """Validate the portal's packaged-branding constraints for our PNG asset."""
    try:
        data = path.read_bytes()
    except OSError as error:
        fail(f"{label} could not be read: {error}")
    if len(data) > 5 * 1024 * 1024:
        fail(f"{label} must not exceed 5 MiB")
    if len(data) < 24 or data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
        fail(f"{label} must be a decodable PNG with an IHDR header")
    width, height = struct.unpack(">II", data[16:24])
    if width != height:
        fail(f"{label} must be square")
    if width < 48 or width > 4096:
        fail(f"{label} dimensions must be between 48x48 and 4096x4096 pixels")


def collect_package_files() -> list[tuple[Path, str]]:
    for forbidden in ("mcp.json", ".mcp.json", ".app.json"):
        if (PLUGIN / forbidden).exists():
            fail(f"skills-only submission must not contain {forbidden}")

    skill_roots = sorted(path for path in (PLUGIN / "skills").iterdir() if path.is_dir())
    if not skill_roots or not any((path / "SKILL.md").is_file() for path in skill_roots):
        fail("submission must contain at least one skills/<skill-name>/SKILL.md")

    candidates = [PLUGIN / "plugin.json"]
    for root_name in ("assets", "skills"):
        root = PLUGIN / root_name
        if root.exists():
            candidates.extend(sorted(path for path in root.rglob("*") if path.is_file()))

    files: list[tuple[Path, str]] = []
    total = 0
    seen: set[str] = set()
    for path in candidates:
        if path.is_symlink():
            fail(f"submission cannot contain symbolic links: {path.relative_to(PLUGIN)}")
        relative = path.relative_to(PLUGIN).as_posix()
        posix = PurePosixPath(relative)
        if posix.is_absolute() or ".." in posix.parts or "" in posix.parts:
            fail(f"unsafe archive path: {relative}")
        if "\\" in relative:
            fail(f"archive paths must use forward slashes: {relative}")
        if len(posix.parts) > MAX_PATH_SEGMENTS:
            fail(f"archive path exceeds {MAX_PATH_SEGMENTS} segments: {relative}")
        normalized = archive_collision_key(relative)
        if normalized in seen:
            fail(f"archive path collides after case normalization: {relative}")
        seen.add(normalized)
        size = path.stat().st_size
        if size > MAX_ENTRY_BYTES:
            fail(f"archive entry exceeds 100 MiB: {relative}")
        total += size
        files.append((path, relative))

    if not files:
        fail("submission archive would be empty")
    if len(files) > MAX_ENTRIES:
        fail(f"submission archive has more than {MAX_ENTRIES} entries")
    if total > MAX_UNCOMPRESSED_BYTES:
        fail("submission archive exceeds the 512 MiB uncompressed limit")
    return files


def write_archive(output: Path, files: list[tuple[Path, str]]) -> str:
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(f".{output.name}.tmp")
    try:
        with zipfile.ZipFile(temporary, "w", compression=zipfile.ZIP_STORED) as archive:
            for source, relative in files:
                info = zipfile.ZipInfo(relative, date_time=(1980, 1, 1, 0, 0, 0))
                info.create_system = 3
                info.external_attr = (stat.S_IFREG | 0o644) << 16
                info.compress_type = zipfile.ZIP_STORED
                archive.writestr(info, source.read_bytes())
        if temporary.stat().st_size > MAX_ARCHIVE_BYTES:
            fail("submission ZIP exceeds the 100 MB compressed limit")
        validate_archive(temporary, {relative for _, relative in files})
        temporary.replace(output)
    finally:
        if temporary.exists():
            temporary.unlink()
    return hashlib.sha256(output.read_bytes()).hexdigest()


def validate_archive(path: Path, expected: set[str]) -> None:
    with zipfile.ZipFile(path, "r") as archive:
        infos = archive.infolist()
        names = [info.filename for info in infos]
        if len(names) != len(set(names)):
            fail("submission ZIP contains duplicate paths")
        if set(names) != expected:
            missing = sorted(expected - set(names))
            unexpected = sorted(set(names) - expected)
            fail(f"submission ZIP file set mismatch; missing={missing}, unexpected={unexpected}")
        if "plugin.json" not in names:
            fail("submission ZIP must contain plugin.json at the archive root")
        if not any(name.startswith("skills/") and name.endswith("/SKILL.md") for name in names):
            fail("submission ZIP must contain at least one skill")
        for name in names:
            if name.startswith("/") or "\\" in name or ".." in PurePosixPath(name).parts:
                fail(f"submission ZIP contains unsafe path: {name}")
            if len(PurePosixPath(name).parts) > MAX_PATH_SEGMENTS:
                fail(f"submission ZIP path is too deep: {name}")
        if any(name in {"mcp.json", ".mcp.json", ".app.json"} for name in names):
            fail("skills-only submission ZIP must not contain MCP/app configuration")
        if any(name.startswith(".agents/") or name.startswith("submission/") for name in names):
            fail("submission ZIP contains repository-only metadata")


def main() -> int:
    parser = argparse.ArgumentParser(description="Build the oStudy Flashcards skills-only web submission ZIP.")
    parser.add_argument("--output", type=Path, help="Destination ZIP path. Defaults to dist/<name>-<version>.zip.")
    parser.add_argument("--check-only", action="store_true", help="Validate package inputs without writing a ZIP.")
    args = parser.parse_args()

    try:
        manifest = load_manifest()
        files = collect_package_files()
        if args.check_only:
            print(f"Valid skills-only submission inputs: {len(files)} files for {manifest['name']} {manifest['version']}")
            return 0
        output = args.output or ROOT / "dist" / f"{manifest['name']}-{manifest['version']}.zip"
        digest = write_archive(output, files)
        print(f"Wrote {output} ({len(files)} files, sha256 {digest})")
        return 0
    except (PackageError, OSError, zipfile.BadZipFile) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
