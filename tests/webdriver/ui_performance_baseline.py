#!/usr/bin/env python3
"""Capture, verify, and compare ADR 0049 Stage 0D UI performance evidence."""

from __future__ import annotations

import argparse
import hashlib
import http.client
import json
import math
import os
import platform
import re
import shutil
import signal
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
SCHEMA_VERSION = 1
INSTRUMENTATION_VERSION = "ui-performance-v1"
FIXTURE_VERSION = "ui-performance-fixture-v1"
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
ROADMAP_PATH = ROOT_DIR / "docs/development/ui-workflow-redesign-roadmap.md"
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
    try:
        value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=reject_duplicate_keys)
    except FileNotFoundError:
        fail(f"Missing performance evidence file: {path}")
    except json.JSONDecodeError as error:
        fail(f"Malformed JSON in {path}: {error}")
    if not isinstance(value, dict):
        fail(f"{path} must contain one JSON object.")
    return value


def exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        fail(f"{label} keys differ; missing={sorted(expected-actual)!r}, extra={sorted(actual-expected)!r}.")


def finite(value: Any, label: str, *, integer: bool = False) -> float | int:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        fail(f"{label} must be numeric.")
    if not math.isfinite(float(value)) or value < 0 or (integer and not isinstance(value, int)):
        fail(f"{label} must be finite and nonnegative{' integer' if integer else ''}.")
    return value


def sha256_file(path: Path) -> str:
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


def assert_measurement_sources_unchanged(source_sha: str) -> None:
    if re.fullmatch(r"[0-9a-f]{40}", source_sha) is None:
        fail(f"manifest.sourceSha is not a Git commit SHA: {source_sha!r}.")
    for relative in MEASUREMENT_SENSITIVE_PATHS:
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
    if sample["ordinal"] != ordinal:
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


def verify_measurement(path: Path, scenario: str, source_sha: str) -> dict[str, Any]:
    measurement = load_json(path)
    exact_keys(measurement, MEASUREMENT_KEYS, path.name)
    expected = {
        "schemaVersion": SCHEMA_VERSION, "baselineId": BASELINE_ID,
        "sourceSha": source_sha, "instrumentationVersion": INSTRUMENTATION_VERSION,
        "fixtureVersion": FIXTURE_VERSION, "scenarioId": scenario, "scenarioVersion": scenario,
    }
    for key, value in expected.items():
        if measurement[key] != value: fail(f"{path.name}.{key} drifted: expected {value!r}, received {measurement[key]!r}.")
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
        if manifest[key] != value: fail(f"manifest.{key} drifted: expected {value!r}, received {manifest[key]!r}.")
    if manifest["scenarioVersions"] != {scenario: scenario for scenario in SCENARIOS}:
        fail("Manifest scenario versions are missing, duplicated, or stale.")
    environment = manifest["captureEnvironment"]
    if not isinstance(environment, dict): fail("captureEnvironment must be an object.")
    exact_keys(environment, {"viewport", "theme", "scalePercent", "motion", "materials", "pointer", "devicePixelRatio", "display", "applicationSha256", "python", "platform", "tauriDriver", "webkitWebDriver", "xvfb", "reactProfilingBuild"}, "captureEnvironment")
    if environment["viewport"] != VIEWPORT or environment["display"] != DISPLAY:
        fail("Capture viewport/display metadata drifted from the reference contract.")
    if {key: environment[key] for key in ("theme", "scalePercent", "motion", "materials", "pointer", "reactProfilingBuild")} != {"theme": "light", "scalePercent": 100, "motion": "full", "materials": "rich", "pointer": "fine", "reactProfilingBuild": True}:
        fail("Capture runtime metadata drifted from the fixed reference contract.")
    finite(environment["devicePixelRatio"], "captureEnvironment.devicePixelRatio")
    if re.fullmatch(r"[0-9a-f]{64}", str(environment["applicationSha256"])) is None:
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
    if budgets["schemaVersion"] != SCHEMA_VERSION or budgets["baselineId"] != BASELINE_ID or budgets["sourceSha"] != source_sha or budgets["instrumentationVersion"] != INSTRUMENTATION_VERSION:
        fail("Budget identity is stale.")
    if budgets["scenarioVersions"] != manifest["scenarioVersions"] or budgets["rounding"] != "milliseconds-to-3-decimal-places": fail("Budget scenario or rounding contract drifted.")
    expected_entries = expected_budget_entries(measurements)
    if budgets["entries"] != expected_entries: fail("Budgets are stale or were not derived exactly from the baseline distribution.")
    expected_files = {"manifest.json", "budgets.json", *(canonical_measurement_path(s) for s in SCENARIOS)}
    actual_files = {str(path.relative_to(baseline_dir)) for path in baseline_dir.rglob("*") if path.is_file()}
    if actual_files != expected_files: fail(f"Baseline file set differs; missing={sorted(expected_files-actual_files)!r}, extra={sorted(actual_files-expected_files)!r}.")
    return {"manifest": manifest, "measurements": measurements, "budgets": budgets}


def metric_values(measurement: dict[str, Any], metric: str) -> list[float]:
    if metric == "avoidableReactCommitCount": return [float(s["react"]["avoidableCommitCount"]) for s in measurement["samples"]]
    if metric == "interactionToVisibleMs": return [float(s["interactionToVisible"]["valueMs"]) for s in measurement["samples"]]
    if metric == "maxPointerFrameWorkMs": return [float(s["pointer"]["maxFrameWorkMs"]) for s in measurement["samples"]]
    fail(f"Unknown budget metric {metric!r}.")


def compare_baseline(candidate_dir: Path, baseline_dir: Path = BASELINE_DIR, *, expected_source_sha: str | None = None) -> list[str]:
    if expected_source_sha is not None:
        baseline = verify_baseline(baseline_dir, expected_source_sha=expected_source_sha)
        candidate = verify_baseline(candidate_dir, expected_source_sha=expected_source_sha)
    else:
        baseline_manifest = load_json(baseline_dir / "manifest.json")
        baseline = verify_baseline(baseline_dir, expected_source_sha=baseline_manifest.get("sourceSha"))
        candidate = verify_baseline(candidate_dir)
    compatible_keys = ("viewport", "theme", "scalePercent", "motion", "materials", "pointer", "display", "python", "platform", "tauriDriver", "webkitWebDriver", "xvfb", "reactProfilingBuild")
    base_env = baseline["manifest"]["captureEnvironment"]
    candidate_env = candidate["manifest"]["captureEnvironment"]
    for key in compatible_keys:
        if candidate_env[key] != base_env[key]: fail(f"Candidate runtime metadata is not comparable: {key} differs.")
    failures: list[str] = []
    for entry in baseline["budgets"]["entries"]:
        value = max(metric_values(candidate["measurements"][entry["scenarioId"]], entry["metric"]))
        if value > float(entry["budget"]):
            failures.append(f"{entry['scenarioId']}:{entry['metric']} maximum {value:.3f} exceeds budget {float(entry['budget']):.3f}")
    return failures


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
    if not isinstance(sample, dict): raise AssertionError(f"Invalid performance sample: {sample!r}")
    return {
        "interactionToVisible": {"state": "available", "valueMs": sample["interactionToVisibleMs"]},
        "interactionToReady": {"state": "available", "valueMs": sample["interactionToReadyMs"]},
        "userTiming": sample["userTiming"],
        "react": {"state": "available", **sample["react"]},
        "pointer": sample["pointer"],
    }


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
    touch_drag: bool = False,
    release: bool = True,
) -> None:
    pointer_type = "touch" if touch_drag else "mouse"
    actions: list[dict[str, Any]] = [{"type": "pointerMove", "duration": 0, "origin": {webdriver.ELEMENT_KEY: element_id}, "x": 0, "y": 0}]
    if touch_drag:
        actions.extend([
            {"type": "pointerDown", "button": 0},
            {"type": "pointerMove", "duration": 80, "origin": "pointer", "x": 24, "y": 16},
            {"type": "pointerMove", "duration": 80, "origin": "viewport", "x": 1400, "y": 850},
            {"type": "pointerCancel"},
        ])
    webdriver.request("POST", f"/session/{session_id}/actions", {"actions": [{"type": "pointer", "id": f"stage0-{pointer_type}", "parameters": {"pointerType": pointer_type}, "actions": actions}]})
    if release:
        release_pointer_actions(session_id)


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
        target = webdriver.find_element(session_id, "css selector", "main [aria-label^='Reorder '][role='button']", deadline)
        arm(session_id, scenario, target); pointer_actions(session_id, target, touch_drag=True)
        wait_js(session_id, "document.querySelectorAll('[data-drag-over]').length === 0", deadline)
    elif scenario == "deck-selection-v1":
        navigate(session_id, "practice", "Practice")
        target = webdriver.find_element(session_id, "xpath", "//main//h2[normalize-space(.)='Select decks to study']/following::input[@type='checkbox'][1]", deadline)
        arm(session_id, scenario, target); webdriver.click(session_id, target)
        wait_js(session_id, "!document.body.textContent.includes('Updating…') && document.body.textContent.includes('selected')", deadline)
    elif scenario == "study-guide-open-v1":
        navigate(session_id, "practice", "Practice")
        target = webdriver.find_element(session_id, "xpath", "//main//button[normalize-space(.)='Open study guide']", deadline)
        arm(session_id, scenario, target); webdriver.click(session_id, target)
        wait_js(session_id, "document.querySelector('[role=dialog]') && !document.querySelector('[role=dialog]').textContent.includes('Loading study guide')", deadline)
    else:
        if sample_index == 0:
            navigate(session_id, "practice", "Practice")
            start = webdriver.find_element(session_id, "xpath", "//main//button[normalize-space(.)='Start scheduled practice']", deadline)
            webdriver.click(session_id, start)
        wait_js(session_id, "document.body.textContent.includes('Show answer')", deadline)
        reveal = webdriver.find_element(session_id, "xpath", "//main//button[normalize-space(.)='Show answer']", deadline)
        webdriver.click(session_id, reveal)
        rate = webdriver.find_element(session_id, "xpath", "//main//button[contains(normalize-space(.),'Easy')]", deadline)
        arm(session_id, scenario, rate); webdriver.click(session_id, rate)
        wait_js(session_id, f"document.body.textContent.includes('{sample_index + 1} studied')", deadline)
    result = finish(session_id)
    # Reset non-durable interactions deterministically.
    if scenario == "navigation-hover-v1":
        release_pointer_actions(session_id)
    elif scenario == "document-list-interaction-v1":
        webdriver.click(session_id, target)
        wait_js(session_id, "document.querySelectorAll('main article').length === 1", deadline)
    elif scenario == "deck-selection-v1":
        webdriver.click(session_id, target)
        wait_js(session_id, "!document.body.textContent.includes('Updating…')", deadline)
    elif scenario == "study-guide-open-v1":
        close = webdriver.find_element(session_id, "css selector", "[role='dialog'] button[aria-label='Close dialog']", deadline)
        webdriver.click(session_id, close)
        wait_js(session_id, "!document.querySelector('[role=dialog]')", deadline)
    return result


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


def capture(application: Path, baseline_dir: Path, source_sha: str) -> None:
    baseline_dir.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix=f".{baseline_dir.name}.capture-", dir=baseline_dir.parent))
    (staging / "measurements").mkdir()
    session_id: str | None = None
    with tempfile.NamedTemporaryFile("w+", encoding="utf-8", delete=False) as log:
        log_path = Path(log.name)
        driver = subprocess.Popen(["tauri-driver"], stdout=log, stderr=subprocess.STDOUT, text=True, start_new_session=True)
    try:
        webdriver.wait_for_driver(webdriver.deadline_after(20))
        session_id = webdriver.create_session(application.resolve())
        webdriver.verify_shell(session_id, webdriver.deadline_after())
        achieve_reference_viewport(session_id)
        wait_js(session_id, "window.__OSTUDY_UI_PERFORMANCE__?.profilingRenderer === true", webdriver.deadline_after(20))
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
        for scenario in SCENARIOS:
            recorded: list[dict[str, Any]] = []
            for index in range(WARMUP_COUNT + SAMPLE_COUNT):
                sample = collect_scenario(session_id, scenario, index if scenario == "active-practice-progress-v1" else 0)
                if index >= WARMUP_COUNT:
                    sample["ordinal"] = index - WARMUP_COUNT + 1
                    recorded.append(sample)
            measurement = {"schemaVersion": SCHEMA_VERSION, "baselineId": BASELINE_ID, "sourceSha": source_sha, "instrumentationVersion": INSTRUMENTATION_VERSION, "fixtureVersion": FIXTURE_VERSION, "scenarioId": scenario, "scenarioVersion": scenario, "samples": recorded}
            path = staging / canonical_measurement_path(scenario)
            path.write_text(json.dumps(measurement, indent=2) + "\n", encoding="utf-8")
            measurements[scenario] = measurement
        budgets = {"schemaVersion": SCHEMA_VERSION, "baselineId": BASELINE_ID, "sourceSha": source_sha, "instrumentationVersion": INSTRUMENTATION_VERSION, "scenarioVersions": {s: s for s in SCENARIOS}, "rounding": "milliseconds-to-3-decimal-places", "entries": expected_budget_entries(measurements)}
        budgets_path = staging / "budgets.json"; budgets_path.write_text(json.dumps(budgets, indent=2) + "\n", encoding="utf-8")
        environment = {**resolved, "display": DISPLAY, "applicationSha256": sha256_file(application), "python": platform.python_version(), "platform": platform.platform(), "tauriDriver": run_version(["tauri-driver", "--version"], "unavailable"), "webkitWebDriver": run_version(["WebKitWebDriver", "--version"], "unavailable"), "xvfb": run_version(["Xvfb", "-version"], "unavailable"), "reactProfilingBuild": True}
        refs = [{"scenarioId": s, "path": canonical_measurement_path(s), "sha256": sha256_file(staging / canonical_measurement_path(s))} for s in SCENARIOS]
        manifest = {"schemaVersion": SCHEMA_VERSION, "baselineId": BASELINE_ID, "sourceSha": source_sha, "instrumentationVersion": INSTRUMENTATION_VERSION, "fixtureVersion": FIXTURE_VERSION, "warmupCount": WARMUP_COUNT, "sampleCount": SAMPLE_COUNT, "scenarioVersions": {s: s for s in SCENARIOS}, "captureEnvironment": environment, "measurements": refs, "budgets": {"path": "budgets.json", "sha256": sha256_file(budgets_path)}}
        (staging / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        verify_baseline(staging, expected_source_sha=source_sha)
        install_staged_baseline(staging, baseline_dir)
        staging = Path()
        update_roadmap_after_capture()
    except Exception:
        try: print(log_path.read_text(encoding="utf-8"))
        except OSError: pass
        raise
    finally:
        if session_id:
            try: webdriver.delete_session(session_id)
            except Exception: pass
        if driver.poll() is None:
            os.killpg(driver.pid, signal.SIGTERM)
            try: driver.wait(timeout=5)
            except subprocess.TimeoutExpired: os.killpg(driver.pid, signal.SIGKILL); driver.wait(timeout=5)
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
    compare = sub.add_parser("compare"); compare.add_argument("--baseline-dir", type=Path, default=BASELINE_DIR); compare.add_argument("--candidate-dir", type=Path, required=True); compare.add_argument("--source-sha")
    capture_parser = sub.add_parser("capture"); capture_parser.add_argument("--baseline-dir", type=Path, default=BASELINE_DIR); capture_parser.add_argument("--application", type=Path, required=True); capture_parser.add_argument("--source-sha", required=True)
    args = parser.parse_args()
    if args.command == "verify": verify_baseline(args.baseline_dir, expected_source_sha=args.source_sha); print("Verified Stage 0D performance baseline (read-only).")
    elif args.command == "compare":
        failures = compare_baseline(args.candidate_dir, args.baseline_dir, expected_source_sha=args.source_sha)
        if failures: raise PerformanceBaselineError("Performance budget comparison failed:\n" + "\n".join(failures))
        print("Candidate satisfies all comparable Stage 0D budgets (read-only).")
    else: capture(args.application, args.baseline_dir, args.source_sha); print("Captured and verified Stage 0D performance baseline.")
    return 0


if __name__ == "__main__": raise SystemExit(main())
