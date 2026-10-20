from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

ROOT = Path(__file__).resolve().parents[2]
SMOKE_PATH = ROOT / "tests" / "webdriver" / "smoke.py"
LEGACY_SITECUSTOMIZE_PATH = ROOT / "tests" / "webdriver" / "sitecustomize.py"
DURABLE_LIBRARY_SMOKE_PATH = ROOT / "tests" / "webdriver" / "durable_library_smoke.py"
UI_PLATFORM_SMOKE_PATH = ROOT / "tests" / "webdriver" / "ui_platform_smoke.py"
NATIVE_SMOKE_RUNNER_PATH = ROOT / "scripts" / "smoke-native.sh"
PROGRESS_STYLES_PATH = ROOT / "src" / "ui" / "primitives" / "Progress.module.css"
PRACTICE_COMPLETE_PATH = ROOT / "src" / "screens" / "practice" / "PracticeComplete.tsx"
MAIN_ENTRY_PATH = ROOT / "src" / "main.tsx"


def load_smoke_module():
    spec = importlib.util.spec_from_file_location("ostudy_webdriver_smoke", SMOKE_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError("Could not load native WebDriver smoke module")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class WebDriverSmokeHarnessTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.smoke = load_smoke_module()

    def test_transport_has_no_implicit_sitecustomize_monkeypatch(self) -> None:
        self.assertFalse(
            LEGACY_SITECUSTOMIZE_PATH.exists(),
            "WebDriver retry ownership must stay explicit in smoke.py operations",
        )

    def test_raw_request_never_replays_mutating_execute_sync_after_disconnect(self) -> None:
        payload = {
            "script": """
                document.body.dataset.retrySentinel = "mutated";
                return document.readyState;
            """,
            "args": [],
        }
        disconnect = self.smoke.http.client.RemoteDisconnected("lost mutation response")
        with patch.object(
            self.smoke.urllib.request,
            "urlopen",
            side_effect=disconnect,
        ) as urlopen:
            with self.assertRaisesRegex(
                self.smoke.http.client.RemoteDisconnected,
                "lost mutation response",
            ):
                self.smoke.request(
                    "POST",
                    "/session/session-1/execute/sync",
                    payload,
                )
        urlopen.assert_called_once()
        sent = urlopen.call_args.args[0]
        self.assertEqual(sent.get_method(), "POST")
        self.assertTrue(sent.full_url.endswith("/session/session-1/execute/sync"))
        self.assertIn(b"retrySentinel", sent.data)
        self.assertIn(b"document.readyState", sent.data)

    def test_protocol_error_decodes_http_error_payload(self) -> None:
        error = self.smoke._protocol_error({"value": {"error": "no such element", "message": "missing"}}, 404)
        self.assertIsNotNone(error)
        assert error is not None
        self.assertEqual(error.status, 404)
        self.assertEqual(error.error, "no such element")
        self.assertEqual(error.message, "missing")

    def test_tutorial_versions_follow_registry(self) -> None:
        versions = self.smoke.tutorial_versions()
        self.assertGreater(len(versions), 0)
        self.assertTrue(all(isinstance(value, int) and value >= 1 for value in versions.values()))

    def test_prepare_automation_profile_stores_completed_tutorials(self) -> None:
        state = {}

        def fake_request(method: str, path: str, payload=None):
            self.assertEqual(method, "POST")
            self.assertTrue(path.endswith("/execute/sync"))
            assert payload is not None
            state["key"] = payload["args"][0]
            state["value"] = payload["args"][1]
            return True

        with patch.object(self.smoke, "request", side_effect=fake_request):
            self.smoke.prepare_automation_profile("session-1")
        self.assertEqual(state["key"], self.smoke.TUTORIAL_STORAGE_KEY)
        self.assertEqual(state["value"]["schemaVersion"], 1)
        self.assertEqual(state["value"]["completed"], self.smoke.tutorial_versions())
        self.assertEqual(state["value"]["dismissed"], {})

    def test_find_element_retries_only_retryable_lookup_errors(self) -> None:
        calls = 0

        def fake_request(method: str, path: str, payload=None):
            nonlocal calls
            calls += 1
            if calls == 1:
                raise self.smoke.WebDriverProtocolError(404, "no such element", "missing")
            return {self.smoke.ELEMENT_KEY: "element-1"}

        with patch.object(self.smoke, "request", side_effect=fake_request), patch.object(self.smoke.time, "sleep"):
            element = self.smoke.find_element("session-1", "css selector", "button[data-testid='nav-library']", self.smoke.deadline_after(1))
        self.assertEqual(element, "element-1")
        self.assertEqual(calls, 2)

    def test_find_element_does_not_retry_nonretryable_protocol_errors(self) -> None:
        with patch.object(self.smoke, "request", side_effect=self.smoke.WebDriverProtocolError(500, "unknown error", "boom")), patch.object(self.smoke.time, "sleep") as sleep:
            with self.assertRaises(self.smoke.WebDriverProtocolError):
                self.smoke.find_element("session-1", "css selector", "button", self.smoke.deadline_after(1))
        sleep.assert_not_called()

    def test_set_input_text_verifies_controlled_value(self) -> None:
        with patch.object(self.smoke, "find_element", return_value="element-1"), patch.object(self.smoke, "clear") as clear, patch.object(self.smoke, "send_keys") as send_keys, patch.object(self.smoke, "element_property", return_value="Expected"):
            element = self.smoke.set_input_text("session-1", "css selector", "input", "Expected", self.smoke.deadline_after(1))
        self.assertEqual(element, "element-1")
        clear.assert_called_once_with("session-1", "element-1")
        send_keys.assert_called_once_with("session-1", "element-1", "Expected")

    def test_set_rich_editor_text_uses_semantic_editable_contract(self) -> None:
        with patch.object(self.smoke, "find_elements", return_value=["editor-1"]), patch.object(self.smoke, "clear"), patch.object(self.smoke, "send_keys"), patch.object(self.smoke, "element_text", return_value="Question") as element_text:
            element = self.smoke.set_rich_editor_text("session-1", 0, "Question", self.smoke.deadline_after(1))
        self.assertEqual(element, "editor-1")
        element_text.assert_called_with("session-1", "editor-1")

    def test_wait_for_enabled_element_waits_for_react_state(self) -> None:
        with patch.object(self.smoke, "find_element", return_value="element-1"), patch.object(self.smoke, "element_enabled", side_effect=[False, True]), patch.object(self.smoke.time, "sleep"):
            element = self.smoke.wait_for_enabled_element("session-1", "css selector", "button", self.smoke.deadline_after(1))
        self.assertEqual(element, "element-1")

    def test_click_retries_transient_ack_arm_disconnect_once_before_one_click(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(side_effect=[self.smoke.http.client.RemoteDisconnected("lost arm response"), True, None])
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(self.smoke, "_wait_for_click_ack", return_value=True), patch.object(self.smoke, "request", request), patch.object(self.smoke.time, "sleep"):
            self.smoke.click("session-1", "element-1")
        self.assertEqual(request.call_count, 3)
        first_arm, second_arm, click = request.call_args_list
        self.assertEqual(first_arm, second_arm)
        self.assertEqual(first_arm.args[0:2], ("POST", "/session/session-1/execute/sync"))
        self.assertEqual(first_arm.args[2]["args"][0], {self.smoke.ELEMENT_KEY: "element-1"})
        self.assertEqual(first_arm.args[2]["args"][1], second_arm.args[2]["args"][1])
        self.assertEqual(click.args, ("POST", "/session/session-1/element/element-1/click", {}))

    def test_click_stops_after_second_transient_ack_arm_disconnect(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(side_effect=[self.smoke.http.client.RemoteDisconnected("lost first arm response"), self.smoke.http.client.RemoteDisconnected("lost second arm response")])
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(self.smoke, "request", request), patch.object(self.smoke.time, "sleep"):
            with self.assertRaisesRegex(self.smoke.http.client.RemoteDisconnected, "lost second arm response"):
                self.smoke.click("session-1", "element-1")
        self.assertEqual(request.call_count, 2)
        self.assertEqual(request.call_args_list[0], request.call_args_list[1])
        self.assertTrue(all(call.args[1] == "/session/session-1/execute/sync" for call in request.call_args_list))

    def test_click_does_not_retry_nontransient_ack_arm_transport_error(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(side_effect=OSError("arm failed"))
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(self.smoke, "request", request), patch.object(self.smoke.time, "sleep"):
            with self.assertRaisesRegex(OSError, "arm failed"):
                self.smoke.click("session-1", "element-1")
        request.assert_called_once()
        self.assertEqual(request.call_args.args[1], "/session/session-1/execute/sync")

    def test_click_does_not_retry_ack_arm_protocol_error(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        protocol_error = self.smoke.WebDriverProtocolError(500, "unknown error", "arm failed")
        request = MagicMock(side_effect=protocol_error)
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(self.smoke, "request", request), patch.object(self.smoke.time, "sleep"):
            with self.assertRaises(self.smoke.WebDriverProtocolError):
                self.smoke.click("session-1", "element-1")
        request.assert_called_once()
        self.assertEqual(request.call_args.args[1], "/session/session-1/execute/sync")

    def test_click_rejects_false_ack_arm_result_before_click(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(return_value=False)
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(self.smoke, "request", request), patch.object(self.smoke.time, "sleep"):
            with self.assertRaisesRegex(RuntimeError, "Could not arm DOM acknowledgement"):
                self.smoke.click("session-1", "element-1")
        request.assert_called_once()
        self.assertEqual(request.call_args.args[1], "/session/session-1/execute/sync")

    def test_click_requires_stable_hit_target_and_dom_ack(self) -> None:
        hits = [{"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}, {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}]
        request = MagicMock(return_value=None)
        with patch.object(self.smoke, "_element_hit_test", side_effect=hits), patch.object(self.smoke, "_arm_click_ack"), patch.object(self.smoke, "_wait_for_click_ack", return_value=True), patch.object(self.smoke, "request", request), patch.object(self.smoke.time, "sleep"):
            self.smoke.click("session-1", "element-1")
        request.assert_called_once_with("POST", "/session/session-1/element/element-1/click", {})

    def test_click_recovers_read_only_hit_test_disconnect_without_replaying_click(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        hit_test = MagicMock(side_effect=[self.smoke.http.client.RemoteDisconnected("lost hit-test response"), hit, hit])
        request = MagicMock(return_value=None)
        with patch.object(self.smoke, "_element_hit_test", hit_test), patch.object(self.smoke, "_arm_click_ack"), patch.object(self.smoke, "_wait_for_click_ack", return_value=True), patch.object(self.smoke, "request", request), patch.object(self.smoke.time, "sleep"):
            self.smoke.click("session-1", "element-1")
        self.assertEqual(hit_test.call_count, 3)
        request.assert_called_once_with("POST", "/session/session-1/element/element-1/click", {})

    def test_click_propagates_nontransient_hit_test_failure_before_click(self) -> None:
        request = MagicMock(return_value=None)
        with patch.object(self.smoke, "_element_hit_test", side_effect=OSError("hit-test failed")), patch.object(self.smoke, "request", request), patch.object(self.smoke.time, "sleep") as sleep:
            with self.assertRaisesRegex(OSError, "hit-test failed"):
                self.smoke.click("session-1", "element-1")
        request.assert_not_called()
        sleep.assert_not_called()

    def test_click_accepts_acknowledged_disconnect_without_replaying_click(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(side_effect=self.smoke.http.client.RemoteDisconnected("lost click response"))
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(self.smoke, "_arm_click_ack"), patch.object(self.smoke, "_wait_for_click_ack", return_value=True), patch.object(self.smoke, "request", request), patch.object(self.smoke.time, "sleep"):
            self.smoke.click("session-1", "element-1")
        request.assert_called_once_with("POST", "/session/session-1/element/element-1/click", {})

    def test_click_propagates_unacknowledged_disconnect_without_replay(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(side_effect=self.smoke.http.client.RemoteDisconnected("lost click response"))
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(self.smoke, "_arm_click_ack"), patch.object(self.smoke, "_wait_for_click_ack", return_value=False), patch.object(self.smoke, "request", request), patch.object(self.smoke.time, "sleep"):
            with self.assertRaisesRegex(self.smoke.http.client.RemoteDisconnected, "lost click response"):
                self.smoke.click("session-1", "element-1")
        request.assert_called_once_with("POST", "/session/session-1/element/element-1/click", {})

    def test_click_rejects_repeated_unacknowledged_webdriver_clicks(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(return_value=None)
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(self.smoke, "_arm_click_ack"), patch.object(self.smoke, "_wait_for_click_ack", return_value=False), patch.object(self.smoke, "request", request), patch.object(self.smoke.time, "sleep"):
            with self.assertRaisesRegex(RuntimeError, "without DOM acknowledgement"):
                self.smoke.click("session-1", "element-1")
        self.assertEqual(request.call_count, 2)

    def test_native_smoke_uses_structural_practice_completion_contract(self) -> None:
        harness = SMOKE_PATH.read_text(encoding="utf-8")
        completion = PRACTICE_COMPLETE_PATH.read_text(encoding="utf-8")
        self.assertIn('data-testid="practice-complete"', completion)
        self.assertIn("PRACTICE_COMPLETE_SELECTOR", harness)
        self.assertNotIn('"Session complete"', harness)

    def test_native_smoke_verifies_analytics_recent_tag_search(self) -> None:
        harness = SMOKE_PATH.read_text(encoding="utf-8")
        for required in ("def verify_search_workspace_selected", "def verify_tag_filtered_search", "def verify_analytics_tag_search", "Search cards tagged {tag_name}", "Native smoke tag", "verify_analytics_tag_search(session_id, question, tag_name, deadline)"):
            self.assertIn(required, harness)

    def test_persistence_restart_uses_explicit_accessible_deck_open_control(self) -> None:
        harness = SMOKE_PATH.read_text(encoding="utf-8")
        durable_harness = DURABLE_LIBRARY_SMOKE_PATH.read_text(encoding="utf-8")
        harness_start = harness.index("def verify_persistence_after_restart(")
        harness_end = harness.index("\ndef main()", harness_start)
        durable_start = durable_harness.index("def verify_persistence_after_restart(")
        durable_end = durable_harness.index("\n\nif __name__", durable_start)
        implementations = {"generic": harness[harness_start:harness_end], "durable": durable_harness[durable_start:durable_end]}
        for implementation, persistence in implementations.items():
            with self.subTest(implementation=implementation):
                # The Workbench's tree opens the deck; its card list shows the persisted card.
                self.assertIn("open_workbench_deck(session_id, deck_name, deadline)", persistence)
                self.assertIn("find_card_row(session_id, question, deadline)", persistence)
                self.assertNotIn("button[.//strong", persistence)

    def test_tag_filtered_search_contract_requires_titled_current_tag_and_card(self) -> None:
        with patch.object(self.smoke, "find_element", side_effect=["title", "current-tag", "reviewed-card"]) as find_element:
            self.smoke.verify_tag_filtered_search("session-1", "What persisted?", "Native smoke tag", 42.0)
        self.assertEqual(3, find_element.call_count)
        selectors = [call.args[2] for call in find_element.call_args_list]
        self.assertIn("workbench-title", selectors[0])
        self.assertIn('"Native smoke tag"', selectors[0])
        self.assertIn("@aria-current='true'", selectors[1])
        self.assertIn('"Native smoke tag"', selectors[1])
        self.assertIn("@data-card-id", selectors[2])
        self.assertIn("What persisted?", selectors[2])

    def test_durable_smoke_restores_stage_five_search_workspace_checkpoint(self) -> None:
        source = DURABLE_LIBRARY_SMOKE_PATH.read_text(encoding="utf-8")
        self.assertIn("Verifying restored Search workspace checkpoint after restart", source)
        self.assertIn("webdriver.verify_search_workspace_selected(", source)
        self.assertNotIn("webdriver.verify_tag_filtered_search(", source)
        self.assertNotIn("Verifying restored Analytics checkpoint after restart", source)

    def test_internal_practice_shell_requires_inline_progress_accessibility(self) -> None:
        state = {"selectedPrimary": ["nav-home"], "practiceDestinations": 0, "heading": "Cardiology", "progressValueText": "Card 1 of 1, 0 studied, 1 remaining, 0% complete", "formerHeaderProgressCount": 0}
        with patch.object(self.smoke, "request", return_value=state):
            self.smoke.assert_internal_practice_shell("session-1", "Cardiology")
        for key, invalid_value in (
            ("progressValueText", "0 of 1"),
            ("formerHeaderProgressCount", 1),
            # A session runs inside Home: no other destination, and not none.
            ("selectedPrimary", []),
            ("selectedPrimary", ["nav-library"]),
            ("practiceDestinations", 1),
        ):
            with self.subTest(key=key), patch.object(self.smoke, "request", return_value={**state, key: invalid_value}):
                with self.assertRaises(AssertionError):
                    self.smoke.assert_internal_practice_shell("session-1", "Cardiology")

    def test_library_views_open_through_main_navigation_then_the_library_tabs(self) -> None:
        with patch.object(self.smoke, "find_element", side_effect=lambda *args: args[2]) as find_element, patch.object(self.smoke, "click") as click, patch.object(self.smoke, "wait_for_main_heading") as heading:
            self.smoke.open_library_view("session-1", "study-guides", 42.0)
        clicked = [call.args[1] for call in click.call_args_list]
        self.assertEqual(clicked[0], "button[data-testid='nav-library']")
        self.assertIn("nav[aria-label='Library pages']", clicked[1])
        self.assertIn("library-view-study-guides", clicked[1])
        selectors = [call.args[2] for call in find_element.call_args_list]
        self.assertIn("button[data-testid='nav-library'][aria-current='page']", selectors)
        self.assertIn("button[data-testid='library-view-study-guides'][aria-current='page']", selectors)
        heading.assert_called_once_with("session-1", "Study Guides", 42.0)

    def test_today_count_must_agree_on_home_and_its_badge(self) -> None:
        with patch.object(self.smoke, "find_element", side_effect=lambda *args: args[2]) as find_element, patch.object(self.smoke, "click") as click:
            self.smoke.verify_due_today("session-1", "3 cards due today", 42.0)
        self.assertEqual([call.args[1] for call in click.call_args_list], ["button[data-testid='nav-home']"])
        selectors = [call.args[2] for call in find_element.call_args_list]
        self.assertTrue(any("home-today" in selector and '"3 cards due today"' in selector for selector in selectors))
        self.assertTrue(any("nav-home" in selector and "nav-badge" in selector and '"3"' in selector for selector in selectors))
        self.assertTrue(any("subject-tile" in selector and "No subject" in selector for selector in selectors))

    def test_a_fresh_library_must_open_on_home(self) -> None:
        with patch.object(self.smoke, "find_element", side_effect=lambda *args: args[2]) as find_element, patch.object(self.smoke, "wait_for_main_heading") as heading:
            self.smoke.verify_fresh_home("session-1", 42.0)
        heading.assert_called_once_with("session-1", "Your library is empty", 42.0)
        selectors = [call.args[2] for call in find_element.call_args_list]
        self.assertEqual(selectors[0], "button[data-testid='nav-home'][aria-current='page']")
        for label in ("Create a subject", "Import a document", "Open Library"):
            self.assertTrue(any(json.dumps(label) in selector for selector in selectors), label)

    def test_native_smoke_pins_x11_software_rendering_before_driver_launch(self) -> None:
        source = NATIVE_SMOKE_RUNNER_PATH.read_text(encoding="utf-8")
        launch = source.index("timeout --signal=INT --kill-after=30s 300s")
        for assignment in ("export GDK_BACKEND=x11", "export WEBKIT_DISABLE_COMPOSITING_MODE=1", "export WEBKIT_DISABLE_DMABUF_RENDERER=1", "export LIBGL_ALWAYS_SOFTWARE=1"):
            with self.subTest(assignment=assignment):
                self.assertIn(assignment, source)
                self.assertLess(source.index(assignment), launch)
        self.assertIn("Native smoke is an X11/WebKit automation contract", source)
        self.assertIn("can reset WebKitWebDriver mid-session", source)

    def test_native_smoke_timeout_retains_hard_kill_fallback(self) -> None:
        source = NATIVE_SMOKE_RUNNER_PATH.read_text(encoding="utf-8")
        self.assertIn("timeout --signal=INT --kill-after=30s 300s", source)
        self.assertNotIn("timeout --signal=INT 300s", source)

    def test_deck_detail_smoke_retains_visual_geometry_contract(self) -> None:
        source = UI_PLATFORM_SMOKE_PATH.read_text(encoding="utf-8")
        for invariant in (
            "const contained = [heading, actions].every(",
            "noHeaderOverflow: header.scrollWidth <= header.clientWidth + 1",
            'snapshot.get("labels") == expected_direct',
            'snapshot.get("contained") is True',
            'snapshot.get("noDocumentOverflow") is True',
            'overflow.get("items") != expected_items',
            '("compact", 390, 700, "sheet")',
        ):
            with self.subTest(invariant=invariant):
                self.assertIn(invariant, source)

    def test_progress_filled_content_retains_all_four_tone_colors(self) -> None:
        source = PROGRESS_STYLES_PATH.read_text(encoding="utf-8")
        colors = {"accent": "--progress-filled-content-color: var(--ostudy-color-accent-on)", "success": "--progress-filled-content-color: var(--ostudy-color-text-inverse)", "warning": "--progress-filled-content-color: var(--ostudy-color-neutral-950)", "danger": "--progress-filled-content-color: var(--ostudy-color-status-danger-on)"}
        for tone, color in colors.items():
            with self.subTest(tone=tone):
                start = source.index(f".{tone} {{")
                end = source.index("}", start)
                self.assertIn(color, source[start:end])
        warning_start = source.index(".warning {")
        warning_end = source.index("}", warning_start)
        self.assertNotIn("--ostudy-color-accent-on", source[warning_start:warning_end])

    def test_android_smoke_entry_point_installs_test_bridge_only_when_requested(self) -> None:
        source = MAIN_ENTRY_PATH.read_text(encoding="utf-8")
        self.assertIn('import.meta.env.VITE_OSTUDY_ANDROID_TEST_BRIDGE === "1"', source)
        self.assertIn('import("./testing/androidTestBridge")', source)
        self.assertIn("installAndroidTestBridge();", source)
        self.assertIn("setAndroidTestBridgeDiagnostic", source)


if __name__ == "__main__":
    unittest.main()
