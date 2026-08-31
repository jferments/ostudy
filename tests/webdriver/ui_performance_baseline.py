#!/usr/bin/env python3
"""Capture, verify, and compare ADR 0049 Stage 0D UI performance evidence."""

from __future__ import annotations

import argparse
import ctypes
import errno
import hashlib
import http.client
import json
import math
import os
import platform
import re
import shutil
import signal
import stat
import statistics
import subprocess
import tempfile
import time
import urllib.error
from pathlib import Path
from typing import Any, Callable, NoReturn

import smoke as webdriver
import ui_platform_smoke as ui


ROOT_DIR = Path(__file__).resolve().parents[2]
BASELINE_DIR = ROOT_DIR / "tests/performance-baselines/ui-workflow-redesign-stage-0"
BASELINE_ID = "ui-workflow-redesign-stage-0"
CANDIDATE_ID = "ui-workflow-redesign-stage-4a-deck-drag-large-library"
SCHEMA_VERSION = 1
INSTRUMENTATION_VERSION = "ui-performance-v1"
FIXTURE_VERSION = "ui-performance-fixture-v1"
CANDIDATE_FIXTURE_VERSION = "ui-performance-large-library-drag-v1"
WARMUP_COUNT = 3
SAMPLE_COUNT = 12
SCENARIOS = (
    "navigation-hover-v1",
    "document-list-interaction-v1",
    "deck-drag-v1",
    "deck-selection-v1",
    "study-guide-open-v1",
    "active-practice-progress-v1",
)
CANDIDATE_SCENARIOS = ("deck-drag-v1",)
LEGACY_DRAG_POINTER_ID = "stage0-touch"
CANDIDATE_DRAG_POINTER_ID = 4_004
CANDIDATE_DRAG_ACTIVATION_OFFSET = {"x": 9, "y": 0}
TRUSTED_NATIVE_INPUT_ATTESTATION = {
    "schemaVersion": 1,
    "method": "x11-xtest-v1",
    "trustedPointerEvents": ["pointerdown", "pointermove", "pointerup"],
    "trustedEscapeObserved": True,
    "previewAppeared": True,
    "previewCleared": True,
    "libraryStructureUnchanged": True,
}
PLATFORM_PATTERN = re.compile(
    r"\A(?P<os>Linux)-"
    r"(?P<kernel_major>0|[1-9][0-9]*)\."
    r"(?P<kernel_minor>0|[1-9][0-9]*)\."
    r"(?P<kernel_patch>0|[1-9][0-9]*)"
    r"(?P<kernel_suffix>(?:-[A-Za-z0-9][A-Za-z0-9._+~]*)*)-"
    r"(?P<architecture>x86_64|aarch64)-with-"
    r"(?P<libc_family>glibc)"
    r"(?P<libc_major>0|[1-9][0-9]*)\."
    r"(?P<libc_minor>0|[1-9][0-9]*)\Z",
    re.ASCII,
)
PYTHON_PATTERN = re.compile(
    r"\A(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\Z",
    re.ASCII,
)
POINTER_SCENARIOS = {"navigation-hover-v1", "deck-drag-v1"}
BOUNDARIES = {"navigation", "documents", "deck-library", "practice-setup", "guide-library", "guide-dialog", "active-practice"}
EXPECTED_BOUNDARIES = {
    "navigation-hover-v1": {"navigation"},
    "document-list-interaction-v1": {"documents"},
    "deck-drag-v1": {"deck-library"},
    "deck-selection-v1": {"practice-setup"},
    "study-guide-open-v1": {"guide-library", "guide-dialog"},
    "active-practice-progress-v1": {"active-practice"},
}
VIEWPORT = {"width": 1440, "height": 900}
DISPLAY = {"width": 1920, "height": 1200, "depth": 24, "dpi": 96}
PRACTICE_DECK_CHECKBOX_XPATH = (
    "//main//h2[normalize-space(.)='Select decks to study']"
    "/following::input[@type='checkbox'][1]"
)
PRACTICE_SELECT_ALL_XPATH = (
    "//main//h2[normalize-space(.)='Select decks to study']"
    "/following::button[normalize-space(.)='Select all'][1]"
)
PRACTICE_START_XPATH = "//main//button[normalize-space(.)='Start scheduled practice']"
PRACTICE_SETUP_STATE_KEYS = {
    "targetFound", "eligibleDeckCount", "selectedDeckCount", "targetChecked",
    "workloadPhase", "startFound", "startEnabled", "workloadTransitionObserved",
}
PRACTICE_WORKLOAD_PHASES = {"unavailable", "idle", "empty", "updating", "error", "settled"}
ROADMAP_PATH = ROOT_DIR / "docs/development/ui-workflow-redesign-roadmap.md"
LARGE_LIBRARY_DRAG_XPATH = (
    "//main//*[@data-library-row='deck' and "
    "@data-item-name='Large library drag source']"
)
MEASUREMENT_KEYS = {
    "schemaVersion", "baselineId", "sourceSha", "instrumentationVersion",
    "fixtureVersion", "scenarioId", "scenarioVersion", "samples",
}
MEASUREMENT_SENSITIVE_PATHS = (
    "package.json", "pnpm-lock.yaml", "vite.config.ts", "src/main.tsx",
    "src/testing/uiPerformanceBridge.ts", "src/testing/uiPerformanceInstrumentation.ts",
    "src/testing/PerformanceProfiler.tsx", "src/App.tsx",
    "src/screens/flashcards/DeckLibraryWorkspace.tsx", "src/screens/PracticeScreen.tsx",
    "src/screens/StudyGuideLibrarySection.tsx",
    "src-tauri/src/library_backup/extended_tests.rs",
    "src-tauri/src/library_backup/performance_fixture_tests.rs",
    "scripts/ui-performance-baselines.sh", "tests/webdriver/smoke.py",
    "tests/webdriver/ui_performance_baseline.py",
)
CANDIDATE_SENSITIVE_PATHS = (
    *MEASUREMENT_SENSITIVE_PATHS,
    "src/screens/flashcards/deck-library/dragSessionModel.ts",
    "src/screens/flashcards/deck-library/pointerDragSession.ts",
    "src/screens/flashcards/deck-library/useDeckLibraryPointerSession.ts",
    "src/screens/flashcards/deck-library/useDeckLibraryReorder.ts",
    "src/screens/flashcards/deck-library/DeckItem.tsx",
    "src/screens/flashcards/deck-library/FolderSection.tsx",
    "src/screens/flashcards/deck-library/DeckLibraryView.tsx",
    "src/screens/flashcards/deck-library/LibraryDragFeedback.tsx",
)


class PerformanceBaselineError(ValueError):
    """Committed or candidate performance evidence violates the strict contract."""


class BrowserPerformanceError(AssertionError):
    """The capture bridge rejected asynchronous measurement finalization."""

    def __init__(self, name: str, message: str) -> None:
        self.name = name
        self.message = message
        super().__init__(f"Browser performance finish failed: {name}: {message}")


def fail(message: str) -> NoReturn:
    raise PerformanceBaselineError(message)


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            fail(f"Duplicate JSON key {key!r}.")
        result[key] = value
    return result


def load_json(path: Path) -> dict[str, Any]:
    require_regular_file(path, "performance evidence file")
    try:
        value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=reject_duplicate_keys)
    except json.JSONDecodeError as error:
        fail(f"Malformed JSON in {path}: {error}")
    if not isinstance(value, dict):
        fail(f"{path} must contain one JSON object.")
    return value


def exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        fail(f"{label} keys differ; missing={sorted(expected-actual)!r}, extra={sorted(actual-expected)!r}.")


def strict_json_equal(actual: Any, expected: Any) -> bool:
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, dict):
        return set(actual) == set(expected) and all(
            strict_json_equal(actual[key], value) for key, value in expected.items()
        )
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(
            strict_json_equal(actual_value, expected_value)
            for actual_value, expected_value in zip(actual, expected, strict=True)
        )
    return actual == expected


def require_regular_file(path: Path, label: str) -> None:
    try:
        mode = path.stat(follow_symlinks=False).st_mode
    except FileNotFoundError:
        fail(f"Missing {label}: {path}")
    if path.is_symlink() or not stat.S_ISREG(mode):
        fail(f"{label} must be a regular non-symlink file: {path}")


def evidence_file_set(root: Path, label: str) -> set[str]:
    if root.is_symlink() or not root.is_dir():
        fail(f"{label} root must be a real directory.")
    files: set[str] = set()
    for path in root.rglob("*"):
        relative = str(path.relative_to(root))
        if path.is_symlink():
            fail(f"{label} contains a symlink: {relative}.")
        mode = path.stat(follow_symlinks=False).st_mode
        if stat.S_ISDIR(mode):
            continue
        if not stat.S_ISREG(mode):
            fail(f"{label} contains a non-regular entry: {relative}.")
        files.add(relative)
    return files


def finite(value: Any, label: str, *, integer: bool = False) -> float | int:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        fail(f"{label} must be numeric.")
    if not math.isfinite(float(value)) or value < 0 or (integer and not isinstance(value, int)):
        fail(f"{label} must be finite and nonnegative{' integer' if integer else ''}.")
    return value


def sha256_file(path: Path) -> str:
    require_regular_file(path, "file to hash")
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def canonical_measurement_path(scenario: str) -> str:
    return f"measurements/{scenario}.json"


def current_source_sha() -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=ROOT_DIR, check=True,
        text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    )
    sha = result.stdout.strip()
    if re.fullmatch(r"[0-9a-f]{40}", sha) is None:
        fail(f"Git returned an invalid source SHA: {sha!r}.")
    return sha


def assert_clean_committed_head(source_sha: str) -> None:
    if current_source_sha() != source_sha:
        fail("Candidate source identity must equal the current committed HEAD.")
    status = subprocess.run(
        ["git", "status", "--porcelain", "--untracked-files=all"],
        cwd=ROOT_DIR,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if status.stdout:
        fail("Candidate capture requires a clean committed HEAD.")


def assert_measurement_sources_unchanged(
    source_sha: str,
    sensitive_paths: tuple[str, ...] = MEASUREMENT_SENSITIVE_PATHS,
) -> None:
    if re.fullmatch(r"[0-9a-f]{40}", source_sha) is None:
        fail(f"manifest.sourceSha is not a Git commit SHA: {source_sha!r}.")
    for relative in sensitive_paths:
        committed = subprocess.run(
            ["git", "show", f"{source_sha}:{relative}"], cwd=ROOT_DIR, check=False,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )
        if committed.returncode != 0:
            fail(f"Measurement source {relative} is unavailable at {source_sha}.")
        try:
            current = (ROOT_DIR / relative).read_bytes()
        except FileNotFoundError:
            fail(f"Measurement source {relative} is missing from the current tree.")
        if current != committed.stdout:
            fail(f"Measurement-sensitive source drifted after capture: {relative}.")


def verify_sample(sample: Any, scenario: str, ordinal: int) -> None:
    if not isinstance(sample, dict):
        fail(f"{scenario} sample {ordinal} must be an object.")
    exact_keys(sample, {"ordinal", "interactionToVisible", "interactionToReady", "userTiming", "react", "pointer"}, f"{scenario} sample {ordinal}")
    if type(sample["ordinal"]) is not int or sample["ordinal"] != ordinal:
        fail(f"{scenario} has stale or duplicate sample ordinal {sample['ordinal']!r}; expected {ordinal}.")
    for metric in ("interactionToVisible", "interactionToReady"):
        value = sample[metric]
        if not isinstance(value, dict):
            fail(f"{scenario}.{metric} must be an object.")
        exact_keys(value, {"state", "valueMs"}, f"{scenario}.{metric}")
        if value["state"] != "available":
            fail(f"{scenario}.{metric} is required and must be available.")
        finite(value["valueMs"], f"{scenario}.{metric}.valueMs")
    timing = sample["userTiming"]
    if not isinstance(timing, dict): fail(f"{scenario}.userTiming must be an object.")
    exact_keys(timing, {"startTimeMs", "visibleTimeMs", "readyTimeMs"}, f"{scenario}.userTiming")
    for key in timing: finite(timing[key], f"{scenario}.userTiming.{key}")
    if not timing["startTimeMs"] <= timing["visibleTimeMs"] <= timing["readyTimeMs"]:
        fail(f"{scenario} User Timing marks are out of order.")
    if not math.isclose(float(sample["interactionToVisible"]["valueMs"]), float(timing["visibleTimeMs"] - timing["startTimeMs"]), abs_tol=0.001):
        fail(f"{scenario} visible duration disagrees with its User Timing marks.")
    if not math.isclose(float(sample["interactionToReady"]["valueMs"]), float(timing["readyTimeMs"] - timing["startTimeMs"]), abs_tol=0.001):
        fail(f"{scenario} ready duration disagrees with its User Timing marks.")
    react = sample["react"]
    if not isinstance(react, dict): fail(f"{scenario}.react must be an object.")
    exact_keys(react, {"state", "commitCount", "totalRenderDurationMs", "avoidableCommitCount", "commits"}, f"{scenario}.react")
    if react["state"] != "available": fail(f"{scenario} React Profiler evidence is required.")
    finite(react["commitCount"], f"{scenario}.react.commitCount", integer=True)
    finite(react["avoidableCommitCount"], f"{scenario}.react.avoidableCommitCount", integer=True)
    finite(react["totalRenderDurationMs"], f"{scenario}.react.totalRenderDurationMs")
    if not isinstance(react["commits"], list): fail(f"{scenario}.react.commits must be an array.")
    if len(react["commits"]) > 512: fail(f"{scenario}.react.commits exceeds the bounded capacity.")
    commit_times: set[float] = set()
    avoidable_times: set[float] = set()
    render_total = 0.0
    for index, commit in enumerate(react["commits"]):
        if not isinstance(commit, dict): fail(f"{scenario}.react.commits[{index}] must be an object.")
        exact_keys(commit, {"boundaryId", "phase", "actualDurationMs", "baseDurationMs", "startTimeMs", "commitTimeMs"}, f"{scenario}.react.commits[{index}]")
        if commit["boundaryId"] not in BOUNDARIES or commit["phase"] not in {"mount", "update", "nested-update"}:
            fail(f"{scenario}.react.commits[{index}] has an unknown boundary or phase.")
        for key in ("actualDurationMs", "baseDurationMs", "startTimeMs", "commitTimeMs"):
            finite(commit[key], f"{scenario}.react.commits[{index}].{key}")
        commit_time = float(commit["commitTimeMs"]); commit_times.add(commit_time)
        if commit["boundaryId"] not in EXPECTED_BOUNDARIES[scenario]: avoidable_times.add(commit_time)
        render_total += float(commit["actualDurationMs"])
    if react["commitCount"] != len(commit_times) or react["avoidableCommitCount"] != len(avoidable_times):
        fail(f"{scenario} React commit aggregates disagree with the commit evidence.")
    if not math.isclose(float(react["totalRenderDurationMs"]), render_total, abs_tol=0.001):
        fail(f"{scenario} React render duration disagrees with the commit evidence.")
    pointer = sample["pointer"]
    if not isinstance(pointer, dict): fail(f"{scenario}.pointer must be an object.")
    exact_keys(pointer, {"state", "eventCount", "frameCount", "maxFrameWorkMs", "maxFrameIntervalMs"}, f"{scenario}.pointer")
    expected_state = "available" if scenario in POINTER_SCENARIOS else "notApplicable"
    if pointer["state"] != expected_state:
        fail(f"{scenario}.pointer must be {expected_state!r}, received {pointer['state']!r}.")
    for key in ("eventCount", "frameCount", "maxFrameWorkMs", "maxFrameIntervalMs"):
        value = pointer[key]
        if expected_state == "notApplicable":
            if value is not None: fail(f"{scenario}.pointer.{key} must be null when not applicable.")
        else:
            finite(value, f"{scenario}.pointer.{key}", integer=key in {"eventCount", "frameCount"})


def verify_measurement(
    path: Path,
    scenario: str,
    source_sha: str,
    *,
    evidence_id: str = BASELINE_ID,
    fixture_version: str = FIXTURE_VERSION,
) -> dict[str, Any]:
    measurement = load_json(path)
    exact_keys(measurement, MEASUREMENT_KEYS, path.name)
    expected = {
        "schemaVersion": SCHEMA_VERSION, "baselineId": evidence_id,
        "sourceSha": source_sha, "instrumentationVersion": INSTRUMENTATION_VERSION,
        "fixtureVersion": fixture_version, "scenarioId": scenario, "scenarioVersion": scenario,
    }
    for key, value in expected.items():
        if not strict_json_equal(measurement[key], value):
            fail(f"{path.name}.{key} drifted: expected {value!r}, received {measurement[key]!r}.")
    samples = measurement["samples"]
    if not isinstance(samples, list) or len(samples) != SAMPLE_COUNT:
        fail(f"{scenario} must contain exactly {SAMPLE_COUNT} recorded samples.")
    for ordinal, sample in enumerate(samples, 1): verify_sample(sample, scenario, ordinal)
    return measurement


def expected_budget_entries(measurements: dict[str, dict[str, Any]]) -> list[dict[str, Any]]:
    entries: list[dict[str, Any]] = []
    refresh_values = [
        float(sample["pointer"]["maxFrameIntervalMs"])
        for scenario in POINTER_SCENARIOS for sample in measurements[scenario]["samples"]
    ]
    refresh_interval = round(statistics.median(refresh_values), 3)
    for scenario in SCENARIOS:
        commits = [float(sample["react"]["avoidableCommitCount"]) for sample in measurements[scenario]["samples"]]
        entries.append({
            "scenarioId": scenario, "metric": "avoidableReactCommitCount", "operator": "lessThanOrEqual",
            "candidateStatistic": "maximum", "budget": int(max(commits)),
            "formula": {"kind": "observedMaximum", "observedMaximum": int(max(commits)), "headroom": 0},
            "rationale": "Discrete unexpected commits receive no noise headroom at Stage 0.",
        })
        if scenario in POINTER_SCENARIOS:
            visible = [float(sample["interactionToVisible"]["valueMs"]) for sample in measurements[scenario]["samples"]]
            work = [float(sample["pointer"]["maxFrameWorkMs"]) for sample in measurements[scenario]["samples"]]
            visible_budget = round(max(visible) + refresh_interval, 3)
            work_stdev = statistics.stdev(work)
            work_budget = round(max(work) + work_stdev, 3)
            entries.extend([
                {
                    "scenarioId": scenario, "metric": "interactionToVisibleMs", "operator": "lessThanOrEqual",
                    "candidateStatistic": "maximum", "budget": visible_budget,
                    "formula": {"kind": "observedMaximumPlusRefreshInterval", "observedMaximum": round(max(visible), 3), "headroom": refresh_interval},
                    "rationale": "Hover and drag visible feedback allow one measured refresh interval of capture noise.",
                },
                {
                    "scenarioId": scenario, "metric": "maxPointerFrameWorkMs", "operator": "lessThanOrEqual",
                    "candidateStatistic": "maximum", "budget": work_budget,
                    "formula": {"kind": "observedMaximumPlusSampleStandardDeviation", "observedMaximum": round(max(work), 3), "headroom": round(work_stdev, 3)},
                    "rationale": "Pointer work allows one sample standard deviation above the observed maximum.",
                },
            ])
    return entries


def verify_baseline(baseline_dir: Path = BASELINE_DIR, *, expected_source_sha: str | None = None) -> dict[str, Any]:
    if baseline_dir.is_symlink():
        fail("Baseline root must not be a symlink.")
    baseline_dir = baseline_dir.resolve()
    manifest = load_json(baseline_dir / "manifest.json")
    exact_keys(manifest, {"schemaVersion", "baselineId", "sourceSha", "instrumentationVersion", "fixtureVersion", "warmupCount", "sampleCount", "scenarioVersions", "captureEnvironment", "measurements", "budgets"}, "manifest")
    source_sha = manifest["sourceSha"]
    if not isinstance(source_sha, str) or re.fullmatch(r"[0-9a-f]{40}", source_sha) is None:
        fail(f"manifest.sourceSha is not a Git commit SHA: {source_sha!r}.")
    if expected_source_sha is not None:
        if source_sha != expected_source_sha:
            fail(f"manifest.sourceSha drifted: expected {expected_source_sha!r}, received {source_sha!r}.")
    else:
        assert_measurement_sources_unchanged(source_sha)
    expected_identity = {
        "schemaVersion": SCHEMA_VERSION, "baselineId": BASELINE_ID, "sourceSha": source_sha,
        "instrumentationVersion": INSTRUMENTATION_VERSION, "fixtureVersion": FIXTURE_VERSION,
        "warmupCount": WARMUP_COUNT, "sampleCount": SAMPLE_COUNT,
    }
    for key, value in expected_identity.items():
        if not strict_json_equal(manifest[key], value):
            fail(f"manifest.{key} drifted: expected {value!r}, received {manifest[key]!r}.")
    if not strict_json_equal(
        manifest["scenarioVersions"],
        {scenario: scenario for scenario in SCENARIOS},
    ):
        fail("Manifest scenario versions are missing, duplicated, or stale.")
    environment = manifest["captureEnvironment"]
    if not isinstance(environment, dict): fail("captureEnvironment must be an object.")
    exact_keys(environment, {"viewport", "theme", "scalePercent", "motion", "materials", "pointer", "devicePixelRatio", "display", "applicationSha256", "python", "platform", "tauriDriver", "webkitWebDriver", "xvfb", "reactProfilingBuild"}, "captureEnvironment")
    if not strict_json_equal(environment["viewport"], VIEWPORT) or not strict_json_equal(
        environment["display"],
        DISPLAY,
    ):
        fail("Capture viewport/display metadata drifted from the reference contract.")
    if not strict_json_equal(
        {
            key: environment[key]
            for key in (
                "theme", "scalePercent", "motion", "materials",
                "pointer", "reactProfilingBuild",
            )
        },
        {
            "theme": "light", "scalePercent": 100, "motion": "full",
            "materials": "rich", "pointer": "fine", "reactProfilingBuild": True,
        },
    ):
        fail("Capture runtime metadata drifted from the fixed reference contract.")
    finite(environment["devicePixelRatio"], "captureEnvironment.devicePixelRatio")
    if not isinstance(environment["applicationSha256"], str) or re.fullmatch(
        r"[0-9a-f]{64}",
        environment["applicationSha256"],
    ) is None:
        fail("captureEnvironment.applicationSha256 must be a SHA-256 digest.")
    entries = manifest["measurements"]
    if not isinstance(entries, list) or len(entries) != len(SCENARIOS): fail("Manifest must reference exactly six measurements.")
    measurements: dict[str, dict[str, Any]] = {}
    seen_paths: set[str] = set()
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict): fail(f"measurements[{index}] must be an object.")
        exact_keys(entry, {"scenarioId", "path", "sha256"}, f"measurements[{index}]")
        scenario = entry["scenarioId"]
        if scenario not in SCENARIOS or scenario in measurements: fail(f"Duplicate or unknown scenario entry {scenario!r}.")
        expected_path = canonical_measurement_path(scenario)
        if entry["path"] != expected_path or Path(entry["path"]).is_absolute() or ".." in Path(entry["path"]).parts:
            fail(f"Noncanonical measurement path for {scenario}: {entry['path']!r}.")
        if entry["path"] in seen_paths: fail(f"Duplicate measurement path {entry['path']!r}.")
        seen_paths.add(entry["path"])
        path = baseline_dir / entry["path"]
        if sha256_file(path) != entry["sha256"]: fail(f"Measurement hash mismatch for {entry['path']}.")
        measurements[scenario] = verify_measurement(path, scenario, source_sha)
    if set(measurements) != set(SCENARIOS): fail("One or more required scenarios are missing.")
    budgets_ref = manifest["budgets"]
    if not isinstance(budgets_ref, dict): fail("manifest.budgets must be an object.")
    exact_keys(budgets_ref, {"path", "sha256"}, "manifest.budgets")
    if budgets_ref["path"] != "budgets.json": fail("Budget path must be budgets.json.")
    budgets_path = baseline_dir / "budgets.json"
    if sha256_file(budgets_path) != budgets_ref["sha256"]: fail("Budget hash mismatch.")
    budgets = load_json(budgets_path)
    exact_keys(budgets, {"schemaVersion", "baselineId", "sourceSha", "instrumentationVersion", "scenarioVersions", "rounding", "entries"}, "budgets")
    if not all(
        strict_json_equal(budgets[key], value)
        for key, value in {
            "schemaVersion": SCHEMA_VERSION,
            "baselineId": BASELINE_ID,
            "sourceSha": source_sha,
            "instrumentationVersion": INSTRUMENTATION_VERSION,
        }.items()
    ):
        fail("Budget identity is stale.")
    if not strict_json_equal(
        budgets["scenarioVersions"],
        manifest["scenarioVersions"],
    ) or not strict_json_equal(
        budgets["rounding"],
        "milliseconds-to-3-decimal-places",
    ):
        fail("Budget scenario or rounding contract drifted.")
    expected_entries = expected_budget_entries(measurements)
    if not strict_json_equal(budgets["entries"], expected_entries):
        fail("Budgets are stale or were not derived exactly from the baseline distribution.")
    expected_files = {"manifest.json", "budgets.json", *(canonical_measurement_path(s) for s in SCENARIOS)}
    actual_files = evidence_file_set(baseline_dir, "Baseline evidence")
    if actual_files != expected_files: fail(f"Baseline file set differs; missing={sorted(expected_files-actual_files)!r}, extra={sorted(actual_files-expected_files)!r}.")
    return {"manifest": manifest, "measurements": measurements, "budgets": budgets}


def verify_reference(baseline_dir: Path = BASELINE_DIR) -> dict[str, Any]:
    """Structurally verify the immutable reference at its recorded source identity."""
    manifest = load_json(baseline_dir / "manifest.json")
    return verify_baseline(baseline_dir, expected_source_sha=manifest.get("sourceSha"))


def verify_trusted_native_input_attestation(value: Any) -> None:
    if not isinstance(value, dict):
        fail("Candidate trusted native input attestation must be an object.")
    exact_keys(value, set(TRUSTED_NATIVE_INPUT_ATTESTATION), "trustedNativeInput")
    if type(value["schemaVersion"]) is not int or value["schemaVersion"] != 1:
        fail("trustedNativeInput.schemaVersion must be integer 1.")
    if type(value["method"]) is not str or value["method"] != "x11-xtest-v1":
        fail("trustedNativeInput.method is invalid.")
    events = value["trustedPointerEvents"]
    if (
        not isinstance(events, list)
        or any(type(event) is not str for event in events)
        or events != ["pointerdown", "pointermove", "pointerup"]
    ):
        fail("trustedNativeInput.trustedPointerEvents is invalid.")
    for key in (
        "trustedEscapeObserved", "previewAppeared",
        "previewCleared", "libraryStructureUnchanged",
    ):
        if type(value[key]) is not bool or value[key] is not True:
            fail(f"trustedNativeInput.{key} must be boolean true.")


def verify_drag_candidate(
    candidate_dir: Path,
    *,
    expected_source_sha: str | None = None,
) -> dict[str, Any]:
    if candidate_dir.is_symlink():
        fail("Candidate root must not be a symlink.")
    candidate_dir = candidate_dir.resolve()
    manifest = load_json(candidate_dir / "manifest.json")
    exact_keys(
        manifest,
        {
            "schemaVersion", "baselineId", "sourceSha", "instrumentationVersion",
            "fixtureVersion", "warmupCount", "sampleCount", "scenarioVersions",
            "captureEnvironment", "measurements", "trustedNativeInput",
        },
        "candidate manifest",
    )
    source_sha = manifest["sourceSha"]
    if not isinstance(source_sha, str) or re.fullmatch(r"[0-9a-f]{40}", source_sha) is None:
        fail(f"candidate manifest.sourceSha is not a Git commit SHA: {source_sha!r}.")
    if expected_source_sha is None:
        if source_sha != current_source_sha():
            fail("Candidate source identity must equal the current committed HEAD.")
        assert_measurement_sources_unchanged(source_sha, CANDIDATE_SENSITIVE_PATHS)
    elif source_sha != expected_source_sha:
        fail(
            "candidate manifest.sourceSha drifted: "
            f"expected {expected_source_sha!r}, received {source_sha!r}."
        )
    expected_identity = {
        "schemaVersion": SCHEMA_VERSION,
        "baselineId": CANDIDATE_ID,
        "sourceSha": source_sha,
        "instrumentationVersion": INSTRUMENTATION_VERSION,
        "fixtureVersion": CANDIDATE_FIXTURE_VERSION,
        "warmupCount": WARMUP_COUNT,
        "sampleCount": SAMPLE_COUNT,
    }
    for key, value in expected_identity.items():
        if not strict_json_equal(manifest[key], value):
            fail(
                f"candidate manifest.{key} drifted: "
                f"expected {value!r}, received {manifest[key]!r}."
            )
    scenario_versions = {scenario: scenario for scenario in CANDIDATE_SCENARIOS}
    if not strict_json_equal(manifest["scenarioVersions"], scenario_versions):
        fail("Candidate must contain only the deck-drag-v1 scenario identity.")
    verify_trusted_native_input_attestation(manifest["trustedNativeInput"])

    environment = manifest["captureEnvironment"]
    if not isinstance(environment, dict):
        fail("candidate captureEnvironment must be an object.")
    exact_keys(
        environment,
        {
            "viewport", "theme", "scalePercent", "motion", "materials", "pointer",
            "devicePixelRatio", "display", "applicationSha256", "python", "platform",
            "tauriDriver", "webkitWebDriver", "xvfb", "reactProfilingBuild",
        },
        "candidate captureEnvironment",
    )
    if not strict_json_equal(environment["viewport"], VIEWPORT) or not strict_json_equal(
        environment["display"],
        DISPLAY,
    ):
        fail("Candidate viewport/display metadata drifted from the reference contract.")
    if not strict_json_equal(
        {
            key: environment[key]
            for key in (
                "theme", "scalePercent", "motion", "materials",
                "pointer", "reactProfilingBuild",
            )
        },
        {
            "theme": "light", "scalePercent": 100, "motion": "full",
            "materials": "rich", "pointer": "fine", "reactProfilingBuild": True,
        },
    ):
        fail("Candidate runtime metadata drifted from the fixed reference contract.")
    finite(environment["devicePixelRatio"], "candidate captureEnvironment.devicePixelRatio")
    if not isinstance(environment["applicationSha256"], str) or re.fullmatch(
        r"[0-9a-f]{64}",
        environment["applicationSha256"],
    ) is None:
        fail("candidate captureEnvironment.applicationSha256 must be a SHA-256 digest.")

    entries = manifest["measurements"]
    if not isinstance(entries, list) or len(entries) != 1:
        fail("Candidate manifest must reference exactly one deck-drag-v1 measurement.")
    entry = entries[0]
    if not isinstance(entry, dict):
        fail("candidate measurements[0] must be an object.")
    exact_keys(entry, {"scenarioId", "path", "sha256"}, "candidate measurements[0]")
    scenario = CANDIDATE_SCENARIOS[0]
    expected_path = canonical_measurement_path(scenario)
    if entry["scenarioId"] != scenario or entry["path"] != expected_path:
        fail("Candidate measurement must be the canonical deck-drag-v1 path.")
    path = candidate_dir / expected_path
    if sha256_file(path) != entry["sha256"]:
        fail("Candidate deck-drag-v1 measurement hash mismatch.")
    measurement = verify_measurement(
        path,
        scenario,
        source_sha,
        evidence_id=CANDIDATE_ID,
        fixture_version=CANDIDATE_FIXTURE_VERSION,
    )
    expected_files = {"manifest.json", expected_path}
    actual_files = evidence_file_set(candidate_dir, "Candidate evidence")
    if actual_files != expected_files:
        fail(
            "Candidate file set differs; "
            f"missing={sorted(expected_files-actual_files)!r}, "
            f"extra={sorted(actual_files-expected_files)!r}."
        )
    return {"manifest": manifest, "measurements": {scenario: measurement}}


def metric_values(measurement: dict[str, Any], metric: str) -> list[float]:
    if metric == "avoidableReactCommitCount": return [float(s["react"]["avoidableCommitCount"]) for s in measurement["samples"]]
    if metric == "interactionToVisibleMs": return [float(s["interactionToVisible"]["valueMs"]) for s in measurement["samples"]]
    if metric == "maxPointerFrameWorkMs": return [float(s["pointer"]["maxFrameWorkMs"]) for s in measurement["samples"]]
    fail(f"Unknown budget metric {metric!r}.")


def normalized_platform_identity(value: Any) -> dict[str, str | int]:
    """Parse the one supported Linux platform form into stable ABI fields."""
    if not isinstance(value, str):
        fail("captureEnvironment.platform must be a raw platform string.")
    match = PLATFORM_PATTERN.fullmatch(value)
    if match is None:
        fail(f"captureEnvironment.platform is not a canonical supported Linux identity: {value!r}.")
    return {
        "osFamily": match.group("os").lower(),
        "kernelMajor": int(match.group("kernel_major")),
        "kernelMinor": int(match.group("kernel_minor")),
        "architecture": match.group("architecture"),
        "libcFamily": match.group("libc_family"),
        "libcMajor": int(match.group("libc_major")),
        "libcMinor": int(match.group("libc_minor")),
    }


def normalized_python_identity(value: Any) -> tuple[int, int]:
    """Normalize platform.python_version() to its compatibility major/minor pair."""
    if not isinstance(value, str):
        fail("captureEnvironment.python must be a raw Python version string.")
    match = PYTHON_PATTERN.fullmatch(value)
    if match is None:
        fail(f"captureEnvironment.python has no normalized major/minor identity: {value!r}.")
    return int(match.group(1)), int(match.group(2))


def assert_compatible_capture_environments(
    baseline_environment: dict[str, Any],
    candidate_environment: dict[str, Any],
) -> None:
    exact_keys_to_compare = (
        "viewport", "theme", "scalePercent", "motion", "materials", "pointer",
        "devicePixelRatio", "display", "tauriDriver", "webkitWebDriver", "xvfb",
        "reactProfilingBuild",
    )
    for key in exact_keys_to_compare:
        if candidate_environment[key] != baseline_environment[key]:
            fail(f"Candidate runtime metadata is not comparable: {key} differs.")

    try:
        baseline_platform = normalized_platform_identity(baseline_environment["platform"])
        candidate_platform = normalized_platform_identity(candidate_environment["platform"])
    except PerformanceBaselineError as error:
        fail(f"Candidate runtime metadata is not comparable: {error}")
    for key in (
        "osFamily", "kernelMajor", "kernelMinor", "architecture",
        "libcFamily", "libcMajor", "libcMinor",
    ):
        if candidate_platform[key] != baseline_platform[key]:
            fail(f"Candidate runtime metadata is not comparable: normalized platform {key} differs.")
    try:
        python_compatible = (
            normalized_python_identity(candidate_environment["python"])
            == normalized_python_identity(baseline_environment["python"])
        )
    except PerformanceBaselineError as error:
        fail(f"Candidate runtime metadata is not comparable: {error}")
    if not python_compatible:
        fail("Candidate runtime metadata is not comparable: Python major/minor differs.")


def compare_baseline(candidate_dir: Path, baseline_dir: Path = BASELINE_DIR, *, expected_source_sha: str | None = None) -> list[str]:
    if expected_source_sha is not None:
        baseline = verify_baseline(baseline_dir, expected_source_sha=expected_source_sha)
        candidate = verify_baseline(candidate_dir, expected_source_sha=expected_source_sha)
    else:
        baseline_manifest = load_json(baseline_dir / "manifest.json")
        baseline = verify_baseline(baseline_dir, expected_source_sha=baseline_manifest.get("sourceSha"))
        candidate = verify_baseline(candidate_dir)
    base_env = baseline["manifest"]["captureEnvironment"]
    candidate_env = candidate["manifest"]["captureEnvironment"]
    assert_compatible_capture_environments(base_env, candidate_env)
    failures: list[str] = []
    for entry in baseline["budgets"]["entries"]:
        value = max(metric_values(candidate["measurements"][entry["scenarioId"]], entry["metric"]))
        if value > float(entry["budget"]):
            failures.append(f"{entry['scenarioId']}:{entry['metric']} maximum {value:.3f} exceeds budget {float(entry['budget']):.3f}")
    return failures


def compare_drag_candidate(
    candidate_dir: Path,
    baseline_dir: Path = BASELINE_DIR,
    *,
    expected_candidate_source_sha: str | None = None,
) -> list[str]:
    baseline = verify_reference(baseline_dir)
    candidate = verify_drag_candidate(
        candidate_dir,
        expected_source_sha=expected_candidate_source_sha,
    )
    base_env = baseline["manifest"]["captureEnvironment"]
    candidate_env = candidate["manifest"]["captureEnvironment"]
    assert_compatible_capture_environments(base_env, candidate_env)
    drag_budgets = [
        entry
        for entry in baseline["budgets"]["entries"]
        if entry["scenarioId"] == CANDIDATE_SCENARIOS[0]
    ]
    expected_metrics = {
        "avoidableReactCommitCount",
        "interactionToVisibleMs",
        "maxPointerFrameWorkMs",
    }
    if len(drag_budgets) != 3 or {entry["metric"] for entry in drag_budgets} != expected_metrics:
        fail("Committed reference must expose exactly the three deck-drag-v1 limits.")
    measurement = candidate["measurements"][CANDIDATE_SCENARIOS[0]]
    failures: list[str] = []
    for entry in drag_budgets:
        value = max(metric_values(measurement, entry["metric"]))
        if value > float(entry["budget"]):
            failures.append(
                f"{entry['scenarioId']}:{entry['metric']} maximum {value:.3f} "
                f"exceeds budget {float(entry['budget']):.3f}"
            )
    return failures


def validate_candidate_destination(destination: Path) -> Path:
    if not destination.is_absolute():
        fail("Candidate destination must be an explicit absolute path.")
    if destination.exists() or destination.is_symlink():
        fail("Candidate destination must not already exist.")
    resolved_parent = destination.parent.resolve(strict=True)
    resolved = resolved_parent / destination.name
    repository = ROOT_DIR.resolve()
    reference = BASELINE_DIR.resolve()
    if resolved == reference or reference in resolved.parents:
        fail("Candidate destination must not alias the committed reference tree.")
    if resolved == repository or repository in resolved.parents:
        fail("Candidate destination must be outside the repository.")
    if destination.parent != resolved_parent:
        fail("Candidate destination must not use a symlinked parent alias.")
    return resolved


def install_staged_candidate(staging: Path, destination: Path) -> None:
    destination = validate_candidate_destination(destination)
    if staging.parent.resolve() != destination.parent:
        fail("Candidate staging must be a sibling of the destination.")
    libc = ctypes.CDLL(None, use_errno=True)
    renameat2 = getattr(libc, "renameat2", None)
    if renameat2 is None:
        fail("Atomic no-replace installation is unavailable on this platform.")
    renameat2.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    renameat2.restype = ctypes.c_int
    result = renameat2(
        -100,
        os.fsencode(staging),
        -100,
        os.fsencode(destination),
        1,
    )
    if result == 0:
        return
    error_number = ctypes.get_errno()
    if error_number in {errno.EEXIST, errno.ENOTEMPTY}:
        fail("Candidate destination appeared during capture; no files were overwritten.")
    raise OSError(error_number, os.strerror(error_number), destination)


def execute(session_id: str, script: str, args: list[Any] | None = None) -> Any:
    return webdriver.request("POST", f"/session/{session_id}/execute/sync", {"script": script, "args": args or []})


def retry_idempotent_transport_once(operation: Callable[[], Any]) -> Any:
    """Retry one read-only, browser-cached, or reset-safe operation after a transient disconnect."""
    try:
        return operation()
    except (OSError, urllib.error.URLError) as error:
        if not is_transient_disconnect(error):
            raise
    return operation()


def execute_read_only(session_id: str, script: str, args: list[Any] | None = None) -> Any:
    return retry_idempotent_transport_once(lambda: execute(session_id, script, args))


def run_version(command: list[str], fallback: str) -> str:
    try:
        return subprocess.run(command, check=False, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT).stdout.strip() or fallback
    except OSError:
        return fallback


def arm(session_id: str, scenario: str, element_id: str) -> None:
    result = retry_idempotent_transport_once(
        lambda: execute(session_id, "const b=window.__OSTUDY_UI_PERFORMANCE__; if(!b) return false; b.arm(arguments[1], arguments[0]); return b.version;", [{webdriver.ELEMENT_KEY: element_id}, scenario])
    )
    if result != INSTRUMENTATION_VERSION: raise AssertionError(f"Performance bridge unavailable or stale: {result!r}")


def performance_sample_from_result(result: Any) -> dict[str, Any]:
    if not isinstance(result, dict):
        raise AssertionError(f"Invalid asynchronous performance result: {result!r}")
    if result.get("ok") is not True:
        error = result.get("error")
        if not isinstance(error, dict):
            raise AssertionError(f"Invalid asynchronous performance error: {result!r}")
        name = error.get("name")
        message = error.get("message")
        if not isinstance(name, str) or not isinstance(message, str):
            raise AssertionError(f"Invalid asynchronous performance error: {result!r}")
        raise BrowserPerformanceError(name, message)
    sample = result.get("sample")
    if not isinstance(sample, dict):
        raise AssertionError(f"Invalid performance sample: {sample!r}")
    return {
        "interactionToVisible": {"state": "available", "valueMs": sample["interactionToVisibleMs"]},
        "interactionToReady": {"state": "available", "valueMs": sample["interactionToReadyMs"]},
        "userTiming": sample["userTiming"],
        "react": {"state": "available", **sample["react"]},
        "pointer": sample["pointer"],
    }


def finish(session_id: str) -> dict[str, Any]:
    result = retry_idempotent_transport_once(
        lambda: webdriver.request(
            "POST",
            f"/session/{session_id}/execute/async",
            {
                "script": """
                  const done = arguments[arguments.length - 1];
                  const bridge = window.__OSTUDY_UI_PERFORMANCE__;
                  if (!bridge) {
                    done({ok: false, error: {name: 'BridgeUnavailableError', message: 'The UI performance bridge is unavailable.'}});
                    return;
                  }
                  Promise.resolve()
                    .then(() => bridge.finish())
                    .then(
                      (sample) => done({ok: true, sample}),
                      (error) => done({
                        ok: false,
                        error: {
                          name: error instanceof Error ? error.name : typeof error,
                          message: error instanceof Error ? error.message : String(error)
                        }
                      })
                    );
                """,
                "args": [],
            },
        )
    )
    return performance_sample_from_result(result)


def park_mouse(session_id: str) -> None:
    webdriver.request(
        "POST",
        f"/session/{session_id}/actions",
        {
            "actions": [{
                "type": "pointer",
                "id": "stage0-mouse",
                "parameters": {"pointerType": "mouse"},
                "actions": [{
                    "type": "pointerMove",
                    "duration": 0,
                    "origin": "viewport",
                    "x": VIEWPORT["width"] - 40,
                    "y": VIEWPORT["height"] - 40,
                }],
            }],
        },
    )


def release_pointer_actions(session_id: str) -> None:
    webdriver.request("DELETE", f"/session/{session_id}/actions")


def pointer_actions(
    session_id: str,
    element_id: str,
    *,
    release: bool = True,
) -> None:
    actions: list[dict[str, Any]] = [{"type": "pointerMove", "duration": 0, "origin": {webdriver.ELEMENT_KEY: element_id}, "x": 0, "y": 0}]
    webdriver.request("POST", f"/session/{session_id}/actions", {"actions": [{"type": "pointer", "id": "stage0-mouse", "parameters": {"pointerType": "mouse"}, "actions": actions}]})
    if release:
        release_pointer_actions(session_id)


def find_safe_drag_point(session_id: str, element_id: str) -> dict[str, int]:
    point = execute_read_only(
        session_id,
        """
        const row = arguments[0];
        const activationOffsetX = arguments[1];
        if (
          !(row instanceof HTMLElement) ||
          row.dataset.libraryRow !== 'deck' ||
          !Number.isInteger(activationOffsetX)
        ) return null;
        const surface = row.querySelector("[data-library-drag-surface='true']");
        if (!(surface instanceof HTMLElement)) return null;
        const rect = surface.getBoundingClientRect();
        const left = Math.max(0, Math.ceil(rect.left));
        const right = Math.min(
          window.innerWidth - 1 - Math.abs(activationOffsetX),
          Math.floor(rect.right) - Math.abs(activationOffsetX),
        );
        const top = Math.max(0, Math.ceil(rect.top));
        const bottom = Math.min(window.innerHeight - 1, Math.floor(rect.bottom));
        const canStartAt = (x, y) => {
          const target = document.elementFromPoint(x, y);
          return (
            target instanceof Element &&
            row.contains(target) &&
            target.closest("[data-library-drag-surface='true']") === surface
          );
        };
        for (let y = top; y <= bottom; y += 1) {
          for (let x = left; x <= right; x += 1) {
            if (canStartAt(x, y) && canStartAt(x + activationOffsetX, y)) {
              return {x, y};
            }
          }
        }
        return null;
        """,
        [
            {webdriver.ELEMENT_KEY: element_id},
            CANDIDATE_DRAG_ACTIVATION_OFFSET["x"],
        ],
    )
    if (
        not isinstance(point, dict)
        or set(point) != {"x", "y"}
        or not all(type(point[axis]) is int for axis in ("x", "y"))
    ):
        raise AssertionError("Deck drag source has no verified primary drag surface point.")
    return {"x": point["x"], "y": point["y"]}


class NativeX11Pointer:
    """Small fail-closed XTEST adapter for the isolated Xvfb qualification display."""

    def __init__(self) -> None:
        if os.environ.get("OSTUDY_PERFORMANCE_IN_XVFB") != "1":
            raise AssertionError("Trusted native pointer qualification requires isolated Xvfb.")
        if os.environ.get("GDK_BACKEND") != "x11":
            raise AssertionError("Trusted native pointer qualification requires GDK_BACKEND=x11.")
        display_name = os.environ.get("DISPLAY")
        if not display_name:
            raise AssertionError("Trusted native pointer qualification requires DISPLAY.")
        try:
            self._x11 = ctypes.CDLL("libX11.so.6")
            self._xtst = ctypes.CDLL("libXtst.so.6")
        except OSError as error:
            raise AssertionError("Trusted native pointer qualification requires X11 and XTEST.") from error

        self._x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
        self._x11.XOpenDisplay.restype = ctypes.c_void_p
        self._x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
        self._x11.XCloseDisplay.restype = ctypes.c_int
        self._x11.XDefaultScreen.argtypes = [ctypes.c_void_p]
        self._x11.XDefaultScreen.restype = ctypes.c_int
        self._x11.XScreenCount.argtypes = [ctypes.c_void_p]
        self._x11.XScreenCount.restype = ctypes.c_int
        self._x11.XRootWindow.argtypes = [ctypes.c_void_p, ctypes.c_int]
        self._x11.XRootWindow.restype = ctypes.c_ulong
        self._x11.XDisplayWidth.argtypes = [ctypes.c_void_p, ctypes.c_int]
        self._x11.XDisplayWidth.restype = ctypes.c_int
        self._x11.XDisplayHeight.argtypes = [ctypes.c_void_p, ctypes.c_int]
        self._x11.XDisplayHeight.restype = ctypes.c_int
        self._x11.XQueryPointer.argtypes = [
            ctypes.c_void_p,
            ctypes.c_ulong,
            ctypes.POINTER(ctypes.c_ulong),
            ctypes.POINTER(ctypes.c_ulong),
            ctypes.POINTER(ctypes.c_int),
            ctypes.POINTER(ctypes.c_int),
            ctypes.POINTER(ctypes.c_int),
            ctypes.POINTER(ctypes.c_int),
            ctypes.POINTER(ctypes.c_uint),
        ]
        self._x11.XQueryPointer.restype = ctypes.c_int
        self._x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
        self._x11.XKeysymToKeycode.restype = ctypes.c_ubyte
        self._x11.XSync.argtypes = [ctypes.c_void_p, ctypes.c_int]
        self._x11.XSync.restype = ctypes.c_int
        self._xtst.XTestQueryExtension.argtypes = [
            ctypes.c_void_p,
            ctypes.POINTER(ctypes.c_int),
            ctypes.POINTER(ctypes.c_int),
            ctypes.POINTER(ctypes.c_int),
            ctypes.POINTER(ctypes.c_int),
        ]
        self._xtst.XTestQueryExtension.restype = ctypes.c_int
        self._xtst.XTestFakeMotionEvent.argtypes = [
            ctypes.c_void_p,
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_int,
            ctypes.c_ulong,
        ]
        self._xtst.XTestFakeMotionEvent.restype = ctypes.c_int
        self._xtst.XTestFakeButtonEvent.argtypes = [
            ctypes.c_void_p,
            ctypes.c_uint,
            ctypes.c_int,
            ctypes.c_ulong,
        ]
        self._xtst.XTestFakeButtonEvent.restype = ctypes.c_int
        self._xtst.XTestFakeKeyEvent.argtypes = [
            ctypes.c_void_p,
            ctypes.c_uint,
            ctypes.c_int,
            ctypes.c_ulong,
        ]
        self._xtst.XTestFakeKeyEvent.restype = ctypes.c_int

        self._display = self._x11.XOpenDisplay(display_name.encode("utf-8"))
        self._button_down = False
        self._escape_down = False
        if not self._display:
            raise AssertionError(f"Could not open isolated X11 display {display_name!r}.")
        event_base = ctypes.c_int()
        error_base = ctypes.c_int()
        major = ctypes.c_int()
        minor = ctypes.c_int()
        if self._xtst.XTestQueryExtension(
            self._display,
            ctypes.byref(event_base),
            ctypes.byref(error_base),
            ctypes.byref(major),
            ctypes.byref(minor),
        ) != 1:
            self._x11.XCloseDisplay(self._display)
            self._display = None
            raise AssertionError("The isolated X11 display does not expose XTEST.")
        if self._x11.XScreenCount(self._display) != 1:
            self._x11.XCloseDisplay(self._display)
            self._display = None
            raise AssertionError("Trusted native pointer qualification requires one X11 screen.")
        self._screen = self._x11.XDefaultScreen(self._display)
        self._root = self._x11.XRootWindow(self._display, self._screen)
        self._width = self._x11.XDisplayWidth(self._display, self._screen)
        self._height = self._x11.XDisplayHeight(self._display, self._screen)
        if self._root == 0 or self._width <= 0 or self._height <= 0:
            self._x11.XCloseDisplay(self._display)
            self._display = None
            raise AssertionError("Trusted native pointer qualification found invalid X11 geometry.")

    def _sync(self) -> None:
        if not self._display:
            raise AssertionError("Native X11 pointer is closed.")
        self._x11.XSync(self._display, 0)

    def _query_pointer(self) -> dict[str, int]:
        if not self._display:
            raise AssertionError("Native X11 pointer is closed.")
        root_return = ctypes.c_ulong()
        child_return = ctypes.c_ulong()
        root_x = ctypes.c_int()
        root_y = ctypes.c_int()
        window_x = ctypes.c_int()
        window_y = ctypes.c_int()
        mask = ctypes.c_uint()
        if self._x11.XQueryPointer(
            self._display,
            self._root,
            ctypes.byref(root_return),
            ctypes.byref(child_return),
            ctypes.byref(root_x),
            ctypes.byref(root_y),
            ctypes.byref(window_x),
            ctypes.byref(window_y),
            ctypes.byref(mask),
        ) != 1:
            raise AssertionError("X11 could not verify the injected pointer state.")
        return {"x": root_x.value, "y": root_y.value, "mask": mask.value}

    def move(self, root_x: int, root_y: int) -> None:
        if type(root_x) is not int or type(root_y) is not int:
            raise AssertionError("Native pointer root coordinates must be exact integers.")
        if self._xtst.XTestFakeMotionEvent(self._display, -1, root_x, root_y, 0) != 1:
            raise AssertionError("XTEST rejected native pointer motion.")
        self._sync()
        observed = self._query_pointer()
        if observed["x"] != root_x or observed["y"] != root_y:
            raise AssertionError(
                f"XTEST pointer motion was clamped: requested=({root_x}, {root_y}), "
                f"observed=({observed['x']}, {observed['y']})."
            )

    def set_primary_button(self, pressed: bool) -> None:
        if type(pressed) is not bool:
            raise AssertionError("Native pointer button state must be boolean.")
        if self._xtst.XTestFakeButtonEvent(self._display, 1, int(pressed), 0) != 1:
            raise AssertionError("XTEST rejected native primary-button input.")
        if pressed:
            self._button_down = True
        self._sync()
        button_mask_set = bool(self._query_pointer()["mask"] & (1 << 8))
        if button_mask_set is not pressed:
            raise AssertionError("X11 primary-button mask did not match injected state.")
        self._button_down = pressed

    def set_escape_key(self, pressed: bool) -> None:
        if type(pressed) is not bool:
            raise AssertionError("Native Escape key state must be boolean.")
        keycode = int(self._x11.XKeysymToKeycode(self._display, 0xFF1B))
        if keycode == 0:
            raise AssertionError("X11 could not resolve the Escape keysym.")
        if self._xtst.XTestFakeKeyEvent(self._display, keycode, int(pressed), 0) != 1:
            raise AssertionError("XTEST rejected native Escape-key input.")
        self._escape_down = pressed
        self._sync()

    def close(self) -> None:
        if not self._display:
            return
        cleanup_error: BaseException | None = None
        if self._escape_down:
            try:
                if self._xtst.XTestFakeKeyEvent(
                    self._display,
                    int(self._x11.XKeysymToKeycode(self._display, 0xFF1B)),
                    0,
                    0,
                ) != 1:
                    raise AssertionError("XTEST rejected cleanup Escape key-up.")
                self._escape_down = False
            except BaseException as error:
                cleanup_error = error
        if self._button_down:
            try:
                if self._xtst.XTestFakeButtonEvent(self._display, 1, 0, 0) != 1:
                    raise AssertionError("XTEST rejected cleanup primary-button release.")
                self._button_down = False
            except BaseException as error:
                if cleanup_error is None:
                    cleanup_error = error
        try:
            self._x11.XSync(self._display, 0)
        except BaseException as error:
            if cleanup_error is None:
                cleanup_error = error
        try:
            if self._query_pointer()["mask"] & (1 << 8):
                raise AssertionError("X11 primary button remained pressed during cleanup.")
        except BaseException as error:
            if cleanup_error is None:
                cleanup_error = error
        try:
            self._x11.XCloseDisplay(self._display)
        except BaseException as error:
            if cleanup_error is None:
                cleanup_error = error
        finally:
            self._display = None
        if cleanup_error is not None:
            raise cleanup_error

    def __enter__(self) -> "NativeX11Pointer":
        return self

    def __exit__(self, _error_type: Any, _error: Any, _traceback: Any) -> None:
        self.close()


def install_native_pointer_probe(
    session_id: str,
    target: str,
) -> dict[str, int]:
    geometry = execute(
        session_id,
        """
        const row = arguments[0];
        if (
          !(row instanceof HTMLElement) ||
          !row.isConnected ||
          row.dataset.libraryRow !== 'deck' ||
          row.dataset.itemName !== 'Large library drag source'
        ) {
          throw new Error('Trusted native drag target is stale or wrong.');
        }
        const key = '__OSTUDY_NATIVE_POINTER_PROBE__';
        window[key]?.dispose?.();
        const state = {sequence: 0, events: [], row};
        const interactiveSelector = [
          "button", "a[href]", "input", "select", "textarea", "label",
          "[contenteditable]:not([contenteditable='false'])",
          "[role='button']", "[role='link']", "[role='menuitem']",
          "[data-library-no-pointer-drag]",
        ].join(",");
        const observe = (event) => {
          const eventTarget = event.target;
          const targetElement = eventTarget instanceof Element ? eventTarget : null;
          const targetWithinRow = eventTarget instanceof Node && row.contains(eventTarget);
          const interactiveTarget = targetElement?.closest(interactiveSelector) ?? null;
          const targetApprovedDragSurface =
            interactiveTarget?.getAttribute("data-library-drag-surface") === "true";
          state.sequence += 1;
          state.events.push({
            sequence: state.sequence,
            type: event.type,
            isTrusted: event.isTrusted,
            pointerId: event.pointerId,
            pointerType: event.pointerType,
            isPrimary: event.isPrimary,
            clientX: event.clientX,
            clientY: event.clientY,
            screenX: event.screenX,
            screenY: event.screenY,
            button: event.button,
            buttons: event.buttons,
            key: event.key,
            targetIsRow: eventTarget === row,
            targetWithinRow,
            targetInteractive: Boolean(interactiveTarget),
            targetApprovedDragSurface,
            targetCanStartDrag:
              targetWithinRow && (!interactiveTarget || targetApprovedDragSurface),
          });
          if (state.events.length > 64) state.events.shift();
        };
        for (const type of ['pointermove', 'pointerdown', 'pointerup', 'keydown', 'keyup']) {
          window.addEventListener(type, observe, true);
        }
        state.dispose = () => {
          for (const type of ['pointermove', 'pointerdown', 'pointerup', 'keydown', 'keyup']) {
            window.removeEventListener(type, observe, true);
          }
          if (window[key] === state) delete window[key];
        };
        window[key] = state;
        return {
          screenX: Math.round(window.screenX),
          screenY: Math.round(window.screenY),
          outerWidth: Math.round(window.outerWidth),
          outerHeight: Math.round(window.outerHeight),
          innerWidth: Math.round(window.innerWidth),
          innerHeight: Math.round(window.innerHeight),
        };
        """,
        [{webdriver.ELEMENT_KEY: target}],
    )
    expected_keys = {
        "screenX", "screenY", "outerWidth", "outerHeight", "innerWidth", "innerHeight",
    }
    if (
        not isinstance(geometry, dict)
        or set(geometry) != expected_keys
        or not all(type(geometry[key]) is int for key in expected_keys)
        or geometry["innerWidth"] <= 0
        or geometry["innerHeight"] <= 0
    ):
        raise AssertionError(f"Trusted native pointer geometry is invalid: {geometry!r}.")
    return {key: geometry[key] for key in expected_keys}


def native_pointer_events(session_id: str) -> list[dict[str, Any]]:
    events = execute_read_only(
        session_id,
        """
        const state = window.__OSTUDY_NATIVE_POINTER_PROBE__;
        return state ? state.events.map((event) => ({...event})) : null;
        """,
    )
    if not isinstance(events, list) or not all(isinstance(event, dict) for event in events):
        raise AssertionError("Trusted native pointer probe is unavailable.")
    return events


def wait_for_native_pointer_event(
    session_id: str,
    *,
    after_sequence: int,
    event_type: str,
    deadline: float,
    client_x: int | None = None,
    client_y: int | None = None,
    screen_x: int | None = None,
    screen_y: int | None = None,
    buttons: int | None = None,
    button: int | None = None,
    pointer_id: int | None = None,
    target_within_row: bool | None = None,
    target_interactive: bool | None = None,
    target_can_start_drag: bool | None = None,
) -> dict[str, Any]:
    last: list[dict[str, Any]] = []
    while time.monotonic() < deadline:
        last = native_pointer_events(session_id)
        for event in last:
            if (
                event.get("sequence", 0) > after_sequence
                and event.get("type") == event_type
                and event.get("isTrusted") is True
                and event.get("pointerType") == "mouse"
                and event.get("isPrimary") is True
                and type(event.get("pointerId")) is int
                and event.get("pointerId") > 0
                and (client_x is None or event.get("clientX") == client_x)
                and (client_y is None or event.get("clientY") == client_y)
                and (screen_x is None or event.get("screenX") == screen_x)
                and (screen_y is None or event.get("screenY") == screen_y)
                and (buttons is None or event.get("buttons") == buttons)
                and (button is None or event.get("button") == button)
                and (pointer_id is None or event.get("pointerId") == pointer_id)
                and (
                    target_within_row is None
                    or event.get("targetWithinRow") is target_within_row
                )
                and (
                    target_interactive is None
                    or event.get("targetInteractive") is target_interactive
                )
                and (
                    target_can_start_drag is None
                    or event.get("targetCanStartDrag") is target_can_start_drag
                )
            ):
                return event
        time.sleep(0.02)
    raise AssertionError(
        "Timed out waiting for trusted native pointer event "
        f"{event_type!r}; last={last[-4:]!r}."
    )


def wait_for_native_key_event(
    session_id: str,
    *,
    after_sequence: int,
    event_type: str,
    deadline: float,
) -> dict[str, Any]:
    last: list[dict[str, Any]] = []
    while time.monotonic() < deadline:
        last = native_pointer_events(session_id)
        for event in last:
            if (
                event.get("sequence", 0) > after_sequence
                and event.get("type") == event_type
                and event.get("isTrusted") is True
                and event.get("key") == "Escape"
            ):
                return event
        time.sleep(0.02)
    raise AssertionError(
        "Timed out waiting for trusted native Escape event "
        f"{event_type!r}; last={last[-4:]!r}."
    )


def native_pointer_sequence(session_id: str) -> int:
    events = native_pointer_events(session_id)
    return max((int(event.get("sequence", 0)) for event in events), default=0)


def calibrate_native_pointer(
    session_id: str,
    pointer: NativeX11Pointer,
    point: dict[str, int],
    geometry: dict[str, int],
) -> dict[str, int]:
    horizontal_inset = max(0, geometry["outerWidth"] - geometry["innerWidth"]) // 2
    vertical_inset = max(0, geometry["outerHeight"] - geometry["innerHeight"])
    origins = (
        {
            "x": geometry["screenX"] + horizontal_inset,
            "y": geometry["screenY"] + vertical_inset,
        },
        {"x": geometry["screenX"], "y": geometry["screenY"]},
    )
    last_error: BaseException | None = None
    for origin in origins:
        try:
            sequence = native_pointer_sequence(session_id)
            requested = {
                "x": origin["x"] + point["x"],
                "y": origin["y"] + point["y"],
            }
            pointer.move(requested["x"], requested["y"])
            observed = wait_for_native_pointer_event(
                session_id,
                after_sequence=sequence,
                event_type="pointermove",
                deadline=webdriver.deadline_after(2),
                buttons=0,
                button=-1,
            )
            if (
                observed.get("clientX") == point["x"]
                and observed.get("clientY") == point["y"]
                and observed.get("screenX") == requested["x"]
                and observed.get("screenY") == requested["y"]
                and observed.get("targetCanStartDrag") is True
            ):
                return requested
            corrected = {
                "x": requested["x"] + point["x"] - int(observed["clientX"]),
                "y": requested["y"] + point["y"] - int(observed["clientY"]),
            }
            sequence = int(observed["sequence"])
            pointer.move(corrected["x"], corrected["y"])
            wait_for_native_pointer_event(
                session_id,
                after_sequence=sequence,
                event_type="pointermove",
                deadline=webdriver.deadline_after(2),
                client_x=point["x"],
                client_y=point["y"],
                screen_x=corrected["x"],
                screen_y=corrected["y"],
                buttons=0,
                button=-1,
                target_can_start_drag=True,
            )
            return corrected
        except (AssertionError, KeyError, TypeError, ValueError) as error:
            last_error = error
    raise AssertionError("Could not calibrate trusted X11 pointer to the WebKit viewport.") from last_error


def remove_native_pointer_probe(session_id: str) -> None:
    execute(
        session_id,
        """
        window.__OSTUDY_NATIVE_POINTER_PROBE__?.dispose?.();
        return true;
        """,
    )


def native_library_structure(session_id: str) -> list[dict[str, str]]:
    structure = execute_read_only(
        session_id,
        """
        return Array.from(document.querySelectorAll("main [data-library-row]")).map((row) => ({
          kind: row instanceof HTMLElement ? row.dataset.libraryRow ?? '' : '',
          geometryKey: row instanceof HTMLElement ? row.dataset.geometryKey ?? '' : '',
          itemId: row instanceof HTMLElement ? row.dataset.itemId ?? '' : '',
          itemIndex: row instanceof HTMLElement ? row.dataset.itemIndex ?? '' : '',
          name: row instanceof HTMLElement ? row.dataset.itemName ?? '' : '',
          folderId: row instanceof HTMLElement ? row.dataset.folderId ?? '' : '',
          folderIndex: row instanceof HTMLElement ? row.dataset.folderIndex ?? '' : '',
          collapsed: row instanceof HTMLElement ? row.dataset.collapsed ?? '' : '',
        }));
        """,
    )
    if (
        not isinstance(structure, list)
        or not all(
            isinstance(row, dict)
            and set(row) == {
                "kind", "geometryKey", "itemId", "itemIndex",
                "name", "folderId", "folderIndex", "collapsed",
            }
            and all(isinstance(value, str) for value in row.values())
            and all(row[key] for key in ("kind", "geometryKey", "itemId", "name", "folderId"))
            for row in structure
        )
        or sum(row["name"] == "Large library drag source" for row in structure) != 1
    ):
        raise AssertionError("Trusted native drag could not establish the synthetic library structure.")
    return structure


def verify_trusted_native_drag_smoke(session_id: str) -> dict[str, Any]:
    """Prove that X11 input crosses WebKit into the real React drag adapter."""
    deadline = webdriver.deadline_after(30)
    target = webdriver.find_element(session_id, "xpath", LARGE_LIBRARY_DRAG_XPATH, deadline)
    point = find_safe_drag_point(session_id, target)
    activation = {
        "x": point["x"] + CANDIDATE_DRAG_ACTIVATION_OFFSET["x"],
        "y": point["y"] + CANDIDATE_DRAG_ACTIVATION_OFFSET["y"],
    }
    if math.hypot(activation["x"] - point["x"], activation["y"] - point["y"]) <= 8:
        raise AssertionError("Trusted native pointermove must cross the 8px activation threshold.")
    structure_before = native_library_structure(session_id)
    geometry = install_native_pointer_probe(session_id, target)
    pointer: NativeX11Pointer | None = None
    primary_error: BaseException | None = None
    try:
        pointer = NativeX11Pointer()
        root_anchor = calibrate_native_pointer(session_id, pointer, point, geometry)
        sequence = native_pointer_sequence(session_id)
        pointer.set_primary_button(True)
        down = wait_for_native_pointer_event(
            session_id,
            after_sequence=sequence,
            event_type="pointerdown",
            deadline=deadline,
            client_x=point["x"],
            client_y=point["y"],
            buttons=1,
            button=0,
            target_can_start_drag=True,
        )
        pointer.move(
            root_anchor["x"] + CANDIDATE_DRAG_ACTIVATION_OFFSET["x"],
            root_anchor["y"] + CANDIDATE_DRAG_ACTIVATION_OFFSET["y"],
        )
        moved = wait_for_native_pointer_event(
            session_id,
            after_sequence=int(down["sequence"]),
            event_type="pointermove",
            deadline=deadline,
            client_x=activation["x"],
            client_y=activation["y"],
            screen_x=root_anchor["x"] + CANDIDATE_DRAG_ACTIVATION_OFFSET["x"],
            screen_y=root_anchor["y"] + CANDIDATE_DRAG_ACTIVATION_OFFSET["y"],
            buttons=1,
            button=-1,
            pointer_id=int(down["pointerId"]),
        )
        wait_js(session_id, "document.querySelector('[data-drag-preview]')", deadline)
        pointer.set_escape_key(True)
        escape_down = wait_for_native_key_event(
            session_id,
            after_sequence=int(moved["sequence"]),
            event_type="keydown",
            deadline=deadline,
        )
        wait_js(session_id, "!document.querySelector('[data-drag-preview]')", deadline)
        pointer.set_escape_key(False)
        escape_up = wait_for_native_key_event(
            session_id,
            after_sequence=int(escape_down["sequence"]),
            event_type="keyup",
            deadline=deadline,
        )
        pointer.set_primary_button(False)
        wait_for_native_pointer_event(
            session_id,
            after_sequence=int(escape_up["sequence"]),
            event_type="pointerup",
            deadline=deadline,
            client_x=activation["x"],
            client_y=activation["y"],
            buttons=0,
            button=0,
            pointer_id=int(down["pointerId"]),
        )
        wait_js(session_id, "!document.querySelector('[data-drag-preview]')", deadline)
        webdriver.find_element(session_id, "xpath", LARGE_LIBRARY_DRAG_XPATH, deadline)
        if native_library_structure(session_id) != structure_before:
            raise AssertionError("Trusted native drag changed the synthetic library structure.")
        return {
            key: list(value) if isinstance(value, list) else value
            for key, value in TRUSTED_NATIVE_INPUT_ATTESTATION.items()
        }
    except BaseException as error:
        primary_error = error
        raise
    finally:
        cleanup_error: BaseException | None = None
        if pointer is not None:
            try:
                pointer.close()
            except BaseException as error:
                cleanup_error = error
        try:
            remove_native_pointer_probe(session_id)
        except BaseException as error:
            if cleanup_error is None:
                cleanup_error = error
        if primary_error is None and cleanup_error is not None:
            raise cleanup_error


def run_trusted_native_drag_smoke(application: Path) -> dict[str, Any]:
    """Run trusted input in a disposable driver/app process before timed samples."""
    session_id: str | None = None
    driver: subprocess.Popen[str] | None = None
    with tempfile.NamedTemporaryFile("w+", encoding="utf-8", delete=False) as driver_log:
        log_path = Path(driver_log.name)
        try:
            driver = subprocess.Popen(
                ["tauri-driver"],
                stdout=driver_log,
                stderr=subprocess.STDOUT,
                text=True,
                start_new_session=True,
            )
            webdriver.wait_for_driver(webdriver.deadline_after(20))
            session_id = webdriver.create_session(application.resolve())
            webdriver.verify_shell(session_id, webdriver.deadline_after())
            achieve_reference_viewport(session_id)
            heading = execute_read_only(
                session_id,
                "return document.querySelector('main h1')?.textContent?.trim() ?? null;",
            )
            if heading != "Flashcards":
                raise AssertionError(
                    f"Trusted native drag must start on Flashcards: {heading!r}."
                )
            return verify_trusted_native_drag_smoke(session_id)
        except BaseException:
            try:
                print(log_path.read_text(encoding="utf-8"))
            except OSError:
                pass
            raise
        finally:
            if session_id:
                try:
                    webdriver.delete_session(session_id)
                except BaseException:
                    pass
            if driver is not None and driver.poll() is None:
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


def candidate_pointer_event_contract(
    anchor: dict[str, int],
    event_type: str,
    client_x: int,
    client_y: int,
) -> dict[str, Any]:
    event_buttons = {
        "pointerdown": {"button": 0, "buttons": 1},
        "pointermove": {"button": -1, "buttons": 1},
        "pointercancel": {"button": -1, "buttons": 0},
    }
    if event_type not in event_buttons:
        raise AssertionError(f"Unsupported candidate pointer event type: {event_type!r}")
    if (
        set(anchor) != {"x", "y"}
        or not all(type(anchor[axis]) is int for axis in ("x", "y"))
        or type(client_x) is not int
        or type(client_y) is not int
    ):
        raise AssertionError("Candidate pointer coordinates must be exact integers.")
    return {
        "type": event_type,
        "pointerId": CANDIDATE_DRAG_POINTER_ID,
        "pointerType": "mouse",
        "isPrimary": True,
        "clientX": client_x,
        "clientY": client_y,
        **event_buttons[event_type],
        "bubbles": True,
        "cancelable": True,
        "composed": True,
        "anchorX": anchor["x"],
        "anchorY": anchor["y"],
    }


def validate_candidate_pointer_observation(
    observed: Any,
    expected: dict[str, Any],
) -> None:
    expected_observed = {
        key: value for key, value in expected.items() if key not in {"anchorX", "anchorY"}
    }
    expected_observed.update({"targetObserved": True, "windowObserved": True})
    if observed != expected_observed:
        raise AssertionError(
            "Candidate PointerEvent observed contract differs: "
            f"expected {expected_observed!r}, received {observed!r}."
        )


def dispatch_candidate_drag_pointer_event(
    session_id: str,
    target: str,
    anchor: dict[str, int],
    event_type: str,
    client_x: int,
    client_y: int,
) -> None:
    expected = candidate_pointer_event_contract(anchor, event_type, client_x, client_y)
    try:
        observed = execute(
            session_id,
            """
            const row = arguments[0];
            const expected = arguments[1];
            if (typeof PointerEvent !== 'function') {
              throw new Error('Candidate PointerEvent is unavailable.');
            }
            if (
              !(row instanceof HTMLElement) ||
              !row.isConnected ||
              row.dataset.libraryRow !== 'deck' ||
              row.dataset.itemName !== 'Large library drag source' ||
              document.elementFromPoint(expected.anchorX, expected.anchorY) !== row
            ) {
              throw new Error('Candidate deck drag target is stale or wrong.');
            }
            const event = new PointerEvent(expected.type, {
              pointerId: expected.pointerId,
              pointerType: expected.pointerType,
              isPrimary: expected.isPrimary,
              clientX: expected.clientX,
              clientY: expected.clientY,
              button: expected.button,
              buttons: expected.buttons,
              bubbles: expected.bubbles,
              cancelable: expected.cancelable,
              composed: expected.composed,
            });
            const contract = {
              type: event.type, pointerId: event.pointerId, pointerType: event.pointerType,
              isPrimary: event.isPrimary, clientX: event.clientX, clientY: event.clientY,
              button: event.button, buttons: event.buttons, bubbles: event.bubbles,
              cancelable: event.cancelable, composed: event.composed,
            };
            for (const field of Object.keys(contract)) {
              if (contract[field] !== expected[field]) {
                throw new Error(`Candidate PointerEvent contract mismatch for ${field}.`);
              }
            }
            let targetObserved = false;
            let windowObserved = false;
            const observeTarget = (observedEvent) => {
              targetObserved = observedEvent === event && observedEvent.target === row;
            };
            const observeWindow = (observedEvent) => { windowObserved = observedEvent === event; };
            row.addEventListener(expected.type, observeTarget, {once: true});
            window.addEventListener(expected.type, observeWindow, {capture: true, once: true});
            try {
              row.dispatchEvent(event);
            } finally {
              row.removeEventListener(expected.type, observeTarget);
              window.removeEventListener(expected.type, observeWindow, true);
            }
            if (!targetObserved || !windowObserved) {
              throw new Error('Candidate PointerEvent was not observed at both target and window.');
            }
            return {...contract, targetObserved, windowObserved};
            """,
            [{webdriver.ELEMENT_KEY: target}, expected],
        )
    except webdriver.WebDriverProtocolError as error:
        if error.error == "stale element reference":
            raise AssertionError("Candidate deck drag target is stale or wrong.") from error
        raise
    validate_candidate_pointer_observation(observed, expected)


def dispatch_candidate_drag_activation_and_finish(
    session_id: str,
    target: str,
    anchor: dict[str, int],
    client_x: int,
    client_y: int,
) -> dict[str, Any]:
    expected = candidate_pointer_event_contract(
        anchor, "pointermove", client_x, client_y
    )
    try:
        result = webdriver.request(
            "POST",
            f"/session/{session_id}/execute/async",
            {
                "script": """
                  const row = arguments[0];
                  const expected = arguments[1];
                  const done = arguments[arguments.length - 1];
                  const errorResult = (error) => ({
                    ok: false,
                    error: {
                      name: error instanceof Error ? error.name : typeof error,
                      message: error instanceof Error ? error.message : String(error),
                    },
                  });
                  try {
                    if (typeof PointerEvent !== 'function') {
                      throw new Error('Candidate PointerEvent is unavailable.');
                    }
                    if (
                      !(row instanceof HTMLElement) ||
                      !row.isConnected ||
                      row.dataset.libraryRow !== 'deck' ||
                      row.dataset.itemName !== 'Large library drag source' ||
                      document.elementFromPoint(expected.anchorX, expected.anchorY) !== row
                    ) {
                      throw new Error('Candidate deck drag target is stale or wrong.');
                    }
                    const event = new PointerEvent(expected.type, {
                      pointerId: expected.pointerId, pointerType: expected.pointerType,
                      isPrimary: expected.isPrimary, clientX: expected.clientX,
                      clientY: expected.clientY, button: expected.button,
                      buttons: expected.buttons, bubbles: expected.bubbles,
                      cancelable: expected.cancelable, composed: expected.composed,
                    });
                    const contract = {
                      type: event.type, pointerId: event.pointerId,
                      pointerType: event.pointerType, isPrimary: event.isPrimary,
                      clientX: event.clientX, clientY: event.clientY,
                      button: event.button, buttons: event.buttons,
                      bubbles: event.bubbles, cancelable: event.cancelable,
                      composed: event.composed,
                    };
                    for (const field of Object.keys(contract)) {
                      if (contract[field] !== expected[field]) {
                        throw new Error(`Candidate PointerEvent contract mismatch for ${field}.`);
                      }
                    }
                    let targetObserved = false;
                    let windowObserved = false;
                    const observeTarget = (observedEvent) => {
                      targetObserved = observedEvent === event && observedEvent.target === row;
                    };
                    const observeWindow = (observedEvent) => {
                      windowObserved = observedEvent === event;
                    };
                    row.addEventListener(expected.type, observeTarget, {once: true});
                    window.addEventListener(expected.type, observeWindow, {capture: true, once: true});
                    try {
                      row.dispatchEvent(event);
                    } finally {
                      row.removeEventListener(expected.type, observeTarget);
                      window.removeEventListener(expected.type, observeWindow, true);
                    }
                    if (!targetObserved || !windowObserved) {
                      throw new Error('Candidate PointerEvent was not observed at both target and window.');
                    }
                    const observed = {...contract, targetObserved, windowObserved};
                    const bridge = window.__OSTUDY_UI_PERFORMANCE__;
                    if (!bridge) {
                      done({ok: false, error: {name: 'BridgeUnavailableError', message: 'The UI performance bridge is unavailable.'}});
                      return;
                    }
                    let settled = false;
                    const complete = (value) => {
                      if (settled) return;
                      settled = true;
                      window.clearTimeout(timeout);
                      done(value);
                    };
                    const timeout = window.setTimeout(() => complete({
                      ok: false,
                      error: {name: 'DragPreviewTimeoutError', message: 'The activated drag preview did not become visible.'},
                    }), 5000);
                    const awaitPreview = () => {
                      if (document.querySelector('[data-drag-preview]')) {
                        Promise.resolve()
                          .then(() => bridge.finish())
                          .then(
                            (sample) => complete({ok: true, observed, sample}),
                            (error) => complete(errorResult(error)),
                          );
                        return;
                      }
                      requestAnimationFrame(awaitPreview);
                    };
                    requestAnimationFrame(awaitPreview);
                  } catch (error) {
                    done(errorResult(error));
                  }
                """,
                "args": [{webdriver.ELEMENT_KEY: target}, expected],
            },
        )
    except webdriver.WebDriverProtocolError as error:
        if error.error == "stale element reference":
            raise AssertionError("Candidate deck drag target is stale or wrong.") from error
        raise
    sample = performance_sample_from_result(result)
    validate_candidate_pointer_observation(result.get("observed"), expected)
    return sample


def position_and_press_drag_pointer(
    session_id: str,
    point: dict[str, int],
    *,
    pointer_id: str,
    pointer_type: str,
) -> None:
    webdriver.request(
        "POST",
        f"/session/{session_id}/actions",
        {
            "actions": [{
                "type": "pointer",
                "id": pointer_id,
                "parameters": {"pointerType": pointer_type},
                "actions": [
                    {
                        "type": "pointerMove",
                        "duration": 0,
                        "origin": "viewport",
                        "x": point["x"],
                        "y": point["y"],
                    },
                    {"type": "pointerDown", "button": 0},
                ],
            }],
        },
    )


def activate_drag_pointer(
    session_id: str,
    *,
    pointer_id: str,
    pointer_type: str,
) -> None:
    webdriver.request(
        "POST",
        f"/session/{session_id}/actions",
        {
            "actions": [{
                "type": "pointer",
                "id": pointer_id,
                "parameters": {"pointerType": pointer_type},
                "actions": [{
                    "type": "pointerMove",
                    "duration": 80,
                    "origin": "pointer",
                    "x": 24,
                    "y": 16,
                }],
            }],
        },
    )


def cancel_drag_pointer(
    session_id: str,
    *,
    pointer_id: str,
    pointer_type: str,
) -> None:
    webdriver.request(
        "POST",
        f"/session/{session_id}/actions",
        {
            "actions": [{
                "type": "pointer",
                "id": pointer_id,
                "parameters": {"pointerType": pointer_type},
                "actions": [
                    {
                        "type": "pointerMove",
                        "duration": 80,
                        "origin": "viewport",
                        "x": 1400,
                        "y": 850,
                    },
                    {"type": "pointerCancel"},
                ],
            }],
        },
    )


def collect_drag_sample(
    session_id: str,
    target: str,
    scenario: str,
    deadline: float,
    *,
    pointer_id: str,
    pointer_type: str,
) -> dict[str, Any]:
    point = find_safe_drag_point(session_id, target)
    try:
        position_and_press_drag_pointer(
            session_id,
            point,
            pointer_id=pointer_id,
            pointer_type=pointer_type,
        )
        arm(session_id, scenario, target)
        activate_drag_pointer(
            session_id,
            pointer_id=pointer_id,
            pointer_type=pointer_type,
        )
        wait_js(session_id, "document.querySelector('[data-drag-preview]')", deadline)
        cancel_drag_pointer(
            session_id,
            pointer_id=pointer_id,
            pointer_type=pointer_type,
        )
        wait_js(session_id, "!document.querySelector('[data-drag-preview]')", deadline)
        return finish(session_id)
    finally:
        release_pointer_actions(session_id)


def collect_candidate_drag_sample(
    session_id: str,
    target: str,
    scenario: str,
    deadline: float,
) -> dict[str, Any]:
    point: dict[str, int] | None = None
    activation_point: dict[str, int] | None = None
    pointer_down_delivered = False
    cancel_attempted = False
    primary_error: BaseException | None = None
    try:
        point = find_safe_drag_point(session_id, target)
        activation_point = {
            "x": point["x"] + CANDIDATE_DRAG_ACTIVATION_OFFSET["x"],
            "y": point["y"] + CANDIDATE_DRAG_ACTIVATION_OFFSET["y"],
        }
        if math.hypot(
            activation_point["x"] - point["x"],
            activation_point["y"] - point["y"],
        ) <= 8:
            raise AssertionError("Candidate pointermove must be strictly beyond the 8px threshold.")
        dispatch_candidate_drag_pointer_event(
            session_id,
            target,
            point,
            "pointerdown",
            point["x"],
            point["y"],
        )
        pointer_down_delivered = True
        arm(session_id, scenario, target)
        # Keep activation, preview readiness, and timing finalization inside one
        # bounded browser command. WebKit can idle its frame clock between
        # automation commands even when the X11 surface itself is healthy.
        sample = dispatch_candidate_drag_activation_and_finish(
            session_id,
            target,
            point,
            activation_point["x"],
            activation_point["y"],
        )
        wait_js(session_id, "document.querySelector('[data-drag-preview]')", deadline)
        cancel_attempted = True
        dispatch_candidate_drag_pointer_event(
            session_id,
            target,
            point,
            "pointercancel",
            activation_point["x"],
            activation_point["y"],
        )
        wait_js(session_id, "!document.querySelector('[data-drag-preview]')", deadline)
        return sample
    except BaseException as error:
        primary_error = error
        if (
            pointer_down_delivered
            and not cancel_attempted
            and point is not None
            and activation_point is not None
        ):
            cancel_attempted = True
            try:
                dispatch_candidate_drag_pointer_event(
                    session_id,
                    target,
                    point,
                    "pointercancel",
                    activation_point["x"],
                    activation_point["y"],
                )
            except BaseException:
                pass
        if pointer_down_delivered:
            try:
                wait_js(
                    session_id,
                    "!document.querySelector('[data-drag-preview]')",
                    webdriver.deadline_after(5),
                )
            except BaseException:
                pass
        raise
    finally:
        try:
            release_pointer_actions(session_id)
        except BaseException:
            if primary_error is None:
                raise


def is_transient_disconnect(error: BaseException) -> bool:
    if isinstance(error, (http.client.RemoteDisconnected, ConnectionResetError, BrokenPipeError)):
        return True
    return isinstance(error, urllib.error.URLError) and isinstance(
        error.reason,
        (http.client.RemoteDisconnected, ConnectionResetError, BrokenPipeError),
    )


def wait_js(session_id: str, expression: str, deadline: float) -> None:
    last: Any = None
    while time.monotonic() < deadline:
        last = execute_read_only(session_id, f"return Boolean({expression});")
        if last is True: return
        time.sleep(0.05)
    raise AssertionError(f"Timed out waiting for explicit ready condition: {expression}; last={last!r}")


def practice_setup_state(session_id: str) -> dict[str, Any]:
    # Return an explicit allowlist rather than DOM text so timeout evidence cannot
    # carry synthetic or learner-authored content into logs.
    state = execute_read_only(session_id, f"""
      const checkbox = document.evaluate(
        \"{PRACTICE_DECK_CHECKBOX_XPATH}\",
        document,
        null,
        XPathResult.FIRST_ORDERED_NODE_TYPE,
        null
      ).singleNodeValue;
      const start = document.evaluate(
        \"{PRACTICE_START_XPATH}\",
        document,
        null,
        XPathResult.FIRST_ORDERED_NODE_TYPE,
        null
      ).singleNodeValue;
      const deckList = checkbox?.parentElement?.parentElement;
      const selectedDeckCount = deckList
        ? Array.from(deckList.querySelectorAll(\"input[type='checkbox']\"))
            .filter((candidate) => candidate instanceof HTMLInputElement && candidate.checked)
            .length
        : 0;
      const eligibleDeckCount = deckList
        ? deckList.querySelectorAll(\"input[type='checkbox']\").length
        : 0;
      const statusSpans = Array.from(document.querySelectorAll('main span'));
      let workloadPhase = 'settled';
      if (!(checkbox instanceof HTMLInputElement)) workloadPhase = 'unavailable';
      else if (statusSpans.some((candidate) => candidate.textContent?.trim() === 'Preparing workload…')) workloadPhase = 'idle';
      else if (statusSpans.some((candidate) => candidate.textContent?.trim() === 'Select at least one deck to plan a session.')) workloadPhase = 'empty';
      else if (statusSpans.some((candidate) => candidate.textContent?.trim() === 'Updating…')) workloadPhase = 'updating';
      else if (statusSpans.some((candidate) => candidate.textContent?.trim().startsWith('Workload unavailable'))) workloadPhase = 'error';
      const startEnabled = start instanceof HTMLButtonElement && !start.disabled;
      return {{
        targetFound: checkbox instanceof HTMLInputElement,
        eligibleDeckCount,
        selectedDeckCount,
        targetChecked: checkbox instanceof HTMLInputElement ? checkbox.checked : null,
        workloadPhase,
        startFound: start instanceof HTMLButtonElement,
        startEnabled,
        workloadTransitionObserved:
          window.__OSTUDY_UI_PERFORMANCE_PRACTICE__?.sawUpdating === true
          || window.__OSTUDY_UI_PERFORMANCE_PRACTICE__?.sawSelectionChange === true,
      }};
    """)
    if not isinstance(state, dict) or set(state) != PRACTICE_SETUP_STATE_KEYS:
        raise AssertionError("Invalid structured practice setup state.")
    for key in ("targetFound", "startFound", "startEnabled", "workloadTransitionObserved"):
        if not isinstance(state[key], bool):
            raise AssertionError("Invalid structured practice setup state.")
    for key in ("eligibleDeckCount", "selectedDeckCount"):
        if isinstance(state[key], bool) or not isinstance(state[key], int) or not 0 <= state[key] <= 2_000:
            raise AssertionError("Invalid structured practice setup state.")
    if state["selectedDeckCount"] > state["eligibleDeckCount"]:
        raise AssertionError("Invalid structured practice setup state.")
    if state["targetChecked"] is not None and not isinstance(state["targetChecked"], bool):
        raise AssertionError("Invalid structured practice setup state.")
    if state["workloadPhase"] not in PRACTICE_WORKLOAD_PHASES:
        raise AssertionError("Invalid structured practice setup state.")
    return state


def wait_for_practice_state(
    session_id: str,
    deadline: float,
    *,
    expected_checkbox_checked: bool | None = None,
    require_all_selected: bool = False,
    require_workload_settled: bool = True,
    require_start_ready: bool = False,
    require_workload_transition: bool = False,
) -> dict[str, Any]:
    last: dict[str, Any] | None = None
    while time.monotonic() < deadline:
        last = practice_setup_state(session_id)
        expected_selection = (
            last["eligibleDeckCount"]
            if expected_checkbox_checked is not False
            else last["eligibleDeckCount"] - 1
        )
        if (
            last["targetFound"]
            and last["eligibleDeckCount"] > 0
            and (not require_workload_settled or last["workloadPhase"] == "settled")
            and (expected_checkbox_checked is None or last["targetChecked"] is expected_checkbox_checked)
            and (not require_all_selected or last["selectedDeckCount"] == expected_selection)
            and (not require_start_ready or (last["startFound"] and last["startEnabled"]))
            and (not require_workload_transition or last["workloadTransitionObserved"])
        ):
            return last
        time.sleep(0.05)
    raise AssertionError(f"Timed out waiting for structured practice setup state; last={last!r}")


def begin_practice_workload_observation(session_id: str) -> None:
    # The observer records only whether the enumerated loading phase occurred.
    # Installing it before arm keeps setup work outside the measured interval.
    installed = retry_idempotent_transport_once(lambda: execute(session_id, """
      window.__OSTUDY_UI_PERFORMANCE_PRACTICE__?.observer?.disconnect();
      const target = document.evaluate(
        "//main//h2[normalize-space(.)='Select decks to study']/following::input[@type='checkbox'][1]",
        document,
        null,
        XPathResult.FIRST_ORDERED_NODE_TYPE,
        null
      ).singleNodeValue;
      const deckList = target?.parentElement?.parentElement;
      const selectedCount = () => deckList
        ? Array.from(deckList.querySelectorAll("input[type='checkbox']"))
            .filter((candidate) => candidate instanceof HTMLInputElement && candidate.checked)
            .length
        : null;
      const state = {
        sawUpdating: false,
        sawSelectionChange: false,
        initialSelectedCount: selectedCount(),
        observer: null,
      };
      const observePhase = () => {
        if (Array.from(document.querySelectorAll('main span'))
          .some((candidate) => candidate.textContent?.trim() === 'Updating…')) {
          state.sawUpdating = true;
        }
        const currentSelectedCount = selectedCount();
        if (state.initialSelectedCount !== null
          && currentSelectedCount !== null
          && currentSelectedCount !== state.initialSelectedCount) {
          state.sawSelectionChange = true;
        }
      };
      state.observer = new MutationObserver(observePhase);
      state.observer.observe(document.querySelector('main') ?? document.body, {
        subtree: true,
        childList: true,
        characterData: true,
      });
      window.__OSTUDY_UI_PERFORMANCE_PRACTICE__ = state;
      observePhase();
      return true;
    """))
    if installed is not True:
        raise AssertionError("Unable to install the bounded practice workload observer.")


def end_practice_workload_observation(session_id: str) -> None:
    removed = retry_idempotent_transport_once(lambda: execute(session_id, """
      window.__OSTUDY_UI_PERFORMANCE_PRACTICE__?.observer?.disconnect();
      delete window.__OSTUDY_UI_PERFORMANCE_PRACTICE__;
      return true;
    """))
    if removed is not True:
        raise AssertionError("Unable to remove the bounded practice workload observer.")


def wait_for_practice_setup_ready(
    session_id: str,
    deadline: float,
    *,
    expected_checkbox_checked: bool | None = None,
) -> dict[str, Any]:
    return wait_for_practice_state(
        session_id,
        deadline,
        expected_checkbox_checked=expected_checkbox_checked,
        require_all_selected=True,
        require_start_ready=True,
    )


def wait_for_practice_selection(
    session_id: str,
    expected_checkbox_checked: bool,
    deadline: float,
) -> dict[str, Any]:
    return wait_for_practice_state(
        session_id,
        deadline,
        expected_checkbox_checked=expected_checkbox_checked,
        require_all_selected=True,
        require_start_ready=True,
        require_workload_transition=True,
    )


def canonicalize_practice_setup(session_id: str, deadline: float) -> None:
    state = wait_for_practice_state(
        session_id,
        deadline,
        require_workload_settled=False,
    )
    if state["selectedDeckCount"] != state["eligibleDeckCount"]:
        select_all = webdriver.find_element(
            session_id,
            "xpath",
            PRACTICE_SELECT_ALL_XPATH,
            deadline,
        )
        begin_practice_workload_observation(session_id)
        try:
            webdriver.click(session_id, select_all)
            wait_for_practice_selection(session_id, True, deadline)
        finally:
            end_practice_workload_observation(session_id)
    else:
        wait_for_practice_setup_ready(
            session_id,
            deadline,
            expected_checkbox_checked=True,
        )


def checkbox_checked(session_id: str, element_id: str) -> bool:
    checked = execute_read_only(
        session_id,
        "return arguments[0] instanceof HTMLInputElement ? arguments[0].checked : null;",
        [{webdriver.ELEMENT_KEY: element_id}],
    )
    if not isinstance(checked, bool):
        raise AssertionError(f"Invalid practice deck checkbox state: {checked!r}")
    return checked


def reset_study_guide_dialog(session_id: str, deadline: float) -> None:
    dialog_absent_expression = "!document.querySelector('[role=dialog]')"
    close_selector = "[role='dialog'] button[aria-label='Close dialog']"
    close = webdriver.find_element(
        session_id,
        "css selector",
        close_selector,
        deadline,
    )
    try:
        webdriver.click(session_id, close)
    except (OSError, urllib.error.URLError) as error:
        if not is_transient_disconnect(error):
            raise
        dialog_absent = execute_read_only(
            session_id,
            f"return Boolean({dialog_absent_expression});",
        )
        if not isinstance(dialog_absent, bool):
            raise AssertionError(
                "Invalid study-guide dialog reset postcondition: "
                f"{dialog_absent!r}"
            )
        if not dialog_absent:
            close = webdriver.find_element(
                session_id,
                "css selector",
                close_selector,
                deadline,
            )
            webdriver.click(session_id, close)
    wait_js(session_id, dialog_absent_expression, deadline)


def navigate(session_id: str, section: str, heading: str) -> None:
    current_heading = execute_read_only(
        session_id,
        "return document.querySelector('main h1')?.textContent?.trim() ?? null;",
    )
    if current_heading == heading:
        return
    element = webdriver.find_element(session_id, "css selector", f"button[data-testid='nav-{section}']", webdriver.deadline_after(20))
    webdriver.click(session_id, element)
    webdriver.wait_for_main_heading(session_id, heading, webdriver.deadline_after(20))


def achieve_reference_viewport(session_id: str) -> None:
    outer_width = VIEWPORT["width"]
    outer_height = VIEWPORT["height"]
    for _ in range(8):
        webdriver.request("POST", f"/session/{session_id}/window/rect", {"width": outer_width, "height": outer_height})
        dimensions = execute_read_only(session_id, "return {width: window.innerWidth, height: window.innerHeight};")
        if dimensions == VIEWPORT:
            ui.wait_for_layout_mode(session_id, "wide", webdriver.deadline_after(20))
            return
        if not isinstance(dimensions, dict): raise AssertionError(f"Invalid viewport dimensions: {dimensions!r}")
        outer_width += VIEWPORT["width"] - int(dimensions["width"])
        outer_height += VIEWPORT["height"] - int(dimensions["height"])
    raise AssertionError(f"Could not achieve exact {VIEWPORT['width']}x{VIEWPORT['height']} CSS viewport.")


def verify_rendering_frame_clock(session_id: str) -> dict[str, Any]:
    result = webdriver.request(
        "POST",
        f"/session/{session_id}/execute/async",
        {
            "script": """
              const done = arguments[arguments.length - 1];
              const timestamps = [];
              const sample = (timestamp) => {
                timestamps.push(timestamp);
                if (timestamps.length < 8) {
                  requestAnimationFrame(sample);
                  return;
                }
                done({
                  visibilityState: document.visibilityState,
                  hidden: document.hidden,
                  hasFocus: document.hasFocus(),
                  intervals: timestamps.slice(1).map(
                    (value, index) => value - timestamps[index],
                  ),
                });
              };
              requestAnimationFrame(sample);
            """,
            "args": [],
        },
    )
    if not isinstance(result, dict) or set(result) != {
        "visibilityState", "hidden", "hasFocus", "intervals",
    }:
        raise AssertionError(f"Invalid rendering frame-clock evidence: {result!r}")
    intervals = result["intervals"]
    if (
        result["visibilityState"] != "visible"
        or result["hidden"] is not False
        or type(result["hasFocus"]) is not bool
        or not isinstance(intervals, list)
        or len(intervals) != 7
        or any(type(value) not in {int, float} or not math.isfinite(value) or value < 0 for value in intervals)
    ):
        raise AssertionError(f"Rendering surface is not visibly measurable: {result!r}")
    median_interval = float(statistics.median(intervals))
    maximum_interval = float(max(intervals))
    if median_interval > 50:
        raise AssertionError(
            "Rendering frame clock is throttled before capture: "
            f"median={median_interval:.3f}ms maximum={maximum_interval:.3f}ms "
            f"focus={result['hasFocus']!r}."
        )
    evidence = {
        "visibilityState": result["visibilityState"],
        "hasFocus": result["hasFocus"],
        "medianIntervalMs": median_interval,
        "maximumIntervalMs": maximum_interval,
    }
    print(
        "[ui-performance] Verified active rendering frame clock: "
        f"visibility={evidence['visibilityState']} focus={evidence['hasFocus']} "
        f"median={median_interval:.3f}ms maximum={maximum_interval:.3f}ms",
        flush=True,
    )
    return evidence


def collect_scenario(session_id: str, scenario: str, sample_index: int) -> dict[str, Any]:
    deadline = webdriver.deadline_after(30)
    if scenario == "navigation-hover-v1":
        navigate(session_id, "decks", "Flashcards")
        target = webdriver.find_element(session_id, "css selector", "button[data-testid='nav-analytics']", deadline)
        park_mouse(session_id)
        arm(session_id, scenario, target); pointer_actions(session_id, target, release=False)
        wait_js(session_id, "document.querySelector(\"button[data-testid='nav-analytics']\").matches(':hover')", deadline)
    elif scenario == "document-list-interaction-v1":
        navigate(session_id, "documents", "Documents")
        target = webdriver.find_element(session_id, "xpath", "//label[contains(.,'Show archived documents')]/input", deadline)
        arm(session_id, scenario, target); webdriver.click(session_id, target)
        wait_js(session_id, "document.querySelectorAll('main article').length === 2", deadline)
    elif scenario == "deck-drag-v1":
        navigate(session_id, "decks", "Flashcards")
        target = webdriver.find_element(
            session_id,
            "css selector",
            "main [data-library-row='deck']",
            deadline,
        )
        result = collect_drag_sample(
            session_id,
            target,
            scenario,
            deadline,
            pointer_id=LEGACY_DRAG_POINTER_ID,
            pointer_type="touch",
        )
    elif scenario == "deck-selection-v1":
        navigate(session_id, "practice", "Practice")
        canonicalize_practice_setup(session_id, deadline)
        target = webdriver.find_element(session_id, "xpath", PRACTICE_DECK_CHECKBOX_XPATH, deadline)
        if checkbox_checked(session_id, target) is not True:
            raise AssertionError("Canonical practice target was not selected.")
        begin_practice_workload_observation(session_id)
        try:
            arm(session_id, scenario, target); webdriver.click(session_id, target)
            wait_for_practice_selection(session_id, False, deadline)
            result = finish(session_id)
        finally:
            end_practice_workload_observation(session_id)
    elif scenario == "study-guide-open-v1":
        navigate(session_id, "practice", "Practice")
        target = webdriver.find_element(session_id, "xpath", "//main//button[normalize-space(.)='Open study guide']", deadline)
        arm(session_id, scenario, target); webdriver.click(session_id, target)
        wait_js(session_id, "document.querySelector('[role=dialog]') && !document.querySelector('[role=dialog]').textContent.includes('Loading study guide')", deadline)
    else:
        if sample_index == 0:
            navigate(session_id, "practice", "Practice")
            canonicalize_practice_setup(session_id, deadline)
            start = webdriver.find_element(session_id, "xpath", PRACTICE_START_XPATH, deadline)
            webdriver.click(session_id, start)
        wait_js(session_id, "document.body.textContent.includes('Show answer')", deadline)
        reveal = webdriver.find_element(session_id, "xpath", "//main//button[normalize-space(.)='Show answer']", deadline)
        webdriver.click(session_id, reveal)
        rate = webdriver.find_element(session_id, "xpath", "//main//button[contains(normalize-space(.),'Easy')]", deadline)
        arm(session_id, scenario, rate); webdriver.click(session_id, rate)
        wait_js(session_id, f"document.body.textContent.includes('{sample_index + 1} studied')", deadline)
    if scenario not in {"deck-drag-v1", "deck-selection-v1"}:
        result = finish(session_id)
    # Reset non-durable interactions deterministically.
    if scenario == "navigation-hover-v1":
        release_pointer_actions(session_id)
    elif scenario == "document-list-interaction-v1":
        webdriver.click(session_id, target)
        wait_js(session_id, "document.querySelectorAll('main article').length === 1", deadline)
    elif scenario == "deck-selection-v1":
        reset_target = webdriver.find_element(
            session_id,
            "xpath",
            PRACTICE_DECK_CHECKBOX_XPATH,
            deadline,
        )
        begin_practice_workload_observation(session_id)
        try:
            webdriver.click(session_id, reset_target)
            wait_for_practice_selection(session_id, True, deadline)
        finally:
            end_practice_workload_observation(session_id)
    elif scenario == "study-guide-open-v1":
        reset_study_guide_dialog(session_id, deadline)
    return result


def collect_candidate_drag_scenario(session_id: str) -> dict[str, Any]:
    """Collect only the large-library drag scenario from the initial Flashcards route."""
    deadline = webdriver.deadline_after(30)
    heading = execute_read_only(
        session_id,
        "return document.querySelector('main h1')?.textContent?.trim() ?? null;",
    )
    if heading != "Flashcards":
        raise AssertionError(
            f"Stage 4A drag candidate must start on Flashcards without navigation: {heading!r}"
        )
    target = webdriver.find_element(session_id, "xpath", LARGE_LIBRARY_DRAG_XPATH, deadline)
    return collect_candidate_drag_sample(
        session_id,
        target,
        CANDIDATE_SCENARIOS[0],
        deadline,
    )


def install_staged_baseline(staging: Path, baseline_dir: Path) -> None:
    previous = baseline_dir.with_name(f".{baseline_dir.name}.previous-{os.getpid()}")
    if previous.exists(): shutil.rmtree(previous)
    moved_existing = False
    try:
        if baseline_dir.exists():
            baseline_dir.rename(previous)
            moved_existing = True
        staging.rename(baseline_dir)
    except Exception:
        if moved_existing and not baseline_dir.exists() and previous.exists():
            previous.rename(baseline_dir)
        raise
    if previous.exists(): shutil.rmtree(previous)


def capture(
    application: Path,
    baseline_dir: Path,
    source_sha: str,
    *,
    candidate: bool = False,
) -> None:
    if candidate:
        baseline_dir = validate_candidate_destination(baseline_dir)
        assert_clean_committed_head(source_sha)
    scenarios = CANDIDATE_SCENARIOS if candidate else SCENARIOS
    evidence_id = CANDIDATE_ID if candidate else BASELINE_ID
    fixture_version = CANDIDATE_FIXTURE_VERSION if candidate else FIXTURE_VERSION
    baseline_dir.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=f".{baseline_dir.name}.capture-", dir=baseline_dir.parent))
    (staging / "measurements").mkdir()
    session_id: str | None = None
    driver: subprocess.Popen[str] | None = None
    trusted_native_input: dict[str, Any] | None = None
    log = tempfile.NamedTemporaryFile("w+", encoding="utf-8", delete=False)
    log_path = Path(log.name)
    try:
        if candidate:
            print(
                "[ui-performance] Verifying trusted X11/WebKit mouse drag in a disposable session",
                flush=True,
            )
            trusted_native_input = run_trusted_native_drag_smoke(application)
        driver = subprocess.Popen(["tauri-driver"], stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True)
        webdriver.wait_for_driver(webdriver.deadline_after(20))
        session_id = webdriver.create_session(application.resolve())
        webdriver.verify_shell(session_id, webdriver.deadline_after())
        achieve_reference_viewport(session_id)
        wait_js(session_id, "window.__OSTUDY_UI_PERFORMANCE__?.profilingRenderer === true", webdriver.deadline_after(20))
        verify_rendering_frame_clock(session_id)
        resolved = execute_read_only(session_id, """
          const root = document.documentElement;
          return {
            viewport: {width: window.innerWidth, height: window.innerHeight},
            theme: root.dataset.theme,
            scalePercent: Number.parseFloat(getComputedStyle(root).getPropertyValue('--ui-scale')) * 100,
            motion: root.dataset.motion,
            materials: root.dataset.material,
            pointer: root.dataset.input,
            devicePixelRatio: window.devicePixelRatio
          };
        """)
        expected_resolved = {"viewport": VIEWPORT, "theme": "light", "scalePercent": 100, "motion": "full", "materials": "rich", "pointer": "fine"}
        if not isinstance(resolved, dict) or any(resolved.get(key) != value for key, value in expected_resolved.items()):
            raise AssertionError(f"Capture runtime did not resolve the fixed metadata contract: {resolved!r}")
        measurements: dict[str, dict[str, Any]] = {}
        for scenario in scenarios:
            recorded: list[dict[str, Any]] = []
            for index in range(WARMUP_COUNT + SAMPLE_COUNT):
                sample = (
                    collect_candidate_drag_scenario(session_id)
                    if candidate
                    else collect_scenario(
                        session_id,
                        scenario,
                        index if scenario == "active-practice-progress-v1" else 0,
                    )
                )
                if index >= WARMUP_COUNT:
                    sample["ordinal"] = index - WARMUP_COUNT + 1
                    recorded.append(sample)
            measurement = {"schemaVersion": SCHEMA_VERSION, "baselineId": evidence_id, "sourceSha": source_sha, "instrumentationVersion": INSTRUMENTATION_VERSION, "fixtureVersion": fixture_version, "scenarioId": scenario, "scenarioVersion": scenario, "samples": recorded}
            path = staging / canonical_measurement_path(scenario)
            path.write_text(json.dumps(measurement, indent=2) + "\n", encoding="utf-8")
            measurements[scenario] = measurement
        environment = {**resolved, "display": DISPLAY, "applicationSha256": sha256_file(application), "python": platform.python_version(), "platform": platform.platform(), "tauriDriver": run_version(["tauri-driver", "--version"], "unavailable"), "webkitWebDriver": run_version(["WebKitWebDriver", "--version"], "unavailable"), "xvfb": run_version(["Xvfb", "-version"], "unavailable"), "reactProfilingBuild": True}
        refs = [{"scenarioId": s, "path": canonical_measurement_path(s), "sha256": sha256_file(staging / canonical_measurement_path(s))} for s in scenarios]
        manifest = {"schemaVersion": SCHEMA_VERSION, "baselineId": evidence_id, "sourceSha": source_sha, "instrumentationVersion": INSTRUMENTATION_VERSION, "fixtureVersion": fixture_version, "warmupCount": WARMUP_COUNT, "sampleCount": SAMPLE_COUNT, "scenarioVersions": {s: s for s in scenarios}, "captureEnvironment": environment, "measurements": refs}
        if candidate:
            if trusted_native_input != TRUSTED_NATIVE_INPUT_ATTESTATION:
                fail("Trusted native input smoke did not produce the exact attestation.")
            manifest["trustedNativeInput"] = trusted_native_input
        if not candidate:
            budgets = {"schemaVersion": SCHEMA_VERSION, "baselineId": BASELINE_ID, "sourceSha": source_sha, "instrumentationVersion": INSTRUMENTATION_VERSION, "scenarioVersions": {s: s for s in SCENARIOS}, "rounding": "milliseconds-to-3-decimal-places", "entries": expected_budget_entries(measurements)}
            budgets_path = staging / "budgets.json"
            budgets_path.write_text(json.dumps(budgets, indent=2) + "\n", encoding="utf-8")
            manifest["budgets"] = {"path": "budgets.json", "sha256": sha256_file(budgets_path)}
        (staging / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        if candidate:
            verify_drag_candidate(staging, expected_source_sha=source_sha)
            install_staged_candidate(staging, baseline_dir)
        else:
            verify_baseline(staging, expected_source_sha=source_sha)
            install_staged_baseline(staging, baseline_dir)
        staging = Path()
        if not candidate:
            update_roadmap_after_capture()
    except Exception:
        try: print(log_path.read_text(encoding="utf-8"))
        except OSError: pass
        raise
    finally:
        if session_id:
            try: webdriver.delete_session(session_id)
            except Exception: pass
        if driver is not None and driver.poll() is None:
            os.killpg(driver.pid, signal.SIGTERM)
            try: driver.wait(timeout=5)
            except subprocess.TimeoutExpired: os.killpg(driver.pid, signal.SIGKILL); driver.wait(timeout=5)
        log.close()
        log_path.unlink(missing_ok=True)
        if staging != Path() and staging.exists(): shutil.rmtree(staging)


def update_roadmap_after_capture(roadmap_path: Path = ROADMAP_PATH) -> None:
    """Check only the two Stage 0D ledger rows after evidence is installed and verified."""
    source = roadmap_path.read_text(encoding="utf-8")
    replacements = {
        "- [ ] Add reproducible User Timing/React Profiler scenarios for navigation hover, document-list interaction, deck drag, deck selection, guide open, and active-practice progress.":
            "- [x] [Add reproducible User Timing/React Profiler scenarios for navigation hover, document-list interaction, deck drag, deck selection, guide open, and active-practice progress](ui-workflow-redesign-performance-baseline.md#scenario-contract).",
        "- [ ] Record initial interaction traces and define budgets for avoidable React commits, pointer-frame work, and visible hover/drag latency.":
            "- [x] [Record initial interaction traces and define budgets for avoidable React commits, pointer-frame work, and visible hover/drag latency](ui-workflow-redesign-performance-baseline.md#evidence-and-budgets).",
    }
    updated = source
    for old, new in replacements.items():
        open_count = updated.count(old)
        complete_count = updated.count(new)
        if open_count == 1 and complete_count == 0:
            updated = updated.replace(old, new)
        elif open_count != 0 or complete_count != 1:
            fail(f"Roadmap capture update expected one open or completed Stage 0 row: {old}")
    temporary = roadmap_path.with_suffix(".md.performance-capture.tmp")
    temporary.write_text(updated, encoding="utf-8")
    temporary.replace(roadmap_path)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    verify = sub.add_parser("verify"); verify.add_argument("--baseline-dir", type=Path, default=BASELINE_DIR); verify.add_argument("--source-sha")
    verify_reference_parser = sub.add_parser("verify-reference"); verify_reference_parser.add_argument("--baseline-dir", type=Path, default=BASELINE_DIR)
    compare = sub.add_parser("compare"); compare.add_argument("--baseline-dir", type=Path, default=BASELINE_DIR); compare.add_argument("--candidate-dir", type=Path, required=True); compare.add_argument("--source-sha")
    compare_candidate = sub.add_parser("compare-candidate"); compare_candidate.add_argument("--baseline-dir", type=Path, default=BASELINE_DIR); compare_candidate.add_argument("--candidate-dir", type=Path, required=True); compare_candidate.add_argument("--source-sha")
    capture_parser = sub.add_parser("capture"); capture_parser.add_argument("--baseline-dir", type=Path, default=BASELINE_DIR); capture_parser.add_argument("--application", type=Path, required=True); capture_parser.add_argument("--source-sha", required=True)
    candidate_parser = sub.add_parser("capture-candidate"); candidate_parser.add_argument("--candidate-dir", type=Path, required=True); candidate_parser.add_argument("--application", type=Path, required=True); candidate_parser.add_argument("--source-sha", required=True)
    validate_candidate = sub.add_parser("validate-candidate-path"); validate_candidate.add_argument("--candidate-dir", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "verify":
        verify_baseline(args.baseline_dir, expected_source_sha=args.source_sha)
        print("Verified Stage 0D performance baseline against the current source (read-only).")
    elif args.command == "verify-reference":
        verify_reference(args.baseline_dir)
        print("Verified immutable Stage 0D reference structure at its recorded source (read-only).")
    elif args.command == "compare":
        failures = compare_baseline(args.candidate_dir, args.baseline_dir, expected_source_sha=args.source_sha)
        if failures: raise PerformanceBaselineError("Performance budget comparison failed:\n" + "\n".join(failures))
        print("Candidate satisfies all comparable Stage 0D budgets (read-only).")
    elif args.command == "compare-candidate":
        failures = compare_drag_candidate(
            args.candidate_dir,
            args.baseline_dir,
            expected_candidate_source_sha=args.source_sha,
        )
        if failures:
            raise PerformanceBaselineError(
                "Stage 4A drag budget comparison failed:\n" + "\n".join(failures)
            )
        print("Stage 4A candidate satisfies all three deck-drag-v1 limits (read-only).")
    elif args.command == "capture":
        capture(args.application, args.baseline_dir, args.source_sha)
        print("Captured and verified Stage 0D performance baseline.")
    elif args.command == "capture-candidate":
        capture(args.application, args.candidate_dir, args.source_sha, candidate=True)
        print("Captured and atomically installed the verified Stage 4A candidate.")
    else:
        print(validate_candidate_destination(args.candidate_dir))
    return 0


if __name__ == "__main__": raise SystemExit(main())
