#!/usr/bin/env python3
"""Capture and verify the Stage 0C native UI visual-baseline matrix."""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import platform
import re
import shutil
import signal
import struct
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Any, NoReturn

import smoke as webdriver
import ui_platform_smoke as ui


ROOT_DIR = Path(__file__).resolve().parents[2]
BASELINE_DIR = ROOT_DIR / "tests/visual-baselines/ui-workflow-redesign-stage-0"
BASELINE_SOURCE_SHA = "d8e723c1cbc1a60f92881ca4e92b44268a418309"
BASELINE_ID = "ui-workflow-redesign-stage-0"
SCENARIO_ID = "flashcards-library"
SCENARIO_VERSION = "flashcards-library-v1"
MANIFEST_SCHEMA_VERSION = 1
THEMES = ("light", "dark", "console", "synthwave")
VIEWPORTS = (
    ("wide", 1440, 900),
    ("medium", 1024, 768),
    ("compact", 390, 844),
)
FOLDER_NAME = "Clinical foundations"
DECK_NAME = "Cardiopulmonary essentials"
DECK_DESCRIPTION = "Representative Stage 0 visual baseline deck"
QUESTION = "What is the primary role of pulmonary surfactant?"
ANSWER = "It lowers alveolar surface tension and helps prevent collapse."
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"


class BaselineValidationError(ValueError):
    """The committed baseline evidence does not satisfy its strict contract."""


def fail(message: str) -> NoReturn:
    raise BaselineValidationError(message)


def canonical_filename(theme: str, layout: str, width: int, height: int) -> str:
    return f"{SCENARIO_VERSION}--{theme}--{layout}-{width}x{height}.png"


def expected_coordinates() -> list[tuple[str, str, int, int, str]]:
    return [
        (theme, layout, width, height, canonical_filename(theme, layout, width, height))
        for theme in THEMES
        for layout, width, height in VIEWPORTS
    ]


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            fail(f"Manifest contains duplicate JSON key {key!r}.")
        result[key] = value
    return result


def load_manifest(path: Path) -> dict[str, Any]:
    try:
        decoded = json.loads(
            path.read_text(encoding="utf-8"),
            object_pairs_hook=reject_duplicate_keys,
        )
    except FileNotFoundError as error:
        fail(f"Missing visual-baseline manifest: {path}")
    except json.JSONDecodeError as error:
        fail(f"Invalid visual-baseline manifest JSON: {error}")
    if not isinstance(decoded, dict):
        fail("Visual-baseline manifest must be a JSON object.")
    return decoded


def require_exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        fail(
            f"{label} keys differ; missing={sorted(expected - actual)!r}, "
            f"extra={sorted(actual - expected)!r}."
        )


def require_int(value: Any, label: str, *, minimum: int = 0) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < minimum:
        fail(f"{label} must be an integer >= {minimum}; received {value!r}.")
    return value


def require_number(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or float(value) <= 0:
        fail(f"{label} must be a positive number; received {value!r}.")
    return float(value)


def png_dimensions(path: Path) -> tuple[int, int]:
    try:
        header = path.read_bytes()[:24]
    except FileNotFoundError:
        fail(f"Missing baseline PNG: {path.name}")
    if len(header) != 24 or header[:8] != PNG_SIGNATURE or header[12:16] != b"IHDR":
        fail(f"Baseline image is not a PNG with an IHDR header: {path.name}")
    return struct.unpack(">II", header[16:24])


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def verify_manifest(
    baseline_dir: Path = BASELINE_DIR,
    *,
    expected_source_sha: str = BASELINE_SOURCE_SHA,
) -> dict[str, Any]:
    """Strictly verify the complete committed matrix without changing it."""
    baseline_dir = baseline_dir.resolve()
    manifest = load_manifest(baseline_dir / "manifest.json")
    require_exact_keys(
        manifest,
        {
            "schemaVersion",
            "baselineId",
            "baselineSourceSha",
            "scenario",
            "matrix",
            "captureEnvironment",
            "captures",
        },
        "manifest",
    )
    if manifest["schemaVersion"] != MANIFEST_SCHEMA_VERSION:
        fail(f"Unsupported manifest schemaVersion {manifest['schemaVersion']!r}.")
    if manifest["baselineId"] != BASELINE_ID:
        fail(f"Unexpected baselineId {manifest['baselineId']!r}.")
    if manifest["baselineSourceSha"] != expected_source_sha:
        fail(
            "Stale baseline source SHA: "
            f"expected {expected_source_sha}, received {manifest['baselineSourceSha']!r}."
        )

    scenario = manifest["scenario"]
    if not isinstance(scenario, dict):
        fail("scenario must be an object.")
    require_exact_keys(scenario, {"id", "version", "fixtureBoundary", "description"}, "scenario")
    if scenario["id"] != SCENARIO_ID or scenario["version"] != SCENARIO_VERSION:
        fail(
            "Stale or unexpected scenario fixture: "
            f"id={scenario['id']!r}, version={scenario['version']!r}."
        )
    if scenario["fixtureBoundary"] != "fresh-temporary-xdg-ui-seeded":
        fail(f"Unexpected fixture boundary {scenario['fixtureBoundary']!r}.")
    if not isinstance(scenario["description"], str) or not scenario["description"].strip():
        fail("scenario.description must be a non-empty string.")

    matrix = manifest["matrix"]
    if not isinstance(matrix, dict):
        fail("matrix must be an object.")
    require_exact_keys(matrix, {"themes", "viewports"}, "matrix")
    if matrix["themes"] != list(THEMES):
        fail(f"Theme matrix differs from the supported themes: {matrix['themes']!r}.")
    expected_viewports = [
        {"layoutClass": layout, "width": width, "height": height}
        for layout, width, height in VIEWPORTS
    ]
    if matrix["viewports"] != expected_viewports:
        fail(f"Viewport matrix differs from the Stage 0 contract: {matrix['viewports']!r}.")

    environment = manifest["captureEnvironment"]
    if not isinstance(environment, dict):
        fail("captureEnvironment must be an object.")
    require_exact_keys(
        environment,
        {
            "applicationSha256",
            "python",
            "platform",
            "tauriDriver",
            "webkitWebDriver",
            "xvfb",
            "display",
        },
        "captureEnvironment",
    )
    for key in ("applicationSha256", "python", "platform", "tauriDriver", "webkitWebDriver", "xvfb"):
        if not isinstance(environment[key], str) or not environment[key].strip():
            fail(f"captureEnvironment.{key} must be a non-empty string.")
    if re.fullmatch(r"[0-9a-f]{64}", environment["applicationSha256"]) is None:
        fail("captureEnvironment.applicationSha256 is not a SHA-256 digest.")
    display = environment["display"]
    if display != {"width": 1920, "height": 1200, "depth": 24, "dpi": 96}:
        fail(f"Unexpected virtual-display contract: {display!r}.")

    captures = manifest["captures"]
    if not isinstance(captures, list):
        fail("captures must be an array.")
    coordinates = expected_coordinates()
    if len(captures) != len(coordinates):
        fail(f"Expected exactly {len(coordinates)} captures, received {len(captures)}.")

    seen_coordinates: set[tuple[str, str]] = set()
    seen_paths: set[str] = set()
    seen_hashes: set[str] = set()
    referenced_files: set[str] = set()
    capture_keys = {
        "theme",
        "resolvedTheme",
        "layoutClass",
        "requestedViewport",
        "actualViewport",
        "rootViewport",
        "outerWindowRect",
        "devicePixelRatio",
        "pngDimensions",
        "path",
        "sha256",
    }
    rectangle_keys = {"x", "y", "width", "height"}
    size_keys = {"width", "height"}

    for index, (capture, expected) in enumerate(zip(captures, coordinates, strict=True)):
        if not isinstance(capture, dict):
            fail(f"captures[{index}] must be an object.")
        require_exact_keys(capture, capture_keys, f"captures[{index}]")
        theme, layout, width, height, filename = expected
        coordinate = (str(capture["theme"]), str(capture["layoutClass"]))
        if coordinate in seen_coordinates:
            fail(f"Duplicate matrix coordinate {coordinate!r}.")
        seen_coordinates.add(coordinate)
        if capture["theme"] != theme or capture["resolvedTheme"] != theme:
            fail(
                f"captures[{index}] has wrong theme metadata; expected {theme!r}, "
                f"received theme={capture['theme']!r}, resolvedTheme={capture['resolvedTheme']!r}."
            )
        if capture["layoutClass"] != layout:
            fail(
                f"captures[{index}] has wrong layout class; expected {layout!r}, "
                f"received {capture['layoutClass']!r}."
            )

        for field in ("requestedViewport", "actualViewport", "rootViewport", "pngDimensions"):
            size = capture[field]
            if not isinstance(size, dict):
                fail(f"captures[{index}].{field} must be an object.")
            require_exact_keys(size, size_keys, f"captures[{index}].{field}")
            actual_size = {
                "width": require_int(size["width"], f"captures[{index}].{field}.width", minimum=1),
                "height": require_int(size["height"], f"captures[{index}].{field}.height", minimum=1),
            }
            if field != "pngDimensions" and actual_size != {"width": width, "height": height}:
                fail(
                    f"captures[{index}].{field} does not match {width}x{height}: {actual_size!r}."
                )

        outer = capture["outerWindowRect"]
        if not isinstance(outer, dict):
            fail(f"captures[{index}].outerWindowRect must be an object.")
        require_exact_keys(outer, rectangle_keys, f"captures[{index}].outerWindowRect")
        for key in ("x", "y"):
            require_int(outer[key], f"captures[{index}].outerWindowRect.{key}", minimum=0)
        require_int(outer["width"], f"captures[{index}].outerWindowRect.width", minimum=width)
        require_int(outer["height"], f"captures[{index}].outerWindowRect.height", minimum=height)

        dpr = require_number(capture["devicePixelRatio"], f"captures[{index}].devicePixelRatio")
        expected_png = {"width": round(width * dpr), "height": round(height * dpr)}
        if capture["pngDimensions"] != expected_png:
            fail(
                f"captures[{index}].pngDimensions does not match viewport × DPR "
                f"{expected_png!r}: {capture['pngDimensions']!r}."
            )

        relative_path = capture["path"]
        if not isinstance(relative_path, str) or relative_path != filename:
            fail(
                f"captures[{index}] path is not canonical; expected {filename!r}, "
                f"received {relative_path!r}."
            )
        path = Path(relative_path)
        if path.is_absolute() or len(path.parts) != 1 or path.name != relative_path:
            fail(f"captures[{index}] path escapes the baseline directory: {relative_path!r}.")
        if relative_path in seen_paths:
            fail(f"Duplicate capture path {relative_path!r}.")
        seen_paths.add(relative_path)
        referenced_files.add(relative_path)

        digest = capture["sha256"]
        if not isinstance(digest, str) or re.fullmatch(r"[0-9a-f]{64}", digest) is None:
            fail(f"captures[{index}].sha256 is not a lowercase SHA-256 digest.")
        if digest in seen_hashes:
            fail(f"Duplicate capture image hash {digest}.")
        seen_hashes.add(digest)

        image_path = baseline_dir / relative_path
        actual_dimensions = png_dimensions(image_path)
        if actual_dimensions != (expected_png["width"], expected_png["height"]):
            fail(
                f"PNG dimensions for {relative_path} are {actual_dimensions[0]}x{actual_dimensions[1]}, "
                f"expected {expected_png['width']}x{expected_png['height']}."
            )
        actual_digest = sha256_file(image_path)
        if actual_digest != digest:
            fail(
                f"SHA-256 mismatch for {relative_path}: manifest={digest}, actual={actual_digest}."
            )

    actual_files = {
        path.name
        for path in baseline_dir.iterdir()
        if path.is_file() and path.name != "manifest.json"
    }
    if actual_files != referenced_files:
        fail(
            "Baseline directory files differ from manifest captures; "
            f"missing={sorted(referenced_files - actual_files)!r}, "
            f"unexpected={sorted(actual_files - referenced_files)!r}."
        )
    return manifest


def run_version(command: list[str], fallback: str) -> str:
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT_DIR,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            check=False,
            timeout=10,
        )
    except (OSError, subprocess.TimeoutExpired):
        return fallback
    output = " ".join(completed.stdout.split())
    return output[:500] if output else fallback


def tauri_driver_version() -> str:
    installed = run_version(["cargo", "install", "--list"], "version unavailable")
    match = re.search(r"(?:^|\s)tauri-driver v([^: ]+):", installed)
    return f"tauri-driver {match.group(1)}" if match else "tauri-driver version unavailable"


def assert_production_sources_match_baseline() -> None:
    tracked = subprocess.run(
        [
            "git",
            "diff",
            "--quiet",
            BASELINE_SOURCE_SHA,
            "--",
            "src",
            "src-tauri",
            "package.json",
            "pnpm-lock.yaml",
            "pnpm-workspace.yaml",
            "index.html",
            "vite.config.ts",
        ],
        cwd=ROOT_DIR,
        check=False,
    )
    untracked = subprocess.run(
        [
            "git",
            "ls-files",
            "--others",
            "--exclude-standard",
            "--",
            "src",
            "src-tauri",
        ],
        cwd=ROOT_DIR,
        text=True,
        stdout=subprocess.PIPE,
        check=True,
    ).stdout.strip()
    if tracked.returncode != 0 or untracked:
        raise RuntimeError(
            "Production UI/native sources differ from baseline source SHA "
            f"{BASELINE_SOURCE_SHA}; capture must run before redesign source changes."
        )


def execute(session_id: str, script: str, args: list[Any] | None = None) -> Any:
    return ui.execute(session_id, script, args)


def wait_for_root_value(session_id: str, dataset_key: str, expected: str, deadline: float) -> None:
    last = ""
    while time.monotonic() < deadline:
        last = str(
            execute(
                session_id,
                "return document.documentElement.dataset[arguments[0]] || '';",
                [dataset_key],
            )
        )
        if last == expected:
            return
        time.sleep(0.05)
    raise AssertionError(
        f"Expected documentElement.dataset.{dataset_key}={expected!r}, received {last!r}."
    )


def create_fixture(session_id: str) -> None:
    deadline = webdriver.deadline_after(60)
    webdriver.click_text_button(session_id, "New folder", deadline)
    webdriver.set_input_text(
        session_id,
        "css selector",
        "[role='dialog'] input[data-autofocus='true']",
        FOLDER_NAME,
        deadline,
    )
    create_folder = webdriver.wait_for_enabled_element(
        session_id,
        "xpath",
        "//*[@role='dialog']//button[normalize-space(.)='Create folder']",
        deadline,
    )
    webdriver.click(session_id, create_folder)
    ui.wait_for_overlay_inactive(session_id, deadline, "visual fixture folder creation")
    webdriver.find_element(
        session_id,
        "xpath",
        f"//main//*[normalize-space(.)={json.dumps(FOLDER_NAME)}]",
        deadline,
    )

    webdriver.click_text_button(session_id, "New deck", deadline)
    webdriver.set_input_text(
        session_id,
        "css selector",
        "[role='dialog'] input[data-autofocus='true']",
        DECK_NAME,
        deadline,
    )
    webdriver.set_input_text(
        session_id,
        "css selector",
        "[role='dialog'] textarea",
        DECK_DESCRIPTION,
        deadline,
    )
    create_deck = webdriver.wait_for_enabled_element(
        session_id,
        "xpath",
        "//*[@role='dialog']//button[normalize-space(.)='Create deck']",
        deadline,
    )
    webdriver.click(session_id, create_deck)
    webdriver.wait_for_main_heading(session_id, DECK_NAME, deadline)
    ui.wait_for_overlay_inactive(session_id, deadline, "visual fixture deck creation")

    webdriver.click_text_button(session_id, "Add flashcard", deadline)
    webdriver.set_rich_editor_text(session_id, 0, QUESTION, deadline)
    webdriver.set_rich_editor_text(session_id, 1, ANSWER, deadline)
    save = webdriver.wait_for_enabled_element(
        session_id,
        "xpath",
        "//button[normalize-space(.)='Save card']",
        deadline,
    )
    webdriver.click(session_id, save)
    webdriver.wait_for_main_heading(session_id, DECK_NAME, deadline)
    webdriver.find_element(
        session_id,
        "xpath",
        f"//main//strong[normalize-space(.)={json.dumps(QUESTION)}]",
        deadline,
    )
    # Re-entering Flashcards through the maintained top-level navigation resets
    # the deck-detail route to the library root independently of the committed
    # card's asynchronous list reconciliation.
    flashcards = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks']",
        deadline,
    )
    webdriver.click(session_id, flashcards)
    wait_for_library(session_id, deadline)


def wait_for_library(session_id: str, deadline: float) -> None:
    webdriver.find_element(
        session_id,
        "css selector",
        "main [role='tablist'][aria-label='Flashcards workspaces']",
        deadline,
    )
    webdriver.find_element(
        session_id,
        "xpath",
        f"//main//*[normalize-space(.)={json.dumps(FOLDER_NAME)}]",
        deadline,
    )
    webdriver.find_element(
        session_id,
        "xpath",
        f"//main//strong[normalize-space(.)={json.dumps(DECK_NAME)}]",
        deadline,
    )


def dismiss_notifications(session_id: str, deadline: float) -> None:
    while time.monotonic() < deadline:
        result = webdriver.request(
            "POST",
            f"/session/{session_id}/elements",
            {"using": "css selector", "value": "button[aria-label='Dismiss notification']"},
        )
        elements = [
            str(item[webdriver.ELEMENT_KEY])
            for item in result
            if isinstance(item, dict) and webdriver.ELEMENT_KEY in item
        ] if isinstance(result, list) else []
        if not elements:
            return
        webdriver.click(session_id, elements[0])
        time.sleep(0.05)
    raise AssertionError("Transient notification did not dismiss before capture.")


def select_theme(session_id: str, theme: str) -> None:
    deadline = webdriver.deadline_after(60)
    ui.set_window_rect(session_id, 1440, 900)
    ui.wait_for_layout_mode(session_id, "wide", deadline)
    settings = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-settings']",
        deadline,
    )
    webdriver.click(session_id, settings)
    webdriver.wait_for_element_text(session_id, "css selector", "main h1", "Settings", deadline)
    option = webdriver.find_element(
        session_id,
        "css selector",
        f"button[data-theme-option='{theme}']",
        deadline,
    )
    webdriver.click(session_id, option)
    wait_for_root_value(session_id, "theme", theme, deadline)
    save = webdriver.wait_for_enabled_element(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='Save preferences']",
        deadline,
    )
    webdriver.click(session_id, save)
    ui.wait_for_absent(
        session_id,
        "xpath",
        "//main//button[normalize-space(.)='Save preferences']",
        deadline,
    )
    dismiss_notifications(session_id, deadline)
    flashcards = webdriver.find_element(
        session_id,
        "css selector",
        "button[data-testid='nav-decks']",
        deadline,
    )
    webdriver.click(session_id, flashcards)
    wait_for_library(session_id, deadline)
    wait_for_root_value(session_id, "theme", theme, deadline)


def viewport_snapshot(session_id: str) -> dict[str, Any]:
    snapshot = execute(
        session_id,
        """
        return {
          innerWidth: window.innerWidth,
          innerHeight: window.innerHeight,
          rootWidth: document.documentElement.clientWidth,
          rootHeight: document.documentElement.clientHeight,
          layoutClass: document.documentElement.dataset.layoutMode || '',
          theme: document.documentElement.dataset.theme || '',
          devicePixelRatio: window.devicePixelRatio
        };
        """,
    )
    if not isinstance(snapshot, dict):
        raise AssertionError(f"Unexpected viewport snapshot: {snapshot!r}")
    return snapshot


def achieve_viewport(
    session_id: str,
    width: int,
    height: int,
    expected_layout: str,
) -> tuple[dict[str, Any], dict[str, int]]:
    outer_width = width
    outer_height = height
    last: dict[str, Any] | None = None
    for _ in range(8):
        result = webdriver.request(
            "POST",
            f"/session/{session_id}/window/rect",
            {"width": outer_width, "height": outer_height},
        )
        if not isinstance(result, dict):
            raise AssertionError(f"Unexpected set-window response: {result!r}")
        time.sleep(0.05)
        last = viewport_snapshot(session_id)
        delta_width = width - int(last["innerWidth"])
        delta_height = height - int(last["innerHeight"])
        if delta_width == 0 and delta_height == 0:
            ui.wait_for_layout_mode(session_id, expected_layout, webdriver.deadline_after(10))
            outer = webdriver.request("GET", f"/session/{session_id}/window/rect")
            if not isinstance(outer, dict):
                raise AssertionError(f"Unexpected get-window response: {outer!r}")
            normalized = {
                key: int(outer[key])
                for key in ("x", "y", "width", "height")
            }
            return viewport_snapshot(session_id), normalized
        outer_width += delta_width
        outer_height += delta_height
    raise AssertionError(
        f"Could not achieve exact CSS viewport {width}x{height}; last measurement={last!r}."
    )


def wait_for_capture_ready(session_id: str, theme: str, layout: str, deadline: float) -> dict[str, Any]:
    dismiss_notifications(session_id, deadline)
    execute(
        session_id,
        """
        window.scrollTo({ top: 0, left: 0, behavior: 'auto' });
        if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
        return true;
        """,
    )
    stable_samples = 0
    last: dict[str, Any] | None = None
    while time.monotonic() < deadline:
        last = execute(
            session_id,
            """
            const runningAnimations = document.getAnimations()
              .filter((animation) => animation.playState === 'running').length;
            return {
              theme: document.documentElement.dataset.theme || '',
              layoutClass: document.documentElement.dataset.layoutMode || '',
              overlayActive: document.documentElement.dataset.overlayActive === 'true',
              busyCount: document.querySelectorAll("[aria-busy='true']").length,
              toastCount: document.querySelectorAll("button[aria-label='Dismiss notification']").length,
              fontsReady: document.fonts.status === 'loaded',
              runningAnimations,
              scrollX: window.scrollX,
              scrollY: window.scrollY,
              innerWidth: window.innerWidth,
              innerHeight: window.innerHeight,
              rootWidth: document.documentElement.clientWidth,
              rootHeight: document.documentElement.clientHeight,
              devicePixelRatio: window.devicePixelRatio
            };
            """,
        )
        ready = (
            isinstance(last, dict)
            and last.get("theme") == theme
            and last.get("layoutClass") == layout
            and last.get("overlayActive") is False
            and int(last.get("busyCount", -1)) == 0
            and int(last.get("toastCount", -1)) == 0
            and last.get("fontsReady") is True
            and int(last.get("runningAnimations", -1)) == 0
            and int(last.get("scrollX", -1)) == 0
            and int(last.get("scrollY", -1)) == 0
        )
        stable_samples = stable_samples + 1 if ready else 0
        if stable_samples >= 3:
            return last
        time.sleep(0.1)
    raise AssertionError(f"UI did not reach a stable capture-ready state: {last!r}")


def capture_screenshot(session_id: str) -> bytes:
    encoded = webdriver.request("GET", f"/session/{session_id}/screenshot")
    if not isinstance(encoded, str) or not encoded:
        raise RuntimeError(f"W3C screenshot endpoint returned an invalid payload: {encoded!r}")
    try:
        image = base64.b64decode(encoded, validate=True)
    except (ValueError, base64.binascii.Error) as error:
        raise RuntimeError("W3C screenshot endpoint returned invalid base64.") from error
    if not image.startswith(PNG_SIGNATURE):
        raise RuntimeError("W3C screenshot endpoint did not return a PNG image.")
    return image


def capture_environment(application: Path) -> dict[str, Any]:
    return {
        "applicationSha256": sha256_file(application),
        "python": platform.python_version(),
        "platform": platform.platform(),
        "tauriDriver": tauri_driver_version(),
        "webkitWebDriver": run_version(["WebKitWebDriver", "--version"], "WebKitWebDriver version unavailable"),
        "xvfb": run_version(["Xvfb", "-version"], "Xvfb version unavailable"),
        "display": {"width": 1920, "height": 1200, "depth": 24, "dpi": 96},
    }


def base_manifest(application: Path) -> dict[str, Any]:
    return {
        "schemaVersion": MANIFEST_SCHEMA_VERSION,
        "baselineId": BASELINE_ID,
        "baselineSourceSha": BASELINE_SOURCE_SHA,
        "scenario": {
            "id": SCENARIO_ID,
            "version": SCENARIO_VERSION,
            "fixtureBoundary": "fresh-temporary-xdg-ui-seeded",
            "description": (
                "Flashcards > Decks with one named folder and one unsorted deck containing "
                "one Basic card, created only through existing native UI workflows."
            ),
        },
        "matrix": {
            "themes": list(THEMES),
            "viewports": [
                {"layoutClass": layout, "width": width, "height": height}
                for layout, width, height in VIEWPORTS
            ],
        },
        "captureEnvironment": capture_environment(application),
        "captures": [],
    }


def install_staged_baseline(staging: Path, destination: Path) -> None:
    backup = destination.parent / f".{destination.name}.previous-{os.getpid()}"
    if backup.exists():
        shutil.rmtree(backup)
    moved_existing = False
    try:
        if destination.exists():
            destination.rename(backup)
            moved_existing = True
        staging.rename(destination)
    except Exception:
        if moved_existing and not destination.exists() and backup.exists():
            backup.rename(destination)
        raise
    if backup.exists():
        shutil.rmtree(backup)


def capture_matrix(application: Path, destination: Path) -> dict[str, Any]:
    assert_production_sources_match_baseline()
    application = application.resolve()
    if not application.is_file():
        raise FileNotFoundError(application)
    destination = destination.resolve()
    destination.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(
        tempfile.mkdtemp(prefix=f".{destination.name}.capture-", dir=destination.parent)
    )
    manifest = base_manifest(application)
    session_id: str | None = None
    with tempfile.NamedTemporaryFile("w+", encoding="utf-8", delete=False) as driver_log:
        log_path = Path(driver_log.name)
        driver = subprocess.Popen(
            ["tauri-driver"],
            stdout=driver_log,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
        )
    try:
        webdriver.wait_for_driver(webdriver.deadline_after(20))
        session_id = webdriver.create_session(application)
        webdriver.verify_shell(session_id, webdriver.deadline_after())
        ui.set_window_rect(session_id, 1440, 900)
        ui.wait_for_layout_mode(session_id, "wide", webdriver.deadline_after(20))
        create_fixture(session_id)

        captures: list[dict[str, Any]] = []
        for theme in THEMES:
            print(f"[visual-baseline] Selecting and persisting {theme} theme", flush=True)
            select_theme(session_id, theme)
            for layout, width, height in VIEWPORTS:
                print(
                    f"[visual-baseline] Capturing {theme}/{layout} at {width}x{height} CSS px",
                    flush=True,
                )
                viewport, outer = achieve_viewport(session_id, width, height, layout)
                wait_for_library(session_id, webdriver.deadline_after(30))
                ready = wait_for_capture_ready(
                    session_id,
                    theme,
                    layout,
                    webdriver.deadline_after(30),
                )
                image = capture_screenshot(session_id)
                filename = canonical_filename(theme, layout, width, height)
                image_path = staging / filename
                image_path.write_bytes(image)
                png_width, png_height = png_dimensions(image_path)
                digest = sha256_file(image_path)
                captures.append(
                    {
                        "theme": theme,
                        "resolvedTheme": str(ready["theme"]),
                        "layoutClass": str(ready["layoutClass"]),
                        "requestedViewport": {"width": width, "height": height},
                        "actualViewport": {
                            "width": int(ready["innerWidth"]),
                            "height": int(ready["innerHeight"]),
                        },
                        "rootViewport": {
                            "width": int(ready["rootWidth"]),
                            "height": int(ready["rootHeight"]),
                        },
                        "outerWindowRect": outer,
                        "devicePixelRatio": ready["devicePixelRatio"],
                        "pngDimensions": {"width": png_width, "height": png_height},
                        "path": filename,
                        "sha256": digest,
                    }
                )
                if int(viewport["innerWidth"]) != width or int(viewport["innerHeight"]) != height:
                    raise AssertionError(
                        f"Viewport changed before capture: requested={width}x{height}, actual={viewport!r}."
                    )
        manifest["captures"] = captures
        (staging / "manifest.json").write_text(
            json.dumps(manifest, indent=2, sort_keys=False) + "\n",
            encoding="utf-8",
        )
        verify_manifest(staging)
        install_staged_baseline(staging, destination)
        staging = Path()
        return verify_manifest(destination)
    except Exception:
        print("\n--- tauri-driver visual-baseline log ---")
        try:
            print(log_path.read_text(encoding="utf-8"))
        except OSError:
            pass
        raise
    finally:
        if session_id:
            try:
                webdriver.delete_session(session_id)
            except Exception:
                pass
        if driver.poll() is None:
            try:
                os.killpg(driver.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                driver.wait(timeout=5)
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(driver.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                driver.wait(timeout=5)
        log_path.unlink(missing_ok=True)
        if staging != Path() and staging.exists():
            shutil.rmtree(staging)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)
    verify_parser = subparsers.add_parser("verify", help="verify committed evidence read-only")
    verify_parser.add_argument("--baseline-dir", type=Path, default=BASELINE_DIR)
    capture_parser = subparsers.add_parser("capture", help="explicitly regenerate native evidence")
    capture_parser.add_argument("--application", required=True, type=Path)
    capture_parser.add_argument("--baseline-dir", type=Path, default=BASELINE_DIR)
    args = parser.parse_args()

    if args.command == "capture":
        manifest = capture_matrix(args.application, args.baseline_dir)
        print(
            f"Captured and verified {len(manifest['captures'])} Stage 0C visual baselines "
            f"in {args.baseline_dir.resolve()}."
        )
    else:
        manifest = verify_manifest(args.baseline_dir)
        print(
            f"Verified {len(manifest['captures'])} Stage 0C visual baselines "
            f"in {args.baseline_dir.resolve()} (read-only)."
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
