"""Manifest and comparison-page generation for UI screenshot review.

This module has no WebDriver dependency so the manifest contract and the
generated comparison page can be unit tested without a built application.
"""

from __future__ import annotations

import hashlib
import html
import json
import re
import shutil
import subprocess
from collections.abc import Callable, Iterable, Sequence
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

MANIFEST_NAME = "manifest.json"
MANIFEST_SCHEMA_VERSION = 1
TOOL_ID = "ostudy-ui-screenshots"
DEFAULT_MAX_FILES = 300
DEFAULT_MAX_BYTES = 40 * 1024 * 1024
CAPTURE_STATUSES = ("ok", "empty", "failed")
_NAME_PATTERN = re.compile(r"^[a-z0-9][a-z0-9-]*$")


class ManifestError(ValueError):
    """A screenshot manifest is missing, malformed, or inconsistent."""


def capture_filename(screen: str, theme: str, width: int, extension: str = "webp") -> str:
    for label, value in (("screen", screen), ("theme", theme)):
        if not _NAME_PATTERN.match(value):
            raise ManifestError(f"Invalid {label} identifier: {value!r}")
    return f"{screen}--{theme}--{int(width)}.{extension}"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def new_manifest(
    *,
    source: dict[str, Any],
    screens: Sequence[dict[str, Any]],
    themes: Sequence[str],
    viewports: Sequence[dict[str, Any]],
    captured_at: str,
    capture_date: str,
    environment: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """Create an empty manifest for a capture run."""
    return {
        "schemaVersion": MANIFEST_SCHEMA_VERSION,
        "tool": TOOL_ID,
        "source": dict(source),
        "capturedAt": captured_at,
        "captureDate": capture_date,
        "environment": dict(environment or {}),
        "matrix": {"themes": list(themes), "viewports": [dict(item) for item in viewports]},
        "screens": [dict(item) for item in screens],
        "captures": [],
        "notes": [],
        "warnings": [],
        "timings": {},
    }


def add_capture(
    manifest: dict[str, Any],
    *,
    screen: str,
    theme: str,
    width: int,
    height: int,
    file: str | None,
    status: str = "ok",
    sha256: str | None = None,
    pixel_sha256: str | None = None,
    byte_size: int | None = None,
    seconds: float | None = None,
    note: str | None = None,
) -> dict[str, Any]:
    if status not in CAPTURE_STATUSES:
        raise ManifestError(f"Unknown capture status: {status!r}")
    entry: dict[str, Any] = {
        "screen": screen,
        "theme": theme,
        "width": int(width),
        "height": int(height),
        "file": file,
        "status": status,
        "sha256": sha256,
        "pixelSha256": pixel_sha256,
        "bytes": byte_size,
        "seconds": None if seconds is None else round(float(seconds), 2),
        "sourceSha": manifest["source"].get("sha"),
    }
    if note:
        entry["note"] = note
    manifest["captures"].append(entry)
    return entry


def write_manifest(directory: Path, manifest: dict[str, Any]) -> Path:
    path = directory / MANIFEST_NAME
    path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return path


def load_manifest(directory: Path) -> dict[str, Any]:
    path = directory / MANIFEST_NAME
    if not path.is_file():
        raise ManifestError(f"No {MANIFEST_NAME} in {directory}")
    try:
        manifest = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ManifestError(f"{path} is not valid JSON: {error}") from error
    validate_manifest(manifest, directory)
    return manifest


def validate_manifest(manifest: Any, directory: Path | None = None) -> None:
    if not isinstance(manifest, dict):
        raise ManifestError("A manifest must be a JSON object.")
    if manifest.get("tool") != TOOL_ID:
        raise ManifestError(f"Not a {TOOL_ID} manifest.")
    if manifest.get("schemaVersion") != MANIFEST_SCHEMA_VERSION:
        raise ManifestError(f"Unsupported manifest schema version: {manifest.get('schemaVersion')!r}")
    for key in ("source", "matrix", "screens", "captures"):
        if key not in manifest:
            raise ManifestError(f"Manifest is missing {key!r}.")
    seen: set[tuple[str, str, int]] = set()
    for capture in manifest["captures"]:
        for key in ("screen", "theme", "width", "height", "status", "file"):
            if key not in capture:
                raise ManifestError(f"Capture entry is missing {key!r}: {capture!r}")
        identity = (capture["screen"], capture["theme"], int(capture["width"]))
        if identity in seen:
            raise ManifestError(f"Duplicate capture for {identity}.")
        seen.add(identity)
        if capture["status"] not in CAPTURE_STATUSES:
            raise ManifestError(f"Unknown capture status in {identity}: {capture['status']!r}")
        if capture["status"] != "failed":
            if not capture["file"]:
                raise ManifestError(f"Capture {identity} has no file.")
            if Path(str(capture["file"])).name != capture["file"]:
                raise ManifestError(f"Capture file must be a plain name: {capture['file']!r}")
            if directory is not None and not (directory / capture["file"]).is_file():
                raise ManifestError(f"Capture file is missing: {directory / capture['file']}")


# --------------------------------------------------------------------------
# Comparison
# --------------------------------------------------------------------------

STATE_CHANGED = "changed"
STATE_UNCHANGED = "unchanged"
STATE_ADDED = "added"
STATE_REMOVED = "removed"
STATE_FAILED = "failed"


@dataclass
class ComparisonItem:
    screen: str
    theme: str
    width: int
    state: str
    before: dict[str, Any] | None
    after: dict[str, Any] | None
    output: dict[str, str] = field(default_factory=dict)

    @property
    def key(self) -> str:
        return f"{self.screen}--{self.theme}--{self.width}"


@dataclass
class Selection:
    screens: frozenset[str] | None = None
    themes: frozenset[str] | None = None
    widths: frozenset[int] | None = None

    def admits(self, screen: str, theme: str, width: int) -> bool:
        return (
            (self.screens is None or screen in self.screens)
            and (self.themes is None or theme in self.themes)
            and (self.widths is None or width in self.widths)
        )


def _identity_hash(capture: dict[str, Any]) -> str | None:
    return capture.get("pixelSha256") or capture.get("sha256")


def compare_manifests(
    before: dict[str, Any],
    after: dict[str, Any],
    selection: Selection | None = None,
) -> list[ComparisonItem]:
    """Pair captures by (screen, theme, width) and classify each pair."""
    selection = selection or Selection()
    before_map = {(c["screen"], c["theme"], int(c["width"])): c for c in before["captures"]}
    after_map = {(c["screen"], c["theme"], int(c["width"])): c for c in after["captures"]}
    items: list[ComparisonItem] = []
    for identity in sorted(set(before_map) | set(after_map), key=lambda key: _sort_key(key, before, after)):
        screen, theme, width = identity
        if not selection.admits(screen, theme, width):
            continue
        left = before_map.get(identity)
        right = after_map.get(identity)
        left_ok = left is not None and left["status"] != "failed"
        right_ok = right is not None and right["status"] != "failed"
        if left_ok and right_ok:
            assert left is not None and right is not None
            same = _identity_hash(left) is not None and _identity_hash(left) == _identity_hash(right)
            state = STATE_UNCHANGED if same else STATE_CHANGED
        elif right_ok:
            state = STATE_ADDED if left is None else STATE_FAILED
        elif left_ok:
            state = STATE_REMOVED if right is None else STATE_FAILED
        else:
            state = STATE_FAILED
        items.append(ComparisonItem(screen, theme, width, state, left, right))
    return items


def _screen_order(manifests: Iterable[dict[str, Any]]) -> list[dict[str, Any]]:
    order: list[dict[str, Any]] = []
    seen: set[str] = set()
    for manifest in manifests:
        for screen in manifest.get("screens", []):
            if screen["id"] not in seen:
                seen.add(screen["id"])
                order.append(screen)
    return order


def _sort_key(identity: tuple[str, str, int], before: dict[str, Any], after: dict[str, Any]) -> tuple[int, str, int, str]:
    screens = [screen["id"] for screen in _screen_order((after, before))]
    themes = list(dict.fromkeys([*after["matrix"].get("themes", []), *before["matrix"].get("themes", [])]))
    screen, theme, width = identity
    screen_rank = screens.index(screen) if screen in screens else len(screens)
    theme_rank = themes.index(theme) if theme in themes else len(themes)
    return (screen_rank, f"{theme_rank:04d}{theme}", width, screen)


PairWriter = Callable[[Path, Path, Path], None]


def imagemagick_pair_writer(quality: int = 90) -> PairWriter:
    """Join two equally sized images side by side into one WebP file."""
    command = shutil.which("magick") or shutil.which("convert")
    if command is None:
        raise RuntimeError("ImageMagick (magick or convert) is required to build the comparison page.")

    def write(left: Path, right: Path, output: Path) -> None:
        subprocess.run(
            [command, str(left), str(right), "+append", "-quality", str(quality), "-define", "webp:method=4", str(output)],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
        )

    return write


def build_comparison(
    before_dir: Path,
    after_dir: Path,
    output_dir: Path,
    *,
    selection: Selection | None = None,
    pair_writer: PairWriter | None = None,
    max_files: int = DEFAULT_MAX_FILES,
    max_bytes: int = DEFAULT_MAX_BYTES,
    workers: int = 4,
) -> dict[str, Any]:
    """Write ``index.html`` and image copies; return a summary dictionary.

    Changed pairs are joined into one side-by-side image so a full matrix stays
    under ``max_files``. Unchanged captures are copied once; added, removed, and
    failed captures keep whichever side exists.
    """
    before = load_manifest(before_dir)
    after = load_manifest(after_dir)
    items = compare_manifests(before, after, selection)
    images_dir = output_dir / "images"
    if output_dir.exists():
        index = output_dir / "index.html"
        if index.exists():
            index.unlink()
        if images_dir.exists():
            shutil.rmtree(images_dir)
    images_dir.mkdir(parents=True, exist_ok=True)

    projected_files = 1 + sum(1 for item in items if _needs_file(item))
    if projected_files > max_files:
        raise RuntimeError(
            f"The comparison would write {projected_files} files (limit {max_files}). "
            "Limit it with --screens, --themes, or --widths."
        )

    jobs: list[tuple[ComparisonItem, Path, Path]] = []
    for item in items:
        if item.state == STATE_CHANGED:
            assert item.before is not None and item.after is not None
            left = before_dir / item.before["file"]
            right = after_dir / item.after["file"]
            if _same_size(item.before, item.after):
                target = images_dir / f"{item.key}.pair.webp"
                jobs.append((item, left, right))
                item.output = {"pair": f"images/{target.name}"}
            else:
                before_name = f"{item.key}.before{left.suffix}"
                after_name = f"{item.key}.after{right.suffix}"
                shutil.copyfile(left, images_dir / before_name)
                shutil.copyfile(right, images_dir / after_name)
                item.output = {"before": f"images/{before_name}", "after": f"images/{after_name}"}
        elif item.state in (STATE_UNCHANGED, STATE_ADDED) or (item.state == STATE_FAILED and _ok(item.after)):
            assert item.after is not None
            source = after_dir / item.after["file"]
            name = f"{item.key}{source.suffix}"
            shutil.copyfile(source, images_dir / name)
            item.output = {"single": f"images/{name}"}
        elif item.state == STATE_REMOVED or (item.state == STATE_FAILED and _ok(item.before)):
            assert item.before is not None
            source = before_dir / item.before["file"]
            name = f"{item.key}{source.suffix}"
            shutil.copyfile(source, images_dir / name)
            item.output = {"single": f"images/{name}"}

    if jobs:
        writer = pair_writer or imagemagick_pair_writer()
        if workers > 1 and len(jobs) > 1:
            from concurrent.futures import ThreadPoolExecutor

            with ThreadPoolExecutor(max_workers=workers) as pool:
                list(pool.map(lambda job: writer(job[1], job[2], output_dir / job[0].output["pair"]), jobs))
        else:
            for item, left, right in jobs:
                writer(left, right, output_dir / item.output["pair"])

    page = render_comparison_page(before, after, items)
    (output_dir / "index.html").write_text(page, encoding="utf-8")
    files = [path for path in output_dir.rglob("*") if path.is_file()]
    total_bytes = sum(path.stat().st_size for path in files)
    summary = summarize(items)
    summary.update({"files": len(files), "bytes": total_bytes, "index": str(output_dir / "index.html")})
    if total_bytes > max_bytes:
        summary["warning"] = (
            f"The comparison is {total_bytes / 1048576:.1f} MiB, above the {max_bytes / 1048576:.0f} MiB budget."
        )
    return summary


def _ok(capture: dict[str, Any] | None) -> bool:
    return capture is not None and capture.get("status") != "failed" and bool(capture.get("file"))


def _needs_file(item: ComparisonItem) -> bool:
    if item.state == STATE_CHANGED:
        return True
    if item.state == STATE_FAILED:
        return _ok(item.before) or _ok(item.after)
    return True


def _same_size(left: dict[str, Any], right: dict[str, Any]) -> bool:
    return int(left["width"]) == int(right["width"]) and int(left["height"]) == int(right["height"])


def summarize(items: Sequence[ComparisonItem]) -> dict[str, Any]:
    counts = {state: 0 for state in (STATE_CHANGED, STATE_UNCHANGED, STATE_ADDED, STATE_REMOVED, STATE_FAILED)}
    changed_screens: dict[str, list[str]] = {}
    for item in items:
        counts[item.state] += 1
        if item.state != STATE_UNCHANGED:
            changed_screens.setdefault(item.screen, []).append(f"{item.theme}/{item.width}")
    return {"counts": counts, "changedScreens": changed_screens, "total": len(items)}


def _page_data(before: dict[str, Any], after: dict[str, Any], items: Sequence[ComparisonItem]) -> dict[str, Any]:
    screens = _screen_order((after, before))
    themes = list(dict.fromkeys([*after["matrix"].get("themes", []), *before["matrix"].get("themes", [])]))
    widths = sorted({item.width for item in items})
    present_themes = [theme for theme in themes if any(item.theme == theme for item in items)]
    viewports = {
        int(viewport["width"]): viewport
        for manifest in (before, after)
        for viewport in manifest["matrix"].get("viewports", [])
    }
    return {
        "before": _side_summary(before),
        "after": _side_summary(after),
        "screens": [
            {"id": screen["id"], "title": screen.get("title", screen["id"]), "group": screen.get("group", "")}
            for screen in screens
            if any(item.screen == screen["id"] for item in items)
        ],
        "themes": present_themes,
        "widths": [
            {"width": width, "label": str(viewports.get(width, {}).get("label") or width)}
            for width in widths
        ],
        "items": [
            {
                "screen": item.screen,
                "theme": item.theme,
                "width": item.width,
                "height": int((item.after or item.before or {}).get("height") or 0),
                "state": item.state,
                "output": item.output,
                "note": " ".join(
                    note for note in ((item.before or {}).get("note"), (item.after or {}).get("note")) if note
                ),
            }
            for item in items
        ],
    }


def _side_summary(manifest: dict[str, Any]) -> dict[str, Any]:
    source = manifest.get("source", {})
    return {
        "sha": source.get("sha") or "",
        "label": source.get("label") or source.get("ref") or "",
        "dirty": bool(source.get("dirty")),
        "capturedAt": manifest.get("capturedAt", ""),
        "captureDate": manifest.get("captureDate", ""),
        "warnings": list(manifest.get("warnings", [])),
    }


def render_comparison_page(
    before: dict[str, Any], after: dict[str, Any], items: Sequence[ComparisonItem]
) -> str:
    """Return a self-contained HTML page: inline CSS and script, relative images."""
    data = _page_data(before, after, items)
    summary = summarize(items)
    warnings: list[str] = []
    if data["before"]["captureDate"] and data["before"]["captureDate"] != data["after"]["captureDate"]:
        warnings.append(
            "The two sides were captured on different days "
            f"({data['before']['captureDate']} and {data['after']['captureDate']}). "
            "Seeded dates follow the capture day, so date text differs on every screen that shows one."
        )
    for side in ("before", "after"):
        for warning in data[side]["warnings"]:
            warnings.append(f"{side.title()}: {warning}")
    data["warnings"] = warnings
    payload = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("</", "<\\/")
    counts = summary["counts"]
    title = f"UI screenshots: {_short(data['before']['sha'])} → {_short(data['after']['sha'])}"
    return _PAGE_TEMPLATE.format(
        title=html.escape(title),
        heading=html.escape(title),
        changed=counts[STATE_CHANGED],
        unchanged=counts[STATE_UNCHANGED],
        other=counts[STATE_ADDED] + counts[STATE_REMOVED] + counts[STATE_FAILED],
        payload=payload,
    )


def _short(sha: str) -> str:
    return sha[:10] if sha else "unknown"


_PAGE_TEMPLATE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
:root {{
  color-scheme: light dark;
  --bg: #f6f7f9; --fg: #1d2330; --muted: #5b6475; --panel: #ffffff; --line: #d9dde5;
  --changed: #b54708; --unchanged: #2e7d32; --added: #1d4ed8; --removed: #9f1239; --failed: #b91c1c;
}}
@media (prefers-color-scheme: dark) {{
  :root {{ --bg: #11141a; --fg: #e6e9ef; --muted: #9aa3b2; --panel: #1a1f28; --line: #2c3340;
    --changed: #f59e0b; --unchanged: #4ade80; --added: #60a5fa; --removed: #fb7185; --failed: #f87171; }}
}}
* {{ box-sizing: border-box; }}
body {{ margin: 0; background: var(--bg); color: var(--fg); font: 15px/1.45 system-ui, -apple-system, "Segoe UI", sans-serif; }}
header {{ position: sticky; top: 0; z-index: 2; background: var(--panel); border-bottom: 1px solid var(--line); padding: 12px 16px; }}
h1 {{ font-size: 18px; margin: 0 0 6px; }}
.meta {{ color: var(--muted); font-size: 13px; display: flex; flex-wrap: wrap; gap: 4px 18px; }}
.controls {{ display: flex; flex-wrap: wrap; gap: 8px 18px; align-items: center; margin-top: 10px; }}
.controls label {{ font-size: 13px; color: var(--muted); display: inline-flex; gap: 6px; align-items: center; }}
select {{ font: inherit; padding: 4px 6px; border-radius: 6px; border: 1px solid var(--line); background: var(--bg); color: var(--fg); }}
.counts span {{ margin-right: 12px; font-size: 13px; }}
main {{ padding: 16px; max-width: 3200px; margin: 0 auto; }}
.warning {{ border: 1px solid var(--changed); border-radius: 8px; padding: 8px 12px; margin: 0 0 12px; }}
section.item {{ background: var(--panel); border: 1px solid var(--line); border-radius: 10px; padding: 12px; margin: 0 0 18px; }}
section.item h2 {{ font-size: 16px; margin: 0 0 8px; display: flex; gap: 10px; align-items: baseline; flex-wrap: wrap; }}
.badge {{ font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: .04em; }}
.badge.changed {{ color: var(--changed); }} .badge.added {{ color: var(--added); }}
.badge.removed {{ color: var(--removed); }} .badge.failed {{ color: var(--failed); }} .badge.unchanged {{ color: var(--unchanged); }}
.note {{ color: var(--muted); font-size: 13px; margin: 0 0 8px; }}
.pair {{ display: grid; grid-template-columns: 1fr 1fr; gap: 12px; align-items: start; }}
.side h3 {{ font-size: 13px; color: var(--muted); margin: 0 0 4px; font-weight: 600; }}
.frame {{ position: relative; overflow: hidden; border: 1px solid var(--line); border-radius: 6px; background: var(--bg); }}
/* Frames are capped at the capture width so small captures are never enlarged. */
.frame img {{ display: block; width: 100%; height: auto; }}
.frame.half img {{ width: 200%; max-width: none; }}
.frame.half.right img {{ transform: translateX(-50%); }}
details.nochange {{ background: var(--panel); border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px; }}
details.nochange summary {{ cursor: pointer; font-weight: 600; }}
details.nochange ul {{ margin: 8px 0 0; padding-left: 20px; columns: 2 260px; }}
details.nochange li button {{ font: inherit; color: inherit; background: none; border: 0; padding: 0; text-decoration: underline dotted; cursor: pointer; }}
.preview {{ margin-top: 10px; }}
.empty {{ color: var(--muted); }}
@media (max-width: 900px) {{ .pair {{ grid-template-columns: 1fr; }} }}
</style>
</head>
<body>
<header>
<h1>{heading}</h1>
<div class="meta" id="meta"></div>
<div class="controls">
<label>Theme <select id="theme"></select></label>
<label>Width <select id="width"></select></label>
<label><input type="checkbox" id="all"> Show every theme and width</label>
<span class="counts"><span>{changed} changed</span><span>{unchanged} unchanged</span><span>{other} added, removed, or failed</span></span>
</div>
</header>
<main id="main"></main>
<script id="data" type="application/json">{payload}</script>
<script>
(() => {{
  const data = JSON.parse(document.getElementById("data").textContent);
  const themeSelect = document.getElementById("theme");
  const widthSelect = document.getElementById("width");
  const showAll = document.getElementById("all");
  const main = document.getElementById("main");
  const titles = Object.fromEntries(data.screens.map((s) => [s.id, s.title]));
  const order = Object.fromEntries(data.screens.map((s, i) => [s.id, i]));
  const el = (tag, attrs = {{}}, ...children) => {{
    const node = document.createElement(tag);
    for (const [k, v] of Object.entries(attrs)) {{
      if (k === "class") node.className = v; else if (k === "text") node.textContent = v; else node.setAttribute(k, v);
    }}
    for (const child of children) if (child) node.append(child);
    return node;
  }};
  const describe = (side) => `${{side.label || "(no label)"}} @ ${{(side.sha || "unknown").slice(0, 12)}}${{side.dirty ? " + local changes" : ""}} · ${{side.capturedAt}}`;
  document.getElementById("meta").append(
    el("span", {{ text: "Before: " + describe(data.before) }}),
    el("span", {{ text: "After: " + describe(data.after) }}),
  );
  for (const theme of data.themes) themeSelect.append(el("option", {{ value: theme, text: theme }}));
  for (const width of data.widths) widthSelect.append(el("option", {{ value: String(width.width), text: `${{width.label}} (${{width.width}} px)` }}));
  const choose = (select, value) => {{
    if (value && [...select.options].some((option) => option.value === value)) select.value = value;
  }};
  const applyHash = () => {{
    const params = new URLSearchParams(location.hash.slice(1));
    choose(themeSelect, params.get("theme"));
    choose(widthSelect, params.get("width"));
    showAll.checked = params.get("all") === "1";
  }};
  const syncHash = (next) => {{
    if (location.hash.slice(1) === next) return;
    try {{ history.replaceState(null, "", "#" + next); }} catch {{ location.hash = next; }}
  }};
  applyHash();
  const label = (item) => `${{titles[item.screen] || item.screen}} · ${{item.theme}} · ${{item.width}} px`;
  const frame = (item, src, alt, half) => el("div", {{ class: "frame" + (half ? " half " + half : ""),
    style: `max-width: ${{item.width + 2}}px` }}, el("img", {{ src, alt, loading: "lazy", decoding: "async" }}));
  const side = (item, title, src, alt, half) => el("div", {{ class: "side" }},
    el("h3", {{ text: title }}), frame(item, src, alt, half));
  const render = () => {{
    const theme = themeSelect.value;
    const width = Number(widthSelect.value);
    themeSelect.disabled = widthSelect.disabled = showAll.checked;
    syncHash(new URLSearchParams({{ theme, width: String(width), all: showAll.checked ? "1" : "0" }}).toString());
    const items = data.items
      .filter((item) => showAll.checked || (item.theme === theme && item.width === width))
      .sort((a, b) => (order[a.screen] - order[b.screen]) || a.theme.localeCompare(b.theme) || a.width - b.width);
    main.replaceChildren();
    for (const warning of data.warnings) main.append(el("p", {{ class: "warning", text: warning }}));
    const unchanged = [];
    for (const item of items) {{
      if (item.state === "unchanged") {{ unchanged.push(item); continue; }}
      const section = el("section", {{ class: "item", id: `${{item.screen}}--${{item.theme}}--${{item.width}}` }},
        el("h2", {{}}, el("span", {{ text: label(item) }}), el("span", {{ class: "badge " + item.state, text: item.state }})));
      if (item.note) section.append(el("p", {{ class: "note", text: item.note }}));
      const out = item.output || {{}};
      if (out.pair) {{
        section.append(el("div", {{ class: "pair" }},
          side(item, "Before", out.pair, "Before: " + label(item), "left"),
          side(item, "After", out.pair, "After: " + label(item), "right")));
      }} else if (out.before && out.after) {{
        section.append(el("div", {{ class: "pair" }},
          side(item, "Before", out.before, "Before: " + label(item)),
          side(item, "After", out.after, "After: " + label(item))));
      }} else if (out.single) {{
        const which = item.state === "removed" ? "Before only" : item.state === "added" ? "After only" : "Available side";
        section.append(el("div", {{ class: "single" }}, side(item, which, out.single, which + ": " + label(item))));
      }} else {{
        section.append(el("p", {{ class: "empty", text: "No image was captured on either side." }}));
      }}
      main.append(section);
    }}
    if (!main.querySelector("section.item")) main.append(el("p", {{ class: "empty", text: "No differences for this selection." }}));
    if (unchanged.length) {{
      const list = el("ul");
      const preview = el("div", {{ class: "preview" }});
      for (const item of unchanged) {{
        const button = el("button", {{ type: "button", text: label(item) }});
        button.addEventListener("click", () => {{
          preview.replaceChildren(el("div", {{ class: "single" }}, side(item, label(item), item.output.single, label(item))));
        }});
        list.append(el("li", {{}}, button));
      }}
      main.append(el("details", {{ class: "nochange" }},
        el("summary", {{ text: `No change (${{unchanged.length}})` }}), list, preview));
    }}
  }};
  themeSelect.addEventListener("change", render);
  widthSelect.addEventListener("change", render);
  showAll.addEventListener("change", render);
  window.addEventListener("hashchange", () => {{ applyHash(); render(); }});
  render();
}})();
</script>
</body>
</html>
"""
