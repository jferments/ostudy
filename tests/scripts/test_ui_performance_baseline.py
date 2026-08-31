from __future__ import annotations

import copy
import hashlib
import importlib.util
import inspect
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
TEST_PLATFORM = "Linux-6.8.0-137-generic-x86_64-with-glibc2.39"


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


def practice_state(
    *,
    eligible: int = 2,
    selected: int = 2,
    checked: bool | None = True,
    phase: str = "settled",
    start_enabled: bool = True,
    transition_observed: bool = False,
) -> dict:
    return {
        "targetFound": checked is not None,
        "eligibleDeckCount": eligible,
        "selectedDeckCount": selected,
        "targetChecked": checked,
        "workloadPhase": phase,
        "startFound": True,
        "startEnabled": start_enabled,
        "workloadTransitionObserved": transition_observed,
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
        "platform": TEST_PLATFORM, "tauriDriver": "2.0", "webkitWebDriver": "2.48",
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


def make_drag_candidate(root: Path, *, offset: float = 0.0) -> None:
    scenario = baseline.CANDIDATE_SCENARIOS[0]
    samples = [sample(index, scenario, offset=offset) for index in range(1, 13)]
    if offset > 0:
        for candidate_sample in samples:
            candidate_sample["react"]["avoidableCommitCount"] = 1
            candidate_sample["react"]["commits"][0]["boundaryId"] = "navigation"
    measurement = {
        "schemaVersion": 1,
        "baselineId": baseline.CANDIDATE_ID,
        "sourceSha": SOURCE_SHA,
        "instrumentationVersion": baseline.INSTRUMENTATION_VERSION,
        "fixtureVersion": baseline.CANDIDATE_FIXTURE_VERSION,
        "scenarioId": scenario,
        "scenarioVersion": scenario,
        "samples": samples,
    }
    measurement_path = root / baseline.canonical_measurement_path(scenario)
    write_json(measurement_path, measurement)
    environment = {
        "viewport": baseline.VIEWPORT, "theme": "light", "scalePercent": 100,
        "motion": "full", "materials": "rich", "pointer": "fine", "devicePixelRatio": 1,
        "display": baseline.DISPLAY, "applicationSha256": "b" * 64, "python": "3.12.0",
        "platform": TEST_PLATFORM, "tauriDriver": "2.0", "webkitWebDriver": "2.48",
        "xvfb": "21.1", "reactProfilingBuild": True,
    }
    manifest = {
        "schemaVersion": 1,
        "baselineId": baseline.CANDIDATE_ID,
        "sourceSha": SOURCE_SHA,
        "instrumentationVersion": baseline.INSTRUMENTATION_VERSION,
        "fixtureVersion": baseline.CANDIDATE_FIXTURE_VERSION,
        "warmupCount": 3,
        "sampleCount": 12,
        "scenarioVersions": {scenario: scenario},
        "captureEnvironment": environment,
        "trustedNativeInput": copy.deepcopy(baseline.TRUSTED_NATIVE_INPUT_ATTESTATION),
        "measurements": [{
            "scenarioId": scenario,
            "path": baseline.canonical_measurement_path(scenario),
            "sha256": digest(measurement_path),
        }],
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

    def test_normal_verify_detects_source_drift_while_reference_verify_is_pinned(self) -> None:
        with mock.patch.object(
            baseline,
            "assert_measurement_sources_unchanged",
            side_effect=baseline.PerformanceBaselineError("current source drift"),
        ) as drift_guard:
            with self.assertRaisesRegex(baseline.PerformanceBaselineError, "current source drift"):
                baseline.verify_baseline(self.root)
            baseline.verify_reference(self.root)
        drift_guard.assert_called_once_with(SOURCE_SHA)

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
        manifest_path = candidate / "manifest.json"; value = json.loads(manifest_path.read_text()); value["captureEnvironment"]["platform"] = "Darwin-24.6.0-x86_64-with-glibc2.39"; write_json(manifest_path, value)
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "not comparable"):
            baseline.compare_baseline(candidate, self.root, expected_source_sha=SOURCE_SHA)

    def test_candidate_destination_rejects_unsafe_aliases_and_existing_paths(self) -> None:
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "absolute"):
            baseline.validate_candidate_destination(Path("relative-candidate"))
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "reference tree"):
            baseline.validate_candidate_destination(baseline.BASELINE_DIR / "candidate")
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "outside the repository"):
            baseline.validate_candidate_destination(baseline.ROOT_DIR / "candidate")
        existing = Path(self.temp.name) / "existing"; existing.mkdir()
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "must not already exist"):
            baseline.validate_candidate_destination(existing)
        real_parent = Path(self.temp.name) / "real"; real_parent.mkdir()
        alias_parent = Path(self.temp.name) / "alias"; alias_parent.symlink_to(real_parent, target_is_directory=True)
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "symlinked parent"):
            baseline.validate_candidate_destination(alias_parent / "candidate")

    def test_candidate_install_is_atomic_and_never_replaces_an_existing_destination(self) -> None:
        parent = Path(self.temp.name)
        staging = parent / ".candidate.capture"; staging.mkdir(); (staging / "manifest.json").write_text("verified")
        destination = parent / "candidate"
        baseline.install_staged_candidate(staging, destination)
        self.assertEqual("verified", (destination / "manifest.json").read_text())

        replacement = parent / ".candidate.replacement"; replacement.mkdir(); (replacement / "manifest.json").write_text("new")
        with mock.patch.object(
            baseline,
            "validate_candidate_destination",
            return_value=destination,
        ):
            with self.assertRaisesRegex(baseline.PerformanceBaselineError, "appeared during capture"):
                baseline.install_staged_candidate(replacement, destination)
        self.assertEqual("verified", (destination / "manifest.json").read_text())
        self.assertTrue(replacement.is_dir())

    def test_failed_candidate_capture_cleans_sibling_staging_without_installing(self) -> None:
        destination = Path(self.temp.name) / "candidate"
        application = Path(self.temp.name) / "ostudy"; application.write_text("binary")
        with mock.patch.object(baseline, "assert_clean_committed_head"), mock.patch.object(
            baseline.subprocess,
            "Popen",
            side_effect=RuntimeError("driver unavailable"),
        ):
            with self.assertRaisesRegex(RuntimeError, "driver unavailable"):
                baseline.capture(application, destination, SOURCE_SHA, candidate=True)
        self.assertFalse(destination.exists())
        self.assertEqual([], list(Path(self.temp.name).glob(".candidate.capture-*")))

    def test_candidate_capture_requires_the_exact_clean_committed_source(self) -> None:
        with mock.patch.object(baseline, "current_source_sha", return_value="b" * 40):
            with self.assertRaisesRegex(baseline.PerformanceBaselineError, "source identity"):
                baseline.assert_clean_committed_head(SOURCE_SHA)
        clean = mock.Mock(stdout="")
        dirty = mock.Mock(stdout=" M source.ts\n")
        with mock.patch.object(baseline, "current_source_sha", return_value=SOURCE_SHA), mock.patch.object(
            baseline.subprocess,
            "run",
            side_effect=[clean, dirty],
        ):
            baseline.assert_clean_committed_head(SOURCE_SHA)
            with self.assertRaisesRegex(baseline.PerformanceBaselineError, "clean committed HEAD"):
                baseline.assert_clean_committed_head(SOURCE_SHA)

    def test_large_library_drag_has_a_distinct_stable_row_identity(self) -> None:
        self.assertIn("data-library-row='deck'", baseline.LARGE_LIBRARY_DRAG_XPATH)
        self.assertIn("Large library drag source", baseline.LARGE_LIBRARY_DRAG_XPATH)
        source = (baseline.ROOT_DIR / "src-tauri/src/library_backup/performance_fixture_tests.rs").read_text()
        self.assertIn("for index in 0..120", source)

    def test_performance_fixture_generators_are_safe_in_normal_rust_suites(self) -> None:
        source = (
            baseline.ROOT_DIR
            / "src-tauri/src/library_backup/performance_fixture_tests.rs"
        ).read_text(encoding="utf-8")
        self.assertNotIn("#[ignore", source)
        self.assertIn('performance_fixture_app_data("stage-0")', source)
        self.assertIn('performance_fixture_app_data("stage-4a")', source)
        self.assertIn("cleanup_performance_fixture_app_data", source)
        launcher = (
            baseline.ROOT_DIR / "scripts/ui-performance-baselines.sh"
        ).read_text(encoding="utf-8")
        self.assertIn("-- --exact", launcher)
        self.assertNotIn("--ignored", launcher)

    def test_fresh_font_smoke_accepts_only_equivalent_cssom_quote_serializations(self) -> None:
        self.assertTrue(
            baseline.ui.is_expected_plex_family(
                "'IBM Plex Sans', var(--ostudy-font-family-system)"
            )
        )
        self.assertTrue(
            baseline.ui.is_expected_plex_family(
                '"IBM Plex Sans", var(--ostudy-font-family-system)'
            )
        )
        for invalid in (
            None,
            "IBM Plex Sans, var(--ostudy-font-family-system)",
            "var(--ostudy-font-family-system), 'IBM Plex Sans'",
            "'IBM Plex Sans'",
            "'IBM Plex Sans', sans-serif",
        ):
            with self.subTest(invalid=invalid):
                self.assertFalse(baseline.ui.is_expected_plex_family(invalid))

    def test_route_layer_has_definite_height_and_smoke_accounts_for_shell_insets(self) -> None:
        shell_css = (baseline.ROOT_DIR / "src/AppShell.module.css").read_text(
            encoding="utf-8"
        )
        route_block = shell_css.split(".routeLayer {", 1)[1].split("}", 1)[0]
        self.assertIn("height: 100%;", route_block)
        viewport_assertion = inspect.getsource(baseline.ui.assert_viewport_consumption)
        self.assertIn("shellPaddingTop", viewport_assertion)
        self.assertIn("shellPaddingBottom", viewport_assertion)
        self.assertIn("shell_padding_top", viewport_assertion)
        self.assertIn("shell_padding_bottom", viewport_assertion)

    def test_element_key_uses_the_targeted_webdriver_endpoint_without_a_click(self) -> None:
        with mock.patch.object(baseline.ui.webdriver, "request") as request:
            baseline.ui.send_element_key("session-id", "window-control-id", "\ue007")

        request.assert_called_once_with(
            "POST",
            "/session/session-id/element/window-control-id/value",
            {"text": "\ue007", "value": ["\ue007"]},
        )

    def test_native_window_keyboard_activation_targets_the_resolved_element(self) -> None:
        probe = {
            "token": "probe",
            "keydown": {"trusted": True, "key": " "},
            "keyup": {"trusted": True, "key": " "},
            "click": {"trusted": True, "detail": 0},
        }
        with mock.patch.object(
            baseline.ui,
            "wait_for_window_control",
            return_value="window-control-id",
        ), mock.patch.object(
            baseline.ui,
            "execute",
            return_value=True,
        ) as execute, mock.patch.object(
            baseline.ui,
            "send_element_key",
        ) as send_element_key, mock.patch.object(
            baseline.ui,
            "arm_window_control_keyboard_probe",
        ) as arm_probe, mock.patch.object(
            baseline.ui,
            "wait_for_window_control_keyboard_probe",
            return_value=probe,
        ) as wait_probe, mock.patch.object(
            baseline.ui,
            "send_key",
        ) as send_global_key, mock.patch.object(
            baseline.ui.webdriver,
            "request",
            return_value={"width": 1440, "height": 900},
        ) as request:
            result = baseline.ui.activate_window_control_with_keyboard(
                "session-id",
                "Maximize window",
                False,
                123.0,
            )

        execute.assert_called_once_with(
            "session-id",
            "arguments[0].focus(); return document.activeElement === arguments[0];",
            [{baseline.ui.webdriver.ELEMENT_KEY: "window-control-id"}],
        )
        request.assert_called_once_with("GET", "/session/session-id/window/rect")
        arm_probe.assert_called_once()
        send_element_key.assert_called_once_with(
            "session-id", "window-control-id", baseline.ui.SPACE_KEY
        )
        wait_probe.assert_called_once()
        send_global_key.assert_not_called()
        self.assertEqual({"width": 1440, "height": 900}, result["initialWindowRect"])

    def test_native_keyboard_probe_requires_trusted_space_and_keyboard_click(self) -> None:
        complete = {
            "token": "probe",
            "keydown": {"trusted": True, "key": " "},
            "keyup": {"trusted": True, "key": " "},
            "click": {"trusted": True, "detail": 0},
        }
        with mock.patch.object(
            baseline.ui,
            "read_window_control_keyboard_probe",
            side_effect=[
                {**complete, "click": {"trusted": False, "detail": 0}},
                complete,
            ],
        ) as read_probe, mock.patch.object(
            baseline.ui,
            "execute",
            return_value=True,
        ) as execute, mock.patch.object(baseline.ui.time, "sleep"):
            result = baseline.ui.wait_for_window_control_keyboard_probe(
                "session-id", "probe", baseline.ui.time.monotonic() + 5
            )

        self.assertEqual(complete, result)
        self.assertEqual(2, read_probe.call_count)
        execute.assert_called_once_with(
            "session-id",
            "window.__ostudyWindowControlProbeCleanup?.(); return true;",
        )

    def test_native_keyboard_probe_rejects_boolean_click_detail(self) -> None:
        invalid = {
            "token": "probe",
            "keydown": {"trusted": True, "key": " "},
            "keyup": {"trusted": True, "key": " "},
            "click": {"trusted": True, "detail": False},
        }
        with mock.patch.object(
            baseline.ui,
            "read_window_control_keyboard_probe",
            return_value=invalid,
        ), mock.patch.object(
            baseline.ui.time,
            "monotonic",
            side_effect=[0.0, 0.0, 0.1],
        ), mock.patch.object(baseline.ui.time, "sleep"):
            with self.assertRaisesRegex(AssertionError, "trusted Space"):
                baseline.ui.wait_for_window_control_keyboard_probe(
                    "session-id", "probe", 0.05
                )

    def test_native_launchers_declare_and_verify_an_ewmh_window_manager(self) -> None:
        setup = (baseline.ROOT_DIR / "scripts/setup-ubuntu.sh").read_text(
            encoding="utf-8"
        )
        bootstrap = (
            baseline.ROOT_DIR / "scripts/runner/bootstrap-unified-guest.sh"
        ).read_text(encoding="utf-8")
        preflight = (
            baseline.ROOT_DIR / "scripts/runner/guest-preflight.sh"
        ).read_text(encoding="utf-8")
        doctor = (baseline.ROOT_DIR / "scripts/doctor.sh").read_text(
            encoding="utf-8"
        )
        smoke = (baseline.ROOT_DIR / "scripts/smoke-native.sh").read_text(
            encoding="utf-8"
        )
        performance = (
            baseline.ROOT_DIR / "scripts/ui-performance-baselines.sh"
        ).read_text(encoding="utf-8")

        for provisioning in (setup, bootstrap):
            self.assertIn("openbox", provisioning)
            self.assertIn("x11-utils", provisioning)
        for verification in (preflight, doctor):
            self.assertIn("openbox", verification)
            self.assertIn("xprop", verification)
        self.assertIn("command -v openbox", smoke)
        self.assertIn("command -v xprop", smoke)
        self.assertIn(
            'exec env OSTUDY_IN_XVFB=1 xvfb-run -a "$0" "$@"',
            smoke,
        )
        for launcher in (smoke, performance):
            self.assertIn("openbox --sm-disable", launcher)
            self.assertIn("_NET_SUPPORTING_WM_CHECK", launcher)
            self.assertIn("window_manager_ready", launcher)
        self.assertIn("OSTUDY_PERFORMANCE_IN_XVFB", performance)
        self.assertIn("UI performance capture cannot qualify a throttled rendering surface", performance)

    def test_drag_candidate_has_distinct_identity_and_only_three_drag_limits(self) -> None:
        candidate = Path(self.temp.name) / "candidate"
        make_drag_candidate(candidate, offset=100)
        verified = baseline.verify_drag_candidate(candidate, expected_source_sha=SOURCE_SHA)
        self.assertEqual(baseline.CANDIDATE_ID, verified["manifest"]["baselineId"])
        self.assertEqual({"deck-drag-v1"}, set(verified["measurements"]))
        failures = baseline.compare_drag_candidate(
            candidate,
            self.root,
            expected_candidate_source_sha=SOURCE_SHA,
        )
        self.assertEqual(3, len(failures))
        self.assertTrue(all(failure.startswith("deck-drag-v1:") for failure in failures))

    def test_drag_candidate_requires_exact_trusted_native_input_attestation(self) -> None:
        candidate = Path(self.temp.name) / "candidate-attestation"
        make_drag_candidate(candidate)
        manifest_path = candidate / "manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["trustedNativeInput"]["trustedEscapeObserved"] = False
        write_json(manifest_path, manifest)
        with self.assertRaisesRegex(
            baseline.PerformanceBaselineError,
            "trustedNativeInput",
        ):
            baseline.verify_drag_candidate(candidate, expected_source_sha=SOURCE_SHA)

    def test_drag_candidate_rejects_boolean_numeric_identity_confusion(self) -> None:
        mutations = {
            "manifest schema bool": lambda manifest: manifest.update({"schemaVersion": True}),
            "attestation schema bool": lambda manifest: manifest["trustedNativeInput"].update({
                "schemaVersion": True,
            }),
            "attestation flag integer": lambda manifest: manifest["trustedNativeInput"].update({
                "previewAppeared": 1,
            }),
            "attestation event nonstring": lambda manifest: manifest["trustedNativeInput"].update({
                "trustedPointerEvents": ["pointerdown", "pointermove", 1],
            }),
            "application digest integer": lambda manifest: manifest["captureEnvironment"].update({
                "applicationSha256": int("1" * 64),
            }),
            "profiling flag integer": lambda manifest: manifest["captureEnvironment"].update({
                "reactProfilingBuild": 1,
            }),
        }
        for index, (label, mutate) in enumerate(mutations.items()):
            with self.subTest(label=label):
                candidate = Path(self.temp.name) / f"typed-candidate-{index}"
                make_drag_candidate(candidate)
                manifest_path = candidate / "manifest.json"
                manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
                mutate(manifest)
                write_json(manifest_path, manifest)
                with self.assertRaises(baseline.PerformanceBaselineError):
                    baseline.verify_drag_candidate(
                        candidate,
                        expected_source_sha=SOURCE_SHA,
                    )

    def test_drag_candidate_rejects_symlinked_evidence_files_and_roots(self) -> None:
        candidate = Path(self.temp.name) / "symlinked-measurement"
        make_drag_candidate(candidate)
        measurement = candidate / baseline.canonical_measurement_path("deck-drag-v1")
        external = Path(self.temp.name) / "external-measurement.json"
        measurement.replace(external)
        measurement.symlink_to(external)
        with self.assertRaisesRegex(
            baseline.PerformanceBaselineError,
            "regular non-symlink|contains a symlink",
        ):
            baseline.verify_drag_candidate(candidate, expected_source_sha=SOURCE_SHA)

        candidate = Path(self.temp.name) / "symlinked-manifest"
        make_drag_candidate(candidate)
        manifest = candidate / "manifest.json"
        external_manifest = Path(self.temp.name) / "external-manifest.json"
        manifest.replace(external_manifest)
        manifest.symlink_to(external_manifest)
        with self.assertRaisesRegex(
            baseline.PerformanceBaselineError,
            "regular non-symlink",
        ):
            baseline.verify_drag_candidate(candidate, expected_source_sha=SOURCE_SHA)

        candidate = Path(self.temp.name) / "real-candidate-root"
        make_drag_candidate(candidate)
        alias = Path(self.temp.name) / "candidate-root-alias"
        alias.symlink_to(candidate, target_is_directory=True)
        with self.assertRaisesRegex(baseline.PerformanceBaselineError, "root must not"):
            baseline.verify_drag_candidate(alias, expected_source_sha=SOURCE_SHA)

    def test_drag_candidate_accepts_kernel_and_python_patch_drift_without_rewriting_provenance(self) -> None:
        candidate = Path(self.temp.name) / "compatible-candidate"
        make_drag_candidate(candidate)
        manifest_path = candidate / "manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["captureEnvironment"]["platform"] = (
            "Linux-6.8.99-999-generic-x86_64-with-glibc2.39"
        )
        manifest["captureEnvironment"]["python"] = "3.12.99"
        write_json(manifest_path, manifest)
        raw_before = manifest_path.read_bytes()

        self.assertEqual(
            [],
            baseline.compare_drag_candidate(
                candidate,
                self.root,
                expected_candidate_source_sha=SOURCE_SHA,
            ),
        )
        self.assertEqual(raw_before, manifest_path.read_bytes())
        compared = json.loads(manifest_path.read_text(encoding="utf-8"))["captureEnvironment"]
        self.assertEqual("Linux-6.8.99-999-generic-x86_64-with-glibc2.39", compared["platform"])
        self.assertEqual("3.12.99", compared["python"])

    def test_drag_candidate_rejects_every_normalized_and_exact_runtime_drift(self) -> None:
        mutations = {
            "os family": lambda environment: environment.update({
                "platform": "Darwin-24.6.0-x86_64-with-glibc2.39",
            }),
            "architecture": lambda environment: environment.update({
                "platform": "Linux-6.8.0-137-generic-aarch64-with-glibc2.39",
            }),
            "kernel ABI": lambda environment: environment.update({
                "platform": "Linux-6.9.0-137-generic-x86_64-with-glibc2.39",
            }),
            "libc family": lambda environment: environment.update({
                "platform": "Linux-6.8.0-137-generic-x86_64-with-musl2.39",
            }),
            "libc ABI": lambda environment: environment.update({
                "platform": "Linux-6.8.0-137-generic-x86_64-with-glibc2.40",
            }),
            "Python major/minor": lambda environment: environment.update({"python": "3.13.0"}),
            "device pixel ratio": lambda environment: environment.update({"devicePixelRatio": 2}),
            "Tauri driver": lambda environment: environment.update({"tauriDriver": "2.1"}),
            "WebKit driver": lambda environment: environment.update({"webkitWebDriver": "2.49"}),
            "Xvfb driver": lambda environment: environment.update({"xvfb": "22.0"}),
            "display": lambda environment: environment["display"].update({"dpi": 120}),
            "viewport": lambda environment: environment["viewport"].update({"width": 1280}),
            "fixed UI metadata": lambda environment: environment.update({"theme": "dark"}),
        }
        for index, (label, mutate) in enumerate(mutations.items()):
            with self.subTest(label=label):
                candidate = Path(self.temp.name) / f"incompatible-candidate-{index}"
                make_drag_candidate(candidate)
                manifest_path = candidate / "manifest.json"
                manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
                mutate(manifest["captureEnvironment"])
                write_json(manifest_path, manifest)
                with self.assertRaises(baseline.PerformanceBaselineError):
                    baseline.compare_drag_candidate(
                        candidate,
                        self.root,
                        expected_candidate_source_sha=SOURCE_SHA,
                    )

    def test_drag_candidate_rejects_malformed_platform_and_python_provenance(self) -> None:
        mutations = {
            "platform without kernel": lambda environment: environment.update({
                "platform": "Linux-x86_64-with-glibc2.39",
            }),
            "nonnumeric kernel": lambda environment: environment.update({
                "platform": "Linux-garbage-x86_64-with-glibc2.39",
            }),
            "trailing platform text": lambda environment: environment.update({
                "platform": "Linux-6.8.0-137-generic-x86_64-with-glibc2.39junk",
            }),
            "architecture alias": lambda environment: environment.update({
                "platform": "Linux-6.8.0-137-generic-amd64-with-glibc2.39",
            }),
            "leading-zero kernel": lambda environment: environment.update({
                "platform": "Linux-06.8.0-137-generic-x86_64-with-glibc2.39",
            }),
            "platform control character": lambda environment: environment.update({
                "platform": "Linux-6.8.0-137-generic-x86_64-with-glibc2.39\nignored",
            }),
            "short Python": lambda environment: environment.update({"python": "3.12"}),
            "Python suffix": lambda environment: environment.update({"python": "3.12.0rc1"}),
            "malformed Python suffix": lambda environment: environment.update({
                "python": "3.12.0........",
            }),
            "leading-zero Python": lambda environment: environment.update({"python": "03.12.0"}),
            "non-string Python": lambda environment: environment.update({"python": None}),
        }
        for index, (label, mutate) in enumerate(mutations.items()):
            with self.subTest(label=label):
                candidate = Path(self.temp.name) / f"malformed-candidate-{index}"
                make_drag_candidate(candidate)
                manifest_path = candidate / "manifest.json"
                manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
                mutate(manifest["captureEnvironment"])
                write_json(manifest_path, manifest)
                with self.assertRaises(baseline.PerformanceBaselineError):
                    baseline.compare_drag_candidate(
                        candidate,
                        self.root,
                        expected_candidate_source_sha=SOURCE_SHA,
                    )

    def test_comparison_rejects_identically_malformed_runtime_metadata(self) -> None:
        candidate = Path(self.temp.name) / "identically-malformed-candidate"
        make_drag_candidate(candidate)
        malformed = "Linux-x86_64-with-glibc2.39"
        for root in (self.root, candidate):
            manifest_path = root / "manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            manifest["captureEnvironment"]["platform"] = malformed
            write_json(manifest_path, manifest)
        with self.assertRaises(baseline.PerformanceBaselineError):
            baseline.compare_drag_candidate(
                candidate,
                self.root,
                expected_candidate_source_sha=SOURCE_SHA,
            )

    def test_candidate_driver_is_drag_only_without_navigation_or_other_workflows(self) -> None:
        source = inspect.getsource(baseline.collect_candidate_drag_scenario)
        self.assertNotIn("navigate(", source)
        self.assertNotIn("document-list-interaction", source)
        self.assertNotIn("deck-selection", source)
        self.assertNotIn("study-guide", source)
        self.assertNotIn("active-practice", source)
        self.assertIn("LARGE_LIBRARY_DRAG_XPATH", source)

    def test_trusted_native_drag_smoke_crosses_webkit_and_always_cleans_up(self) -> None:
        pointer = mock.Mock()
        pointer.close = mock.Mock()
        observed = [
            {"sequence": 11, "pointerId": 7},
            {"sequence": 12, "pointerId": 7},
            {"sequence": 15, "pointerId": 7},
        ]
        with mock.patch.object(
            baseline.webdriver,
            "find_element",
            return_value="large-drag-row",
        ) as find_element, mock.patch.object(
            baseline,
            "find_safe_drag_point",
            return_value={"x": 20, "y": 30},
        ), mock.patch.object(
            baseline,
            "native_library_structure",
            side_effect=[
                [{"kind": "deck", "geometryKey": "deck:source", "itemId": "source",
                  "itemIndex": "0", "name": "Large library drag source",
                  "folderId": "__unsorted__", "folderIndex": "", "collapsed": ""}],
                [{"kind": "deck", "geometryKey": "deck:source", "itemId": "source",
                  "itemIndex": "0", "name": "Large library drag source",
                  "folderId": "__unsorted__", "folderIndex": "", "collapsed": ""}],
            ],
        ), mock.patch.object(
            baseline,
            "install_native_pointer_probe",
            return_value={
                "screenX": 0,
                "screenY": 0,
                "outerWidth": 1400,
                "outerHeight": 900,
                "innerWidth": 1400,
                "innerHeight": 900,
            },
        ), mock.patch.object(
            baseline,
            "NativeX11Pointer",
            return_value=pointer,
        ), mock.patch.object(
            baseline,
            "calibrate_native_pointer",
            return_value={"x": 120, "y": 230},
        ), mock.patch.object(
            baseline,
            "native_pointer_sequence",
            return_value=10,
        ), mock.patch.object(
            baseline,
            "wait_for_native_pointer_event",
            side_effect=observed,
        ) as wait_event, mock.patch.object(
            baseline,
            "wait_for_native_key_event",
            side_effect=[{"sequence": 13}, {"sequence": 14}],
        ) as wait_key, mock.patch.object(
            baseline,
            "wait_js",
        ) as wait_js, mock.patch.object(
            baseline,
            "remove_native_pointer_probe",
        ) as remove_probe:
            attestation = baseline.verify_trusted_native_drag_smoke("session-1")

        self.assertEqual(
            [
                mock.call.set_primary_button(True),
                mock.call.move(129, 230),
                mock.call.set_escape_key(True),
                mock.call.set_escape_key(False),
                mock.call.set_primary_button(False),
                mock.call.close(),
            ],
            pointer.mock_calls,
        )
        self.assertEqual(3, wait_event.call_count)
        self.assertEqual(2, wait_key.call_count)
        self.assertEqual(True, wait_event.call_args_list[0].kwargs["target_can_start_drag"])
        self.assertEqual(1, wait_event.call_args_list[0].kwargs["buttons"])
        self.assertEqual(0, wait_event.call_args_list[2].kwargs["buttons"])
        self.assertEqual(
            [
                mock.call(
                    "session-1",
                    "document.querySelector('[data-drag-preview]')",
                    mock.ANY,
                ),
                mock.call(
                    "session-1",
                    "!document.querySelector('[data-drag-preview]')",
                    mock.ANY,
                ),
                mock.call(
                    "session-1",
                    "!document.querySelector('[data-drag-preview]')",
                    mock.ANY,
                ),
            ],
            wait_js.call_args_list,
        )
        self.assertEqual(2, find_element.call_count)
        remove_probe.assert_called_once_with("session-1")
        self.assertEqual(baseline.TRUSTED_NATIVE_INPUT_ATTESTATION, attestation)

    def test_trusted_native_drag_smoke_releases_button_and_probe_after_failure(self) -> None:
        pointer = mock.Mock()
        with mock.patch.object(
            baseline.webdriver,
            "find_element",
            return_value="large-drag-row",
        ), mock.patch.object(
            baseline,
            "find_safe_drag_point",
            return_value={"x": 20, "y": 30},
        ), mock.patch.object(
            baseline,
            "native_library_structure",
            return_value=[{
                "kind": "deck", "geometryKey": "deck:source", "itemId": "source",
                "itemIndex": "0", "name": "Large library drag source",
                "folderId": "__unsorted__", "folderIndex": "", "collapsed": "",
            }],
        ), mock.patch.object(
            baseline,
            "install_native_pointer_probe",
            return_value={
                "screenX": 0,
                "screenY": 0,
                "outerWidth": 1400,
                "outerHeight": 900,
                "innerWidth": 1400,
                "innerHeight": 900,
            },
        ), mock.patch.object(
            baseline,
            "NativeX11Pointer",
            return_value=pointer,
        ), mock.patch.object(
            baseline,
            "calibrate_native_pointer",
            side_effect=AssertionError("no trusted input"),
        ), mock.patch.object(
            baseline,
            "remove_native_pointer_probe",
        ) as remove_probe:
            with self.assertRaisesRegex(AssertionError, "no trusted input"):
                baseline.verify_trusted_native_drag_smoke("session-1")

        pointer.close.assert_called_once_with()
        remove_probe.assert_called_once_with("session-1")

    def test_native_pointer_probe_and_adapter_are_fail_closed(self) -> None:
        probe_source = inspect.getsource(baseline.install_native_pointer_probe)
        wait_source = inspect.getsource(baseline.wait_for_native_pointer_event)
        adapter_source = MODULE_PATH.read_text(encoding="utf-8")
        self.assertIn("event.isTrusted", probe_source)
        self.assertIn('event.get("isTrusted") is True', wait_source)
        self.assertIn('event.get("pointerType") == "mouse"', wait_source)
        self.assertIn('event.get("targetWithinRow")', wait_source)
        self.assertIn('event.get("targetInteractive")', wait_source)
        self.assertIn('event.get("targetCanStartDrag")', wait_source)
        self.assertIn("targetElement?.closest(interactiveSelector)", probe_source)
        self.assertIn("targetApprovedDragSurface", probe_source)
        self.assertIn("XTestQueryExtension", adapter_source)
        self.assertIn("if self._button_down", adapter_source)
        self.assertIn("XTestFakeButtonEvent(self._display, 1, 0, 0)", adapter_source)

    def test_native_pointer_calibration_accepts_an_exact_first_motion(self) -> None:
        pointer = mock.Mock()
        geometry = {
            "screenX": 0, "screenY": 0, "outerWidth": 1400, "outerHeight": 900,
            "innerWidth": 1400, "innerHeight": 900,
        }
        exact = {
            "sequence": 5,
            "clientX": 20,
            "clientY": 30,
            "screenX": 20,
            "screenY": 30,
            "targetIsRow": True,
            "targetWithinRow": True,
            "targetInteractive": True,
            "targetApprovedDragSurface": True,
            "targetCanStartDrag": True,
        }
        with mock.patch.object(
            baseline,
            "native_pointer_sequence",
            return_value=4,
        ), mock.patch.object(
            baseline,
            "wait_for_native_pointer_event",
            return_value=exact,
        ) as wait_event:
            calibrated = baseline.calibrate_native_pointer(
                "session-1",
                pointer,
                {"x": 20, "y": 30},
                geometry,
            )

        self.assertEqual({"x": 20, "y": 30}, calibrated)
        pointer.move.assert_called_once_with(20, 30)
        wait_event.assert_called_once()

    def test_native_pointer_calibration_corrects_a_measured_viewport_offset(self) -> None:
        pointer = mock.Mock()
        geometry = {
            "screenX": 0, "screenY": 0, "outerWidth": 1400, "outerHeight": 900,
            "innerWidth": 1400, "innerHeight": 900,
        }
        with mock.patch.object(
            baseline,
            "native_pointer_sequence",
            return_value=4,
        ), mock.patch.object(
            baseline,
            "wait_for_native_pointer_event",
            side_effect=[
                {
                    "sequence": 5,
                    "clientX": 10,
                    "clientY": 25,
                    "screenX": 20,
                    "screenY": 30,
                    "targetIsRow": False,
                    "targetWithinRow": False,
                    "targetInteractive": True,
                    "targetApprovedDragSurface": False,
                    "targetCanStartDrag": False,
                },
                {"sequence": 6},
            ],
        ) as wait_event:
            calibrated = baseline.calibrate_native_pointer(
                "session-1",
                pointer,
                {"x": 20, "y": 30},
                geometry,
            )

        self.assertEqual({"x": 30, "y": 35}, calibrated)
        self.assertEqual([mock.call(20, 30), mock.call(30, 35)], pointer.move.call_args_list)
        self.assertEqual(2, wait_event.call_count)
        self.assertEqual(30, wait_event.call_args_list[1].kwargs["screen_x"])
        self.assertEqual(35, wait_event.call_args_list[1].kwargs["screen_y"])

    def test_native_pointer_cleanup_attempts_button_release_after_escape_failure(self) -> None:
        pointer = object.__new__(baseline.NativeX11Pointer)
        pointer._display = 1
        pointer._escape_down = True
        pointer._button_down = True
        pointer._root = 1
        pointer._x11 = mock.Mock()
        pointer._x11.XKeysymToKeycode.return_value = 9
        pointer._xtst = mock.Mock()
        pointer._xtst.XTestFakeKeyEvent.return_value = 0
        pointer._xtst.XTestFakeButtonEvent.return_value = 1
        with mock.patch.object(
            pointer,
            "_query_pointer",
            return_value={"x": 0, "y": 0, "mask": 0},
        ):
            with self.assertRaisesRegex(AssertionError, "cleanup Escape key-up"):
                pointer.close()

        pointer._xtst.XTestFakeButtonEvent.assert_called_once_with(1, 1, 0, 0)
        pointer._x11.XCloseDisplay.assert_called_once_with(1)
        self.assertIsNone(pointer._display)

    def test_native_pointer_constructor_rejects_multiple_screens_without_partial_cleanup(self) -> None:
        x11 = mock.Mock()
        xtst = mock.Mock()
        x11.XOpenDisplay.return_value = 1
        x11.XScreenCount.return_value = 2
        xtst.XTestQueryExtension.return_value = 1
        environment = {
            "OSTUDY_PERFORMANCE_IN_XVFB": "1",
            "GDK_BACKEND": "x11",
            "DISPLAY": ":99",
        }
        with mock.patch.dict(baseline.os.environ, environment, clear=True), mock.patch.object(
            baseline.ctypes,
            "CDLL",
            side_effect=[x11, xtst],
        ):
            with self.assertRaisesRegex(AssertionError, "one X11 screen"):
                baseline.NativeX11Pointer()

        x11.XCloseDisplay.assert_called_once_with(1)
        x11.XQueryPointer.assert_not_called()

    def test_rendering_frame_clock_requires_visible_unthrottled_frames(self) -> None:
        active = {
            "visibilityState": "visible",
            "hidden": False,
            "hasFocus": True,
            "intervals": [16.0, 17.0, 16.5, 17.5, 16.0, 18.0, 16.0],
        }
        with mock.patch.object(baseline.webdriver, "request", return_value=active):
            evidence = baseline.verify_rendering_frame_clock("session-1")
        self.assertEqual("visible", evidence["visibilityState"])
        self.assertTrue(evidence["hasFocus"])
        self.assertLess(evidence["medianIntervalMs"], 20)

        hidden = {**active, "visibilityState": "hidden", "hidden": True}
        with mock.patch.object(baseline.webdriver, "request", return_value=hidden):
            with self.assertRaisesRegex(AssertionError, "visibly measurable"):
                baseline.verify_rendering_frame_clock("session-1")

        throttled = {**active, "hasFocus": False, "intervals": [200.0] * 7}
        with mock.patch.object(baseline.webdriver, "request", return_value=throttled):
            with self.assertRaisesRegex(AssertionError, "frame clock is throttled"):
                baseline.verify_rendering_frame_clock("session-1")


    def test_candidate_launcher_pins_x11_backend_for_trusted_input(self) -> None:
        launcher = (
            baseline.ROOT_DIR / "scripts/ui-performance-baselines.sh"
        ).read_text(encoding="utf-8")
        candidate_section = launcher.split(
            'if [[ "$CAPTURE_COMMAND" == "capture-candidate" ]]',
        )[-1].split("else", 1)[0]
        self.assertIn("GDK_BACKEND=x11", candidate_section)
        self.assertIn("WEBKIT_DISABLE_COMPOSITING_MODE=1", candidate_section)
        self.assertIn("-fakescreenfps 60", launcher)

    def test_candidate_driver_uses_only_the_large_library_native_selector(self) -> None:
        with mock.patch.object(
            baseline,
            "execute_read_only",
            return_value="Flashcards",
        ), mock.patch.object(
            baseline.webdriver,
            "find_element",
            return_value="large-drag-row",
        ) as find_element, mock.patch.object(
            baseline,
            "collect_candidate_drag_sample",
            return_value={"sample": True},
        ) as collect_candidate_drag_sample:
            result = baseline.collect_candidate_drag_scenario("session-1")

        find_element.assert_called_once_with(
            "session-1",
            "xpath",
            baseline.LARGE_LIBRARY_DRAG_XPATH,
            mock.ANY,
        )
        collect_candidate_drag_sample.assert_called_once_with(
            "session-1",
            "large-drag-row",
            "deck-drag-v1",
            mock.ANY,
        )
        self.assertEqual({"sample": True}, result)

    def test_candidate_drag_finalizes_activation_in_one_browser_command_before_cleanup(self) -> None:
        events: list[str] = []
        with mock.patch.object(
            baseline,
            "find_safe_drag_point",
            side_effect=lambda *_args: events.append("safe-point") or {"x": 10, "y": 20},
        ), mock.patch.object(
            baseline,
            "dispatch_candidate_drag_pointer_event",
            side_effect=lambda _session, _target, _anchor, event_type, x, y: events.append(
                f"{event_type}:{x},{y}"
            ),
        ), mock.patch.object(
            baseline,
            "dispatch_candidate_drag_activation_and_finish",
            side_effect=lambda _session, _target, _anchor, x, y: events.append(
                f"pointermove+finish:{x},{y}"
            ) or {"sample": True},
        ), mock.patch.object(
            baseline,
            "arm",
            side_effect=lambda *_args: events.append("arm"),
        ), mock.patch.object(
            baseline,
            "wait_js",
            side_effect=lambda _session, expression, _deadline: events.append(
                "preview-active"
                if expression.startswith("document.querySelector")
                else "preview-clean"
            ),
        ), mock.patch.object(
            baseline,
            "release_pointer_actions",
            side_effect=lambda *_args: events.append("release"),
        ):
            result = baseline.collect_candidate_drag_sample(
                "session-1", "large-drag-row", "deck-drag-v1", 123.0,
            )

        self.assertEqual(
            [
                "safe-point", "pointerdown:10,20", "arm",
                "pointermove+finish:19,20", "preview-active",
                "pointercancel:19,20", "preview-clean", "release",
            ],
            events,
        )
        self.assertEqual({"sample": True}, result)
        self.assertGreater(
            math.hypot(
                baseline.CANDIDATE_DRAG_ACTIVATION_OFFSET["x"],
                baseline.CANDIDATE_DRAG_ACTIVATION_OFFSET["y"],
            ),
            8,
        )

    def test_candidate_drag_rejects_a_preview_missing_after_async_finalization(self) -> None:
        dispatches: list[str] = []
        with mock.patch.object(
            baseline, "find_safe_drag_point", return_value={"x": 10, "y": 20},
        ), mock.patch.object(
            baseline,
            "dispatch_candidate_drag_pointer_event",
            side_effect=lambda _session, _target, _anchor, event_type, _x, _y: dispatches.append(event_type),
        ), mock.patch.object(baseline, "arm"), mock.patch.object(
            baseline,
            "dispatch_candidate_drag_activation_and_finish",
            return_value={"sample": True},
        ) as activate, mock.patch.object(
            baseline.webdriver, "deadline_after", return_value=456.0,
        ), mock.patch.object(
            baseline, "wait_js", side_effect=[AssertionError("no active drag preview"), None],
        ) as wait_js, mock.patch.object(
            baseline, "release_pointer_actions",
        ) as release:
            with self.assertRaisesRegex(AssertionError, "no active drag preview"):
                baseline.collect_candidate_drag_sample(
                    "session-1", "large-drag-row", "deck-drag-v1", 123.0,
                )

        self.assertEqual(["pointerdown", "pointercancel"], dispatches)
        activate.assert_called_once_with(
            "session-1", "large-drag-row", {"x": 10, "y": 20}, 19, 20,
        )
        self.assertEqual(
            mock.call(
                "session-1",
                "!document.querySelector('[data-drag-preview]')",
                456.0,
            ),
            wait_js.call_args_list[-1],
        )
        release.assert_called_once_with("session-1")


    def test_drag_capture_rejects_a_row_without_an_approved_primary_surface(self) -> None:
        with mock.patch.object(
            baseline,
            "execute_read_only",
            return_value=None,
        ) as execute_read_only:
            with self.assertRaisesRegex(AssertionError, "no verified primary drag surface point"):
                baseline.find_safe_drag_point("session-1", "covered-row")

        script = execute_read_only.call_args.args[1]
        self.assertIn("[data-library-drag-surface='true']", script)
        self.assertIn("target.closest", script)
        self.assertIn("canStartAt(x + activationOffsetX, y)", script)
        self.assertEqual(
            baseline.CANDIDATE_DRAG_ACTIVATION_OFFSET["x"],
            execute_read_only.call_args.args[2][1],
        )

    def test_candidate_dom_pointer_dispatch_uses_exact_contract_payload(self) -> None:
        def observe(_session: str, _script: str, args: list[dict]) -> dict:
            expected = args[1]
            return {
                key: value
                for key, value in expected.items()
                if key not in {"anchorX", "anchorY"}
            } | {"targetObserved": True, "windowObserved": True}

        with mock.patch.object(baseline, "execute", side_effect=observe) as execute:
            for event_type, x, y in (
                ("pointerdown", 10, 20),
                ("pointermove", 19, 20),
                ("pointercancel", 19, 20),
            ):
                baseline.dispatch_candidate_drag_pointer_event(
                    "session-1",
                    "large-drag-row",
                    {"x": 10, "y": 20},
                    event_type,
                    x,
                    y,
                )

        expected_contracts = [
            {
                "type": "pointerdown", "pointerId": 4004, "pointerType": "mouse",
                "isPrimary": True, "clientX": 10, "clientY": 20,
                "button": 0, "buttons": 1,
            },
            {
                "type": "pointermove", "pointerId": 4004, "pointerType": "mouse",
                "isPrimary": True, "clientX": 19, "clientY": 20,
                "button": -1, "buttons": 1,
            },
            {
                "type": "pointercancel", "pointerId": 4004, "pointerType": "mouse",
                "isPrimary": True, "clientX": 19, "clientY": 20,
                "button": -1, "buttons": 0,
            },
        ]
        self.assertEqual(3, execute.call_count)
        for call, expected_contract in zip(execute.call_args_list, expected_contracts, strict=True):
            script = call.args[1]
            payload = call.args[2]
            self.assertEqual({baseline.webdriver.ELEMENT_KEY: "large-drag-row"}, payload[0])
            self.assertEqual(
                expected_contract | {
                    "bubbles": True, "cancelable": True, "composed": True,
                    "anchorX": 10, "anchorY": 20,
                },
                payload[1],
            )
            self.assertIn("typeof PointerEvent !== 'function'", script)
            self.assertIn("Candidate deck drag target is stale or wrong", script)
            self.assertIn("document.elementFromPoint(expected.anchorX, expected.anchorY) !== row", script)
            self.assertIn("row.dispatchEvent(event)", script)
            self.assertIn("!targetObserved || !windowObserved", script)

    def test_candidate_async_activation_owns_dispatch_preview_and_finish(self) -> None:
        expected = baseline.candidate_pointer_event_contract(
            {"x": 10, "y": 20}, "pointermove", 19, 20,
        )
        observed = {
            key: value for key, value in expected.items()
            if key not in {"anchorX", "anchorY"}
        } | {"targetObserved": True, "windowObserved": True}
        raw_sample = {
            "interactionToVisibleMs": 12,
            "interactionToReadyMs": 13,
            "userTiming": {"startTimeMs": 1, "visibleTimeMs": 13, "readyTimeMs": 14},
            "react": {
                "totalRenderDurationMs": 0, "avoidableCommitCount": 0,
                "commits": [], "commitCount": 0,
            },
            "pointer": {
                "state": "available", "eventCount": 1, "frameCount": 1,
                "maxFrameWorkMs": 0, "maxFrameIntervalMs": 16,
            },
        }
        with mock.patch.object(
            baseline.webdriver, "request",
            return_value={"ok": True, "observed": observed, "sample": raw_sample},
        ) as request:
            sample = baseline.dispatch_candidate_drag_activation_and_finish(
                "session-1", "large-drag-row", {"x": 10, "y": 20}, 19, 20,
            )

        self.assertEqual(12, sample["interactionToVisible"]["valueMs"])
        self.assertEqual("POST", request.call_args.args[0])
        self.assertEqual("/session/session-1/execute/async", request.call_args.args[1])
        payload = request.call_args.args[2]
        self.assertEqual({baseline.webdriver.ELEMENT_KEY: "large-drag-row"}, payload["args"][0])
        self.assertEqual(expected, payload["args"][1])
        self.assertIn("row.dispatchEvent(event)", payload["script"])
        self.assertIn("requestAnimationFrame(awaitPreview)", payload["script"])
        self.assertIn("bridge.finish()", payload["script"])

        with mock.patch.object(
            baseline.webdriver, "request",
            return_value={
                "ok": False,
                "error": {
                    "name": "DragPreviewTimeoutError",
                    "message": "The activated drag preview did not become visible.",
                },
            },
        ):
            with self.assertRaisesRegex(
                baseline.BrowserPerformanceError, "DragPreviewTimeoutError",
            ):
                baseline.dispatch_candidate_drag_activation_and_finish(
                    "session-1", "large-drag-row", {"x": 10, "y": 20}, 19, 20,
                )


    def test_candidate_dom_pointer_dispatch_rejects_stale_and_mismatched_contracts(self) -> None:
        stale = baseline.webdriver.WebDriverProtocolError(
            404,
            "stale element reference",
            "row was replaced",
        )
        with mock.patch.object(baseline, "execute", side_effect=stale):
            with self.assertRaisesRegex(AssertionError, "target is stale or wrong"):
                baseline.dispatch_candidate_drag_pointer_event(
                    "session-1", "stale-row", {"x": 10, "y": 20},
                    "pointerdown", 10, 20,
                )

        with mock.patch.object(baseline, "execute", return_value={"type": "pointerdown"}):
            with self.assertRaisesRegex(AssertionError, "observed contract differs"):
                baseline.dispatch_candidate_drag_pointer_event(
                    "session-1", "large-drag-row", {"x": 10, "y": 20},
                    "pointerdown", 10, 20,
                )

        unavailable = baseline.webdriver.WebDriverProtocolError(
            500,
            "javascript error",
            "Candidate PointerEvent is unavailable.",
        )
        with mock.patch.object(baseline, "execute", side_effect=unavailable):
            with self.assertRaisesRegex(
                baseline.webdriver.WebDriverProtocolError,
                "Candidate PointerEvent is unavailable",
            ):
                baseline.dispatch_candidate_drag_pointer_event(
                    "session-1", "large-drag-row", {"x": 10, "y": 20},
                    "pointerdown", 10, 20,
                )

    def test_candidate_drag_releases_actions_without_cancelling_when_pointerdown_fails(self) -> None:
        with mock.patch.object(
            baseline,
            "find_safe_drag_point",
            return_value={"x": 10, "y": 20},
        ), mock.patch.object(
            baseline,
            "dispatch_candidate_drag_pointer_event",
            side_effect=ConnectionResetError("ambiguous press delivery"),
        ) as dispatch, mock.patch.object(baseline, "arm") as arm, mock.patch.object(
            baseline,
            "release_pointer_actions",
        ) as release:
            with self.assertRaisesRegex(ConnectionResetError, "ambiguous press delivery"):
                baseline.collect_candidate_drag_sample(
                    "session-1",
                    "large-drag-row",
                    "deck-drag-v1",
                    123.0,
                )

        arm.assert_not_called()
        self.assertEqual("pointerdown", dispatch.call_args.args[3])
        dispatch.assert_called_once()
        release.assert_called_once_with("session-1")

    def test_candidate_drag_cleanup_does_not_retry_an_ambiguous_cancel_or_mask_failure(self) -> None:
        dispatches: list[str] = []

        def dispatch(_session: str, _target: str, _point: dict, event_type: str, _x: int, _y: int) -> None:
            dispatches.append(event_type)
            if event_type == "pointercancel":
                raise ConnectionResetError("ambiguous cancel delivery")

        with mock.patch.object(
            baseline,
            "find_safe_drag_point",
            return_value={"x": 10, "y": 20},
        ), mock.patch.object(
            baseline,
            "dispatch_candidate_drag_pointer_event",
            side_effect=dispatch,
        ), mock.patch.object(baseline, "arm"), mock.patch.object(
            baseline,
            "dispatch_candidate_drag_activation_and_finish",
            return_value={"sample": True},
        ) as activate, mock.patch.object(
            baseline,
            "wait_js",
            side_effect=[None, None],
        ), mock.patch.object(
            baseline,
            "release_pointer_actions",
            side_effect=RuntimeError("release cleanup also failed"),
        ) as release:
            with self.assertRaisesRegex(ConnectionResetError, "ambiguous cancel delivery"):
                baseline.collect_candidate_drag_sample(
                    "session-1",
                    "large-drag-row",
                    "deck-drag-v1",
                    123.0,
                )

        self.assertEqual(["pointerdown", "pointercancel"], dispatches)
        activate.assert_called_once_with(
            "session-1", "large-drag-row", {"x": 10, "y": 20}, 19, 20,
        )
        release.assert_called_once_with("session-1")

    def test_candidate_post_pointerdown_cleanup_preserves_the_primary_failure(self) -> None:
        dispatches: list[str] = []

        def dispatch(_session: str, _target: str, _point: dict, event_type: str, _x: int, _y: int) -> None:
            dispatches.append(event_type)
            if event_type == "pointercancel":
                raise RuntimeError("cleanup cancel failed")

        with mock.patch.object(
            baseline,
            "find_safe_drag_point",
            return_value={"x": 10, "y": 20},
        ), mock.patch.object(
            baseline,
            "dispatch_candidate_drag_pointer_event",
            side_effect=dispatch,
        ), mock.patch.object(
            baseline,
            "arm",
            side_effect=AssertionError("primary arm failure"),
        ), mock.patch.object(
            baseline,
            "wait_js",
            side_effect=RuntimeError("cleanup preview check failed"),
        ), mock.patch.object(
            baseline,
            "dispatch_candidate_drag_activation_and_finish",
        ) as activate, mock.patch.object(
            baseline,
            "release_pointer_actions",
            side_effect=RuntimeError("cleanup release failed"),
        ) as release:
            with self.assertRaisesRegex(AssertionError, "primary arm failure"):
                baseline.collect_candidate_drag_sample(
                    "session-1",
                    "large-drag-row",
                    "deck-drag-v1",
                    123.0,
                )

        self.assertEqual(["pointerdown", "pointercancel"], dispatches)
        activate.assert_not_called()
        release.assert_called_once_with("session-1")

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

    def test_arm_recovers_one_disconnect_and_still_requires_the_bridge_version(self) -> None:
        with mock.patch.object(
            baseline,
            "execute",
            side_effect=[
                ConnectionResetError("lost arm response"),
                baseline.INSTRUMENTATION_VERSION,
            ],
        ) as execute:
            baseline.arm("session-1", "navigation-hover-v1", "element-1")

        self.assertEqual(2, execute.call_count)
        self.assertEqual(execute.call_args_list[0], execute.call_args_list[1])

        with mock.patch.object(
            baseline,
            "execute",
            side_effect=[ConnectionResetError("lost arm response"), "stale-version"],
        ) as execute:
            with self.assertRaisesRegex(AssertionError, "Performance bridge unavailable or stale"):
                baseline.arm("session-1", "navigation-hover-v1", "element-1")
        self.assertEqual(2, execute.call_count)

    def test_arm_retries_only_transient_disconnects_once(self) -> None:
        with mock.patch.object(
            baseline,
            "execute",
            side_effect=[ConnectionResetError("reset"), ConnectionResetError("reset again")],
        ) as execute:
            with self.assertRaises(ConnectionResetError):
                baseline.arm("session-1", "navigation-hover-v1", "element-1")
        self.assertEqual(2, execute.call_count)

        failure = baseline.webdriver.WebDriverProtocolError(500, "unknown error", "browser assertion")
        with mock.patch.object(baseline, "execute", side_effect=failure) as execute:
            with self.assertRaises(baseline.webdriver.WebDriverProtocolError):
                baseline.arm("session-1", "navigation-hover-v1", "element-1")
        execute.assert_called_once()

    def test_study_guide_reset_accepts_an_applied_close_after_a_lost_response(self) -> None:
        with mock.patch.object(
            baseline.webdriver,
            "find_element",
            return_value="close-1",
        ) as find_element, mock.patch.object(
            baseline.webdriver,
            "click",
            side_effect=ConnectionResetError("lost close response"),
        ) as click, mock.patch.object(
            baseline,
            "execute_read_only",
            return_value=True,
        ) as execute_read_only, mock.patch.object(baseline, "wait_js") as wait_js:
            baseline.reset_study_guide_dialog("session-1", 123.0)

        find_element.assert_called_once()
        click.assert_called_once_with("session-1", "close-1")
        execute_read_only.assert_called_once_with(
            "session-1",
            "return Boolean(!document.querySelector('[role=dialog]'));",
        )
        wait_js.assert_called_once_with(
            "session-1",
            "!document.querySelector('[role=dialog]')",
            123.0,
        )

    def test_study_guide_reset_re_resolves_and_retries_one_unapplied_close(self) -> None:
        with mock.patch.object(
            baseline.webdriver,
            "find_element",
            side_effect=["close-1", "close-2"],
        ) as find_element, mock.patch.object(
            baseline.webdriver,
            "click",
            side_effect=[ConnectionResetError("lost close response"), None],
        ) as click, mock.patch.object(
            baseline,
            "execute_read_only",
            return_value=False,
        ), mock.patch.object(baseline, "wait_js") as wait_js:
            baseline.reset_study_guide_dialog("session-1", 123.0)

        self.assertEqual(2, find_element.call_count)
        self.assertEqual(
            [
                mock.call("session-1", "close-1"),
                mock.call("session-1", "close-2"),
            ],
            click.call_args_list,
        )
        wait_js.assert_called_once()

    def test_study_guide_reset_propagates_unresolved_and_repeated_disconnects(self) -> None:
        with mock.patch.object(
            baseline.webdriver,
            "find_element",
            return_value="close-1",
        ), mock.patch.object(
            baseline.webdriver,
            "click",
            side_effect=ConnectionResetError("lost close response"),
        ) as click, mock.patch.object(
            baseline,
            "execute_read_only",
            return_value=None,
        ), mock.patch.object(baseline, "wait_js") as wait_js:
            with self.assertRaisesRegex(
                AssertionError,
                "Invalid study-guide dialog reset postcondition",
            ):
                baseline.reset_study_guide_dialog("session-1", 123.0)
        click.assert_called_once()
        wait_js.assert_not_called()

        with mock.patch.object(
            baseline.webdriver,
            "find_element",
            side_effect=["close-1", "close-2"],
        ), mock.patch.object(
            baseline.webdriver,
            "click",
            side_effect=[
                ConnectionResetError("lost close response"),
                ConnectionResetError("lost retry response"),
            ],
        ) as click, mock.patch.object(
            baseline,
            "execute_read_only",
            return_value=False,
        ), mock.patch.object(baseline, "wait_js") as wait_js:
            with self.assertRaisesRegex(ConnectionResetError, "lost retry response"):
                baseline.reset_study_guide_dialog("session-1", 123.0)
        self.assertEqual(2, click.call_count)
        wait_js.assert_not_called()

    def test_study_guide_reset_propagates_non_transient_close_failures(self) -> None:
        failure = baseline.webdriver.WebDriverProtocolError(
            500,
            "unknown error",
            "browser assertion",
        )
        with mock.patch.object(
            baseline.webdriver,
            "find_element",
            return_value="close-1",
        ), mock.patch.object(
            baseline.webdriver,
            "click",
            side_effect=failure,
        ) as click, mock.patch.object(
            baseline,
            "execute_read_only",
        ) as execute_read_only, mock.patch.object(baseline, "wait_js") as wait_js:
            with self.assertRaises(baseline.webdriver.WebDriverProtocolError):
                baseline.reset_study_guide_dialog("session-1", 123.0)

        click.assert_called_once()
        execute_read_only.assert_not_called()
        wait_js.assert_not_called()

    def test_study_guide_reset_requires_the_final_absent_postcondition(self) -> None:
        with mock.patch.object(
            baseline.webdriver,
            "find_element",
            return_value="close-1",
        ), mock.patch.object(baseline.webdriver, "click"), mock.patch.object(
            baseline,
            "wait_js",
            side_effect=AssertionError("dialog remained open"),
        ) as wait_js:
            with self.assertRaisesRegex(AssertionError, "dialog remained open"):
                baseline.reset_study_guide_dialog("session-1", 123.0)

        wait_js.assert_called_once_with(
            "session-1",
            "!document.querySelector('[role=dialog]')",
            123.0,
        )

    def test_study_guide_reset_runs_only_after_the_sample_finishes(self) -> None:
        events: list[str] = []
        with mock.patch.object(baseline, "navigate"), mock.patch.object(
            baseline.webdriver,
            "find_element",
            return_value="open-guide",
        ), mock.patch.object(baseline, "arm"), mock.patch.object(
            baseline.webdriver,
            "click",
            side_effect=lambda *_args: events.append("measured-click"),
        ), mock.patch.object(
            baseline,
            "wait_js",
            side_effect=lambda *_args: events.append("ready"),
        ), mock.patch.object(
            baseline,
            "finish",
            side_effect=lambda _session: events.append("finish") or {"sample": True},
        ), mock.patch.object(
            baseline,
            "reset_study_guide_dialog",
            side_effect=lambda *_args: events.append("reset"),
        ):
            result = baseline.collect_scenario(
                "session-1",
                "study-guide-open-v1",
                0,
            )

        self.assertEqual(["measured-click", "ready", "finish", "reset"], events)
        self.assertEqual({"sample": True}, result)

    def test_deck_selection_canonicalizes_before_arm_and_restores_after_finish(self) -> None:
        events: list[str] = []
        with mock.patch.object(baseline, "navigate"), mock.patch.object(
            baseline,
            "canonicalize_practice_setup",
            side_effect=lambda *_args: events.append("canonicalize"),
        ), mock.patch.object(
            baseline.webdriver,
            "find_element",
            side_effect=["measured-checkbox", "reset-checkbox"],
        ), mock.patch.object(
            baseline,
            "checkbox_checked",
            side_effect=lambda *_args: events.append("read-original") or True,
        ), mock.patch.object(
            baseline,
            "begin_practice_workload_observation",
            side_effect=lambda *_args: events.append("observe"),
        ), mock.patch.object(
            baseline,
            "arm",
            side_effect=lambda *_args: events.append("arm"),
        ), mock.patch.object(
            baseline.webdriver,
            "click",
            side_effect=lambda _session, element: events.append(f"click:{element}"),
        ), mock.patch.object(
            baseline,
            "wait_for_practice_selection",
            side_effect=lambda *_args: events.append("changed-ready"),
        ) as wait_selection, mock.patch.object(
            baseline,
            "finish",
            side_effect=lambda _session: events.append("finish") or {"sample": True},
        ), mock.patch.object(
            baseline,
            "end_practice_workload_observation",
            side_effect=lambda *_args: events.append("stop-observing"),
        ):
            result = baseline.collect_scenario("session-1", "deck-selection-v1", 0)

        self.assertEqual(
            [
                "canonicalize",
                "read-original",
                "observe",
                "arm",
                "click:measured-checkbox",
                "changed-ready",
                "finish",
                "stop-observing",
                "observe",
                "click:reset-checkbox",
                "changed-ready",
                "stop-observing",
            ],
            events,
        )
        self.assertEqual(
            [mock.call("session-1", False, mock.ANY), mock.call("session-1", True, mock.ANY)],
            wait_selection.call_args_list,
        )
        self.assertEqual({"sample": True}, result)

    def test_practice_readiness_requires_canonical_settled_start_state(self) -> None:
        expected = practice_state()
        with mock.patch.object(
            baseline,
            "wait_for_practice_state",
            return_value=expected,
        ) as wait_state:
            result = baseline.wait_for_practice_setup_ready(
                "session-1",
                123.0,
                expected_checkbox_checked=True,
            )

        self.assertIs(expected, result)
        wait_state.assert_called_once_with(
            "session-1",
            123.0,
            expected_checkbox_checked=True,
            require_all_selected=True,
            require_start_ready=True,
        )

    def test_practice_state_diagnostics_are_allowlisted_and_content_free(self) -> None:
        expected = practice_state(transition_observed=True)
        with mock.patch.object(
            baseline,
            "execute_read_only",
            return_value=expected,
        ) as execute_read_only:
            self.assertEqual(expected, baseline.practice_setup_state("session-1"))

        script = execute_read_only.call_args.args[1]
        self.assertEqual(baseline.PRACTICE_SETUP_STATE_KEYS, set(expected))
        self.assertNotIn("document.body.textContent", script)
        for forbidden in ("filename", "credential", "provider", "prompt", "response"):
            self.assertNotIn(forbidden, script.lower())

        secret = "private learner content"
        with mock.patch.object(
            baseline,
            "execute_read_only",
            return_value={**expected, "rawText": secret},
        ):
            with self.assertRaises(AssertionError) as invalid:
                baseline.practice_setup_state("session-1")
        self.assertNotIn(secret, str(invalid.exception))

    def test_practice_transition_observer_latches_bounded_selection_change(self) -> None:
        source = MODULE_PATH.read_text(encoding="utf-8")
        self.assertIn("sawSelectionChange: false", source)
        self.assertIn("initialSelectedCount: selectedCount()", source)
        self.assertIn("currentSelectedCount !== state.initialSelectedCount", source)
        self.assertIn("?.sawSelectionChange === true", source)
        self.assertIn("?.sawUpdating === true", source)

    def test_practice_state_timeout_reports_only_structured_diagnostics(self) -> None:
        stalled = practice_state(
            selected=1,
            checked=False,
            phase="error",
            start_enabled=False,
        )
        with mock.patch.object(
            baseline,
            "practice_setup_state",
            return_value=stalled,
        ), mock.patch.object(
            baseline.time,
            "monotonic",
            side_effect=[0.0, 2.0],
        ), mock.patch.object(baseline.time, "sleep"):
            with self.assertRaises(AssertionError) as timeout:
                baseline.wait_for_practice_state("session-1", 1.0)

        message = str(timeout.exception)
        self.assertIn("'workloadPhase': 'error'", message)
        self.assertEqual(set(stalled), baseline.PRACTICE_SETUP_STATE_KEYS)

    def test_selection_ready_requires_an_observed_workload_transition(self) -> None:
        stale = practice_state(selected=1, checked=False)
        reconciled = practice_state(
            selected=1,
            checked=False,
            transition_observed=True,
        )
        with mock.patch.object(
            baseline,
            "practice_setup_state",
            side_effect=[stale, reconciled],
        ) as read_state, mock.patch.object(baseline.time, "sleep"):
            result = baseline.wait_for_practice_selection("session-1", False, 10**20)

        self.assertIs(reconciled, result)
        self.assertEqual(2, read_state.call_count)

    def test_canonical_setup_selects_all_once_outside_measurement(self) -> None:
        events: list[str] = []
        partial = practice_state(
            selected=0,
            checked=False,
            phase="empty",
            start_enabled=False,
        )
        with mock.patch.object(
            baseline,
            "wait_for_practice_state",
            side_effect=lambda *_args, **_kwargs: events.append("discover") or partial,
        ) as wait_state, mock.patch.object(
            baseline.webdriver,
            "find_element",
            side_effect=lambda *_args: events.append("resolve-select-all") or "select-all",
        ), mock.patch.object(
            baseline,
            "begin_practice_workload_observation",
            side_effect=lambda *_args: events.append("observe"),
        ), mock.patch.object(
            baseline.webdriver,
            "click",
            side_effect=lambda *_args: events.append("click-select-all"),
        ) as click, mock.patch.object(
            baseline,
            "wait_for_practice_selection",
            side_effect=lambda *_args: events.append("settled"),
        ) as wait_selection, mock.patch.object(
            baseline,
            "end_practice_workload_observation",
            side_effect=lambda *_args: events.append("stop-observing"),
        ):
            baseline.canonicalize_practice_setup("session-1", 123.0)

        self.assertEqual(
            ["discover", "resolve-select-all", "observe", "click-select-all", "settled", "stop-observing"],
            events,
        )
        wait_state.assert_called_once_with(
            "session-1",
            123.0,
            require_workload_settled=False,
        )
        click.assert_called_once_with("session-1", "select-all")
        wait_selection.assert_called_once_with("session-1", True, 123.0)

    def test_canonical_setup_does_not_mutate_an_already_canonical_scope(self) -> None:
        with mock.patch.object(
            baseline,
            "wait_for_practice_state",
            return_value=practice_state(),
        ), mock.patch.object(
            baseline,
            "wait_for_practice_setup_ready",
        ) as wait_ready, mock.patch.object(
            baseline.webdriver,
            "find_element",
        ) as find_element, mock.patch.object(baseline.webdriver, "click") as click:
            baseline.canonicalize_practice_setup("session-1", 123.0)

        wait_ready.assert_called_once_with(
            "session-1",
            123.0,
            expected_checkbox_checked=True,
        )
        find_element.assert_not_called()
        click.assert_not_called()

    def test_active_practice_re_resolves_enabled_start_after_readiness(self) -> None:
        events: list[str] = []

        def find_element(_session: str, _strategy: str, selector: str, _deadline: float) -> str:
            if selector == baseline.PRACTICE_START_XPATH:
                events.append("resolve-start")
                return "enabled-start"
            if "Show answer" in selector:
                return "reveal"
            return "easy"

        with mock.patch.object(baseline, "navigate"), mock.patch.object(
            baseline,
            "canonicalize_practice_setup",
            side_effect=lambda *_args, **_kwargs: events.append("setup-ready"),
        ), mock.patch.object(
            baseline.webdriver,
            "find_element",
            side_effect=find_element,
        ), mock.patch.object(
            baseline.webdriver,
            "click",
            side_effect=lambda _session, element: events.append(f"click:{element}"),
        ), mock.patch.object(
            baseline,
            "wait_js",
            side_effect=lambda *_args: events.append("scenario-ready"),
        ), mock.patch.object(
            baseline,
            "arm",
            side_effect=lambda *_args: events.append("arm"),
        ), mock.patch.object(
            baseline,
            "finish",
            side_effect=lambda _session: events.append("finish") or {"sample": True},
        ):
            result = baseline.collect_scenario(
                "session-1",
                "active-practice-progress-v1",
                0,
            )

        self.assertEqual("setup-ready", events[0])
        self.assertEqual("resolve-start", events[1])
        self.assertEqual("click:enabled-start", events[2])
        self.assertLess(events.index("arm"), events.index("click:easy"))
        self.assertEqual({"sample": True}, result)

    def test_pointer_and_click_actions_are_not_retried_after_disconnect(self) -> None:
        with mock.patch.object(baseline, "navigate"), mock.patch.object(
            baseline.webdriver,
            "find_element",
            return_value="document-toggle",
        ), mock.patch.object(baseline, "arm"), mock.patch.object(
            baseline.webdriver,
            "click",
            side_effect=ConnectionResetError("ambiguous click delivery"),
        ) as click:
            with self.assertRaises(ConnectionResetError):
                baseline.collect_scenario("session-1", "document-list-interaction-v1", 0)
        click.assert_called_once_with("session-1", "document-toggle")

        with mock.patch.object(baseline, "navigate"), mock.patch.object(
            baseline.webdriver,
            "find_element",
            return_value="analytics-element",
        ), mock.patch.object(baseline, "park_mouse"), mock.patch.object(
            baseline,
            "arm",
        ), mock.patch.object(
            baseline,
            "pointer_actions",
            side_effect=ConnectionResetError("ambiguous pointer delivery"),
        ) as pointer_actions:
            with self.assertRaises(ConnectionResetError):
                baseline.collect_scenario("session-1", "navigation-hover-v1", 0)
        pointer_actions.assert_called_once_with(
            "session-1",
            "analytics-element",
            release=False,
        )

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
