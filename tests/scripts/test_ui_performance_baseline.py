from __future__ import annotations

import copy
import hashlib
import importlib.util
import json
import math
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock


MODULE_PATH = Path(__file__).resolve().parents[1] / "webdriver" / "ui_performance_baseline.py"
sys.path.insert(0, str(MODULE_PATH.parent))
SPEC = importlib.util.spec_from_file_location("ui_performance_baseline", MODULE_PATH)
assert SPEC and SPEC.loader
baseline = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(baseline)
SOURCE_SHA = "a" * 40


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sample(ordinal: int, scenario: str, *, offset: float = 0.0) -> dict:
    pointer = scenario in baseline.POINTER_SCENARIOS
    boundary_id = sorted(baseline.EXPECTED_BOUNDARIES[scenario])[0]
    visible_duration = 4.0 + ordinal / 10 + offset
    ready_duration = 8.0 + ordinal / 10 + offset
    return {
        "ordinal": ordinal,
        "interactionToVisible": {"state": "available", "valueMs": visible_duration},
        "interactionToReady": {"state": "available", "valueMs": ready_duration},
        "userTiming": {"startTimeMs": 10.0, "visibleTimeMs": 10.0 + visible_duration, "readyTimeMs": 10.0 + ready_duration},
        "react": {
            "state": "available", "commitCount": 1, "totalRenderDurationMs": 0.5,
            "avoidableCommitCount": 0,
            "commits": [{"boundaryId": boundary_id, "phase": "update", "actualDurationMs": 0.5, "baseDurationMs": 1.0, "startTimeMs": 10.0, "commitTimeMs": 12.0}],
        },
        "pointer": {
            "state": "available" if pointer else "notApplicable",
            "eventCount": 2 if pointer else None,
            "frameCount": 1 if pointer else None,
            "maxFrameWorkMs": 0.3 + offset if pointer else None,
            "maxFrameIntervalMs": 16.667 if pointer else None,
        },
    }


def browser_sample() -> dict:
    return {
        "interactionToVisibleMs": 4.0,
        "interactionToReadyMs": 8.0,
        "userTiming": {"startTimeMs": 10.0, "visibleTimeMs": 14.0, "readyTimeMs": 18.0},
        "react": {
            "commitCount": 1,
            "totalRenderDurationMs": 0.5,
            "avoidableCommitCount": 0,
            "commits": [],
        },
        "pointer": {
            "state": "available",
            "eventCount": 1,
            "frameCount": 1,
            "maxFrameWorkMs": 0.3,
            "maxFrameIntervalMs": 16.667,
        },
    }


def write_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def make_tree(root: Path, *, offset: float = 0.0) -> None:
    measurements = {}
    refs = []
    for scenario in baseline.SCENARIOS:
        measurement = {
            "schemaVersion": 1, "baselineId": baseline.BASELINE_ID, "sourceSha": SOURCE_SHA,
            "instrumentationVersion": baseline.INSTRUMENTATION_VERSION,
            "fixtureVersion": baseline.FIXTURE_VERSION, "scenarioId": scenario,
            "scenarioVersion": scenario,
            "samples": [sample(index, scenario, offset=offset) for index in range(1, 13)],
        }
        path = root / baseline.canonical_measurement_path(scenario)
        write_json(path, measurement)
        measurements[scenario] = measurement
        refs.append({"scenarioId": scenario, "path": baseline.canonical_measurement_path(scenario), "sha256": digest(path)})
    budgets = {
        "schemaVersion": 1, "baselineId": baseline.BASELINE_ID, "sourceSha": SOURCE_SHA,
        "instrumentationVersion": baseline.INSTRUMENTATION_VERSION,
        "scenarioVersions": {scenario: scenario for scenario in baseline.SCENARIOS},
        "rounding": "milliseconds-to-3-decimal-places",
        "entries": baseline.expected_budget_entries(measurements),
    }
    budget_path = root / "budgets.json"; write_json(budget_path, budgets)
    environment = {
        "viewport": baseline.VIEWPORT, "theme": "light", "scalePercent": 100,
        "motion": "full", "materials": "rich", "pointer": "fine", "devicePixelRatio": 1,
        "display": baseline.DISPLAY, "applicationSha256": "b" * 64, "python": "3.12.0",
        "platform": "codex-dev", "tauriDriver": "2.0", "webkitWebDriver": "2.48",
        "xvfb": "21.1", "reactProfilingBuild": True,
    }
    manifest = {
        "schemaVersion": 1, "baselineId": baseline.BASELINE_ID, "sourceSha": SOURCE_SHA,
        "instrumentationVersion": baseline.INSTRUMENTATION_VERSION,
        "fixtureVersion": baseline.FIXTURE_VERSION, "warmupCount": 3, "sampleCount": 12,
        "scenarioVersions": {scenario: scenario for scenario in baseline.SCENARIOS},
        "captureEnvironment": environment, "measurements": refs,
        "budgets": {"path": "budgets.json", "sha256": digest(budget_path)},
    }
    write_json(root / "manifest.json", manifest)


class PerformanceBaselineContractTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / "baseline"
        make_tree(self.root)

    def tearDown(self) -> None:
        self.temp.cleanup()

    def rewrite_manifest(self, mutate) -> None:
        path = self.root / "manifest.json"; value = json.loads(path.read_text()); mutate(value); write_json(path, value)

    def rewrite_measurement(self, scenario, mutate) -> None:
        path = self.root / baseline.canonical_measurement_path(scenario); value = json.loads(path.read_text()); mutate(value); write_json(path, value)
        self.rewrite_manifest(lambda manifest: next(entry for entry in manifest["measurements"] if entry["scenarioId"] == scenario).__setitem__("sha256", digest(path)))

    def test_valid_contract_and_self_comparison_are_read_only(self) -> None:
        before = {path.relative_to(self.root): path.read_bytes() for path in self.root.rglob("*") if path.is_file()}
        baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)
        self.assertEqual([], baseline.compare_baseline(self.root, self.root, expected_source_sha=SOURCE_SHA))
        self.assertEqual(before, {path.relative_to(self.root): path.read_bytes() for path in self.root.rglob("*") if path.is_file()})

    def test_duplicate_json_key_is_rejected(self) -> None:
        path = self.root / "manifest.json"; path.write_text('{"schemaVersion":1,"schemaVersion":1}', encoding="utf-8")
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "Duplicate JSON key"):
            baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)

    def test_missing_and_duplicate_scenarios_are_rejected(self) -> None:
        self.rewrite_manifest(lambda value: value["measurements"].pop())
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "exactly six"):
            baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)
        make_tree(self.root)
        self.rewrite_manifest(lambda value: value["measurements"].__setitem__(-1, copy.deepcopy(value["measurements"][0])))
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "Duplicate or unknown"):
            baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)

    def test_nonfinite_and_unavailable_required_metrics_are_rejected(self) -> None:
        scenario = baseline.SCENARIOS[0]
        self.rewrite_measurement(scenario, lambda value: value["samples"][0]["interactionToVisible"].update({"valueMs": math.inf}))
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "finite"):
            baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)
        make_tree(self.root)
        self.rewrite_measurement(scenario, lambda value: value["samples"][0]["react"].update({"state": "unavailable"}))
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "Profiler evidence"):
            baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)

    def test_metric_applicability_is_strict(self) -> None:
        scenario = "deck-selection-v1"
        self.rewrite_measurement(scenario, lambda value: value["samples"][0]["pointer"].update({"state": "available", "eventCount": 1, "frameCount": 1, "maxFrameWorkMs": 1, "maxFrameIntervalMs": 16}))
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "notApplicable"):
            baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)

    def test_source_scenario_and_sample_ordinal_drift_are_rejected(self) -> None:
        self.rewrite_manifest(lambda value: value.update({"sourceSha": "c" * 40}))
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "sourceSha drifted"):
            baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)
        make_tree(self.root)
        scenario = baseline.SCENARIOS[1]
        self.rewrite_measurement(scenario, lambda value: value["samples"][1].update({"ordinal": 1}))
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "ordinal"):
            baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)

    def test_hash_path_and_extra_file_mismatch_are_rejected(self) -> None:
        measurement = self.root / baseline.canonical_measurement_path(baseline.SCENARIOS[0])
        measurement.write_text(measurement.read_text() + " ", encoding="utf-8")
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "hash mismatch"):
            baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)
        make_tree(self.root)
        self.rewrite_manifest(lambda value: value["measurements"][0].update({"path": "../escape.json"}))
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "Noncanonical"):
            baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)
        make_tree(self.root); (self.root / "unexpected.json").write_text("{}")
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "file set differs"):
            baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)

    def test_stale_budget_is_rejected(self) -> None:
        path = self.root / "budgets.json"; value = json.loads(path.read_text()); value["entries"][0]["budget"] += 1; write_json(path, value)
        self.rewrite_manifest(lambda manifest: manifest["budgets"].update({"sha256": digest(path)}))
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "Budgets are stale"):
            baseline.verify_baseline(self.root, expected_source_sha=SOURCE_SHA)

    def test_comparison_reports_budget_failure_and_runtime_drift(self) -> None:
        candidate = Path(self.temp.name) / "candidate"; make_tree(candidate, offset=100)
        failures = baseline.compare_baseline(candidate, self.root, expected_source_sha=SOURCE_SHA)
        self.assertTrue(any("interactionToVisibleMs" in failure for failure in failures))
        manifest_path = candidate / "manifest.json"; value = json.loads(manifest_path.read_text()); value["captureEnvironment"]["platform"] = "different"; write_json(manifest_path, value)
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "not comparable"):
            baseline.compare_baseline(candidate, self.root, expected_source_sha=SOURCE_SHA)

    def test_capture_updates_only_the_two_stage_zero_rows_and_is_idempotent(self) -> None:
        roadmap = Path(self.temp.name) / "roadmap.md"
        roadmap.write_text(
            "\n".join([
                "- [ ] Add reproducible User Timing/React Profiler scenarios for navigation hover, document-list interaction, deck drag, deck selection, guide open, and active-practice progress.",
                "- [ ] Record initial interaction traces and define budgets for avoidable React commits, pointer-frame work, and visible hover/drag latency.",
                "- [ ] Stage 4 performance remains open.",
                "- [ ] Stage 12 performance remains open.",
                "- [ ] Stage 13 human acceptance remains open.",
            ]) + "\n",
            encoding="utf-8",
        )
        baseline.update_roadmap_after_capture(roadmap)
        first = roadmap.read_text(encoding="utf-8")
        baseline.update_roadmap_after_capture(roadmap)
        self.assertEqual(first, roadmap.read_text(encoding="utf-8"))
        self.assertEqual(2, first.count("- [x]"))
        self.assertEqual(3, first.count("remains open"))

    def test_capture_bootstrap_is_static_and_remains_compile_time_gated(self) -> None:
        source = (baseline.ROOT_DIR / "src/main.tsx").read_text(encoding="utf-8")
        self.assertIn(
            'import { installUiPerformanceBridge } from "./testing/uiPerformanceBridge";',
            source,
        )
        self.assertIn("__OSTUDY_UI_PERFORMANCE_CAPTURE__", source)
        self.assertNotIn('import("./testing/uiPerformanceBridge")', source)

    def test_shared_webdriver_helper_is_measurement_sensitive(self) -> None:
        self.assertIn("tests/webdriver/smoke.py", baseline.MEASUREMENT_SENSITIVE_PATHS)

    def test_finish_recovers_one_disconnect_and_returns_the_cached_sample(self) -> None:
        with mock.patch.object(
            baseline.webdriver,
            "request",
            side_effect=[
                ConnectionResetError("lost finalization response"),
                {"ok": True, "sample": browser_sample()},
            ],
        ) as request:
            result = baseline.finish("session-1")

        self.assertEqual(2, request.call_count)
        method, path, payload = request.call_args_list[0].args
        self.assertEqual("POST", method)
        self.assertEqual("/session/session-1/execute/async", path)
        self.assertIn("bridge.finish()", payload["script"])
        self.assertEqual([], payload["args"])
        self.assertEqual(4.0, result["interactionToVisible"]["valueMs"])

    def test_finish_propagates_structured_browser_errors_without_retry(self) -> None:
        with mock.patch.object(
            baseline.webdriver,
            "request",
            return_value={
                "ok": False,
                "error": {"name": "Error", "message": "No visible-response mutation was observed."},
            },
        ) as request:
            with self.assertRaisesRegex(
                baseline.BrowserPerformanceError,
                "No visible-response mutation was observed",
            ) as raised:
                baseline.finish("session-1")

        self.assertEqual("Error", raised.exception.name)
        request.assert_called_once()

    def test_read_only_execution_recovers_one_connection_reset_only(self) -> None:
        with mock.patch.object(
            baseline,
            "execute",
            side_effect=[ConnectionResetError("reset"), True],
        ) as execute:
            self.assertTrue(baseline.execute_read_only("session-1", "return window.ready"))
        self.assertEqual(2, execute.call_count)

        with mock.patch.object(
            baseline,
            "execute",
            side_effect=baseline.urllib.error.URLError("non-transport failure"),
        ) as execute, mock.patch.object(baseline.time, "sleep"):
            with self.assertRaises(baseline.urllib.error.URLError):
                baseline.execute_read_only("session-1", "return window.ready")
        execute.assert_called_once()

        with mock.patch.object(
            baseline,
            "execute",
            side_effect=[ConnectionResetError("reset"), ConnectionResetError("reset again")],
        ) as execute:
            with self.assertRaises(ConnectionResetError):
                baseline.execute_read_only("session-1", "return window.ready")
        self.assertEqual(2, execute.call_count)

    def test_finish_retries_only_transient_disconnects_once(self) -> None:
        with mock.patch.object(
            baseline.webdriver,
            "request",
            side_effect=[ConnectionResetError("reset"), ConnectionResetError("reset again")],
        ) as request:
            with self.assertRaises(ConnectionResetError):
                baseline.finish("session-1")
        self.assertEqual(2, request.call_count)

        failure = baseline.webdriver.WebDriverProtocolError(500, "unknown error", "browser assertion")
        with mock.patch.object(
            baseline.webdriver,
            "request",
            side_effect=failure,
        ) as request:
            with self.assertRaises(baseline.webdriver.WebDriverProtocolError):
                baseline.finish("session-1")
        request.assert_called_once()

    def test_mutating_bridge_commands_are_not_retried_after_disconnect(self) -> None:
        with mock.patch.object(
            baseline,
            "execute",
            side_effect=ConnectionResetError("ambiguous arm delivery"),
        ) as execute:
            with self.assertRaises(ConnectionResetError):
                baseline.arm("session-1", "navigation-hover-v1", "element-1")
        execute.assert_called_once()

        with mock.patch.object(
            baseline.webdriver,
            "request",
            side_effect=ConnectionResetError("ambiguous pointer delivery"),
        ) as request:
            with self.assertRaises(ConnectionResetError):
                baseline.park_mouse("session-1")
        request.assert_called_once()

    def test_navigation_reuses_the_current_route_without_an_extra_click(self) -> None:
        with mock.patch.object(baseline, "execute_read_only", return_value="Flashcards") as read, mock.patch.object(
            baseline.webdriver,
            "find_element",
        ) as find_element, mock.patch.object(baseline.webdriver, "click") as click:
            baseline.navigate("session-1", "decks", "Flashcards")

        read.assert_called_once()
        find_element.assert_not_called()
        click.assert_not_called()

    def test_hover_pointer_is_parked_deterministically_and_can_remain_held(self) -> None:
        with mock.patch.object(baseline.webdriver, "request", return_value=None) as request:
            baseline.park_mouse("session-1")

        request.assert_called_once()
        _, _, payload = request.call_args.args
        move = payload["actions"][0]["actions"][0]
        self.assertEqual("viewport", move["origin"])
        self.assertEqual(baseline.VIEWPORT["width"] - 40, move["x"])
        self.assertEqual(baseline.VIEWPORT["height"] - 40, move["y"])

        with mock.patch.object(baseline.webdriver, "request", return_value=None) as request:
            baseline.pointer_actions("session-1", "element-1", release=False)

        request.assert_called_once()
        self.assertEqual("POST", request.call_args.args[0])

    def test_navigation_hover_parks_before_arm_and_finalizes_after_hover(self) -> None:
        events: list[str] = []
        with mock.patch.object(baseline, "navigate"), mock.patch.object(
            baseline.webdriver,
            "find_element",
            return_value="analytics-element",
        ), mock.patch.object(
            baseline,
            "park_mouse",
            side_effect=lambda _session: events.append("park"),
        ), mock.patch.object(
            baseline,
            "arm",
            side_effect=lambda *_args: events.append("arm"),
        ), mock.patch.object(
            baseline,
            "pointer_actions",
            side_effect=lambda *_args, **_kwargs: events.append("pointer"),
        ) as pointer_actions, mock.patch.object(
            baseline,
            "wait_js",
            side_effect=lambda *_args: events.append("hover-ready"),
        ), mock.patch.object(
            baseline,
            "finish",
            side_effect=lambda _session: events.append("finish") or {"sample": True},
        ), mock.patch.object(
            baseline,
            "release_pointer_actions",
            side_effect=lambda _session: events.append("release"),
        ):
            result = baseline.collect_scenario("session-1", "navigation-hover-v1", 0)

        self.assertEqual(
            ["park", "arm", "pointer", "hover-ready", "finish", "release"],
            events,
        )
        pointer_actions.assert_called_once_with(
            "session-1",
            "analytics-element",
            release=False,
        )
        self.assertEqual({"sample": True}, result)


if __name__ == "__main__": unittest.main()
