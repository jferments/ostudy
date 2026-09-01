from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

ROOT = Path(__file__).resolve().parents[2]
SMOKE_PATH = ROOT / "tests" / "webdriver" / "smoke.py"
DURABLE_LIBRARY_SMOKE_PATH = ROOT / "tests" / "webdriver" / "durable_library_smoke.py"
UI_PLATFORM_SMOKE_PATH = ROOT / "tests" / "webdriver" / "ui_platform_smoke.py"
NATIVE_SMOKE_RUNNER_PATH = ROOT / "scripts" / "smoke-native.sh"
PROGRESS_STYLES_PATH = ROOT / "src" / "ui" / "primitives" / "Progress.module.css"
PRACTICE_COMPLETE_PATH = ROOT / "src" / "screens" / "practice" / "PracticeComplete.tsx"
DECKS_SCREEN_PATH = ROOT / "src" / "screens" / "DecksScreen.tsx"
CARDS_SCREEN_PATH = ROOT / "src" / "screens" / "CardsScreen.tsx"
CARD_BROWSER_WORKSPACE_PATH = ROOT / "src" / "screens" / "cards" / "CardBrowserWorkspace.tsx"
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

    def test_protocol_error_decodes_http_error_payload(self) -> None:
        error = self.smoke._protocol_error(
            {"value": {"error": "no such element", "message": "missing"}},
            404,
        )
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

        with patch.object(self.smoke, "request", side_effect=fake_request), patch.object(
            self.smoke.time,
            "sleep",
        ):
            element = self.smoke.find_element(
                "session-1",
                "css selector",
                "button[data-testid='nav-decks']",
                self.smoke.deadline_after(1),
            )

        self.assertEqual(element, "element-1")
        self.assertEqual(calls, 2)

    def test_find_element_does_not_retry_nonretryable_protocol_errors(self) -> None:
        with patch.object(
            self.smoke,
            "request",
            side_effect=self.smoke.WebDriverProtocolError(500, "unknown error", "boom"),
        ), patch.object(self.smoke.time, "sleep") as sleep:
            with self.assertRaises(self.smoke.WebDriverProtocolError):
                self.smoke.find_element(
                    "session-1",
                    "css selector",
                    "button",
                    self.smoke.deadline_after(1),
                )
        sleep.assert_not_called()

    def test_set_input_text_verifies_controlled_value(self) -> None:
        with patch.object(self.smoke, "find_element", return_value="element-1"), patch.object(
            self.smoke,
            "clear",
        ) as clear, patch.object(self.smoke, "send_keys") as send_keys, patch.object(
            self.smoke,
            "element_property",
            return_value="Expected",
        ):
            element = self.smoke.set_input_text(
                "session-1",
                "css selector",
                "input",
                "Expected",
                self.smoke.deadline_after(1),
            )
        self.assertEqual(element, "element-1")
        clear.assert_called_once_with("session-1", "element-1")
        send_keys.assert_called_once_with("session-1", "element-1", "Expected")

    def test_set_rich_editor_text_uses_semantic_editable_contract(self) -> None:
        with patch.object(self.smoke, "find_elements", return_value=["editor-1"]), patch.object(
            self.smoke,
            "clear",
        ), patch.object(self.smoke, "send_keys"), patch.object(
            self.smoke,
            "element_text",
            return_value="Question",
        ) as element_text:
            element = self.smoke.set_rich_editor_text(
                "session-1",
                0,
                "Question",
                self.smoke.deadline_after(1),
            )
        self.assertEqual(element, "editor-1")
        element_text.assert_called_with("session-1", "editor-1")

    def test_wait_for_enabled_element_waits_for_react_state(self) -> None:
        with patch.object(self.smoke, "find_element", return_value="element-1"), patch.object(
            self.smoke,
            "element_enabled",
            side_effect=[False, True],
        ), patch.object(self.smoke.time, "sleep"):
            element = self.smoke.wait_for_enabled_element(
                "session-1",
                "css selector",
                "button",
                self.smoke.deadline_after(1),
            )
        self.assertEqual(element, "element-1")

    def test_click_retries_transient_ack_arm_disconnect_once_before_one_click(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(
            side_effect=[
                self.smoke.http.client.RemoteDisconnected("lost arm response"),
                True,
                None,
            ]
        )
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(
            self.smoke,
            "_wait_for_click_ack",
            return_value=True,
        ), patch.object(self.smoke, "request", request), patch.object(
            self.smoke.time,
            "sleep",
        ):
            self.smoke.click("session-1", "element-1")

        self.assertEqual(request.call_count, 3)
        first_arm, second_arm, click = request.call_args_list
        self.assertEqual(first_arm, second_arm)
        self.assertEqual(first_arm.args[0:2], ("POST", "/session/session-1/execute/sync"))
        self.assertEqual(first_arm.args[2]["args"][0], {self.smoke.ELEMENT_KEY: "element-1"})
        self.assertEqual(first_arm.args[2]["args"][1], second_arm.args[2]["args"][1])
        self.assertEqual(
            click.args,
            ("POST", "/session/session-1/element/element-1/click", {}),
        )

    def test_click_stops_after_second_transient_ack_arm_disconnect(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(
            side_effect=[
                self.smoke.http.client.RemoteDisconnected("lost first arm response"),
                self.smoke.http.client.RemoteDisconnected("lost second arm response"),
            ]
        )
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(
            self.smoke,
            "request",
            request,
        ), patch.object(self.smoke.time, "sleep"):
            with self.assertRaisesRegex(
                self.smoke.http.client.RemoteDisconnected,
                "lost second arm response",
            ):
                self.smoke.click("session-1", "element-1")

        self.assertEqual(request.call_count, 2)
        self.assertEqual(request.call_args_list[0], request.call_args_list[1])
        self.assertTrue(
            all(call.args[1] == "/session/session-1/execute/sync" for call in request.call_args_list)
        )

    def test_click_does_not_retry_nontransient_ack_arm_transport_error(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(side_effect=OSError("arm failed"))
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(
            self.smoke,
            "request",
            request,
        ), patch.object(self.smoke.time, "sleep"):
            with self.assertRaisesRegex(OSError, "arm failed"):
                self.smoke.click("session-1", "element-1")

        request.assert_called_once()
        self.assertEqual(request.call_args.args[1], "/session/session-1/execute/sync")

    def test_click_does_not_retry_ack_arm_protocol_error(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        protocol_error = self.smoke.WebDriverProtocolError(500, "unknown error", "arm failed")
        request = MagicMock(side_effect=protocol_error)
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(
            self.smoke,
            "request",
            request,
        ), patch.object(self.smoke.time, "sleep"):
            with self.assertRaises(self.smoke.WebDriverProtocolError):
                self.smoke.click("session-1", "element-1")

        request.assert_called_once()
        self.assertEqual(request.call_args.args[1], "/session/session-1/execute/sync")

    def test_click_rejects_false_ack_arm_result_before_click(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(return_value=False)
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(
            self.smoke,
            "request",
            request,
        ), patch.object(self.smoke.time, "sleep"):
            with self.assertRaisesRegex(RuntimeError, "Could not arm DOM acknowledgement"):
                self.smoke.click("session-1", "element-1")

        request.assert_called_once()
        self.assertEqual(request.call_args.args[1], "/session/session-1/execute/sync")

    def test_click_requires_stable_hit_target_and_dom_ack(self) -> None:
        hits = [
            {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}},
            {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}},
        ]
        request = MagicMock(return_value=None)
        with patch.object(self.smoke, "_element_hit_test", side_effect=hits), patch.object(
            self.smoke,
            "_arm_click_ack",
        ), patch.object(self.smoke, "_wait_for_click_ack", return_value=True), patch.object(
            self.smoke,
            "request",
            request,
        ), patch.object(self.smoke.time, "sleep"):
            self.smoke.click("session-1", "element-1")

        request.assert_called_once_with("POST", "/session/session-1/element/element-1/click", {})

    def test_click_recovers_read_only_hit_test_disconnect_without_replaying_click(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        hit_test = MagicMock(
            side_effect=[
                self.smoke.http.client.RemoteDisconnected("lost hit-test response"),
                hit,
                hit,
            ]
        )
        request = MagicMock(return_value=None)
        with patch.object(self.smoke, "_element_hit_test", hit_test), patch.object(
            self.smoke,
            "_arm_click_ack",
        ), patch.object(self.smoke, "_wait_for_click_ack", return_value=True), patch.object(
            self.smoke,
            "request",
            request,
        ), patch.object(self.smoke.time, "sleep"):
            self.smoke.click("session-1", "element-1")

        self.assertEqual(hit_test.call_count, 3)
        request.assert_called_once_with("POST", "/session/session-1/element/element-1/click", {})

    def test_click_propagates_nontransient_hit_test_failure_before_click(self) -> None:
        hit_test_error = OSError("hit-test failed")
        request = MagicMock(return_value=None)
        with patch.object(
            self.smoke,
            "_element_hit_test",
            side_effect=hit_test_error,
        ), patch.object(self.smoke, "request", request), patch.object(
            self.smoke.time,
            "sleep",
        ) as sleep:
            with self.assertRaisesRegex(OSError, "hit-test failed"):
                self.smoke.click("session-1", "element-1")

        request.assert_not_called()
        sleep.assert_not_called()

    def test_click_accepts_acknowledged_disconnect_without_replaying_click(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(
            side_effect=self.smoke.http.client.RemoteDisconnected("lost click response")
        )
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(
            self.smoke,
            "_arm_click_ack",
        ), patch.object(self.smoke, "_wait_for_click_ack", return_value=True), patch.object(
            self.smoke,
            "request",
            request,
        ), patch.object(self.smoke.time, "sleep"):
            self.smoke.click("session-1", "element-1")

        request.assert_called_once_with("POST", "/session/session-1/element/element-1/click", {})

    def test_click_propagates_unacknowledged_disconnect_without_replay(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(
            side_effect=self.smoke.http.client.RemoteDisconnected("lost click response")
        )
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(
            self.smoke,
            "_arm_click_ack",
        ), patch.object(self.smoke, "_wait_for_click_ack", return_value=False), patch.object(
            self.smoke,
            "request",
            request,
        ), patch.object(self.smoke.time, "sleep"):
            with self.assertRaisesRegex(
                self.smoke.http.client.RemoteDisconnected,
                "lost click response",
            ):
                self.smoke.click("session-1", "element-1")

        request.assert_called_once_with("POST", "/session/session-1/element/element-1/click", {})

    def test_click_rejects_repeated_unacknowledged_webdriver_clicks(self) -> None:
        hit = {"ready": True, "rect": {"left": 1, "top": 2, "width": 10, "height": 12}}
        request = MagicMock(return_value=None)
        with patch.object(self.smoke, "_element_hit_test", return_value=hit), patch.object(
            self.smoke,
            "_arm_click_ack",
        ), patch.object(self.smoke, "_wait_for_click_ack", return_value=False), patch.object(
            self.smoke,
            "request",
            request,
        ), patch.object(self.smoke.time, "sleep"):
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

        self.assertIn("def verify_search_workspace_selected", harness)
        self.assertIn("def verify_tag_filtered_search", harness)
        self.assertIn("def verify_analytics_tag_search", harness)
        self.assertIn("Search cards tagged {tag_name}", harness)
        self.assertIn("Tag: {tag_name}", harness)
        self.assertIn("Native smoke tag", harness)
        self.assertIn("verify_analytics_tag_search(session_id, question, tag_name, deadline)", harness)

    def test_persistence_restart_uses_explicit_accessible_deck_open_control(self) -> None:
        harness = SMOKE_PATH.read_text(encoding="utf-8")
        start = harness.index("def verify_persistence_after_restart(")
        end = harness.index("\ndef main()", start)
        persistence = harness[start:end]

        self.assertIn("//main//button[@aria-label=", persistence)
        self.assertIn("Open {deck_name}", persistence)
        self.assertNotIn("button[.//strong", persistence)

    def test_tag_filtered_search_contract_requires_selected_tab_filter_and_card(self) -> None:
        with patch.object(self.smoke, "wait_for_main_heading") as heading, patch.object(
            self.smoke,
            "find_element",
            side_effect=["search-tab", "tag-filter", "reviewed-card"],
        ) as find_element:
            self.smoke.verify_tag_filtered_search(
                "session-1",
                "What persisted?",
                "Native smoke tag",
                42.0,
            )

        heading.assert_called_once_with("session-1", "Flashcards", 42.0)
        self.assertEqual(3, find_element.call_count)
        selectors = [call.args[2] for call in find_element.call_args_list]
        self.assertIn("@aria-selected='true'", selectors[0])
        self.assertIn("Search", selectors[0])
        self.assertIn("Remove Tag: Native smoke tag filter", selectors[1])
        self.assertIn("What persisted?", selectors[2])

    def test_durable_smoke_restores_stage_five_search_workspace_checkpoint(self) -> None:
        source = DURABLE_LIBRARY_SMOKE_PATH.read_text(encoding="utf-8")

        self.assertIn("Verifying restored Search workspace checkpoint after restart", source)
        self.assertIn("webdriver.verify_search_workspace_selected(", source)
        self.assertNotIn("webdriver.verify_tag_filtered_search(", source)
        self.assertNotIn("Verifying restored Analytics checkpoint after restart", source)

    def test_internal_practice_shell_requires_inline_progress_accessibility(self) -> None:
        state = {
            "selectedPrimary": [],
            "practiceDestinations": 0,
            "heading": "Cardiology",
            "progressValueText": (
                "Card 1 of 1, 0 studied, 1 remaining, 0% complete"
            ),
            "formerHeaderProgressCount": 0,
        }
        with patch.object(self.smoke, "request", return_value=state):
            self.smoke.assert_internal_practice_shell("session-1", "Cardiology")

        for key, invalid_value in (
            ("progressValueText", "0 of 1"),
            ("formerHeaderProgressCount", 1),
        ):
            invalid = {**state, key: invalid_value}
            with self.subTest(key=key), patch.object(
                self.smoke,
                "request",
                return_value=invalid,
            ):
                with self.assertRaises(AssertionError):
                    self.smoke.assert_internal_practice_shell("session-1", "Cardiology")

    def test_deck_and_search_delegate_to_one_card_browser(self) -> None:
        decks = DECKS_SCREEN_PATH.read_text(encoding="utf-8")
        search = CARDS_SCREEN_PATH.read_text(encoding="utf-8")
        browser = CARD_BROWSER_WORKSPACE_PATH.read_text(encoding="utf-8")

        self.assertIn("<CardBrowserWorkspace", decks)
        self.assertIn("fixedDeckId={selectedDeck.id}", decks)
        self.assertIn("showFilters={false}", decks)
        self.assertNotIn("controller.cards(", decks)
        self.assertNotIn('className="card-row-preview"', decks)

        self.assertIn("<CardBrowserWorkspace", search)
        self.assertIn('controls="search"', search)
        self.assertNotIn("Bulk add", search)
        self.assertNotIn("Add flashcard", search)
        self.assertNotIn("organized-card-row", search)

        self.assertIn('export type CardBrowserControls = "deck" | "search"', browser)
        self.assertIn('controlMode === "search"', browser)
        self.assertIn('aria-label="Search this deck"', browser)
        self.assertIn('controlMode === "deck"', browser)
        self.assertIn(
            'ownedClassNames(ownedStyleMaps, "card-browser-actions")',
            browser,
        )
        self.assertIn("setBulkEntryOpen(true)", browser)
        self.assertIn("Bulk add", browser)
        self.assertIn("Add flashcard", browser)
        self.assertIn("Practice selected", browser)
        self.assertNotIn("Practice results", browser)
        self.assertNotIn("Ready now", browser)
        self.assertIn(
            'ownedClassNames(ownedStyleMaps, "organized-card-sides card-listing-sides")',
            browser,
        )
        self.assertIn("Q:", browser)
        self.assertIn("A:", browser)

    def test_native_smoke_pins_x11_software_rendering_before_driver_launch(self) -> None:
        source = NATIVE_SMOKE_RUNNER_PATH.read_text(encoding="utf-8")
        launch = source.index("timeout --signal=INT 300s")

        for assignment in (
            "export GDK_BACKEND=x11",
            "export WEBKIT_DISABLE_COMPOSITING_MODE=1",
            "export WEBKIT_DISABLE_DMABUF_RENDERER=1",
            "export LIBGL_ALWAYS_SOFTWARE=1",
        ):
            with self.subTest(assignment=assignment):
                self.assertIn(assignment, source)
                self.assertLess(source.index(assignment), launch)
        self.assertIn("Native smoke is an X11/WebKit automation contract", source)
        self.assertIn("can reset WebKitWebDriver mid-session", source)

    def test_deck_detail_smoke_retains_visual_geometry_contract(self) -> None:
        source = UI_PLATFORM_SMOKE_PATH.read_text(encoding="utf-8")

        for rectangle in (
            "header: rect(header)",
            "back: rect(back)",
            "titleRow: rect(titleRow)",
            "heading: rect(heading)",
            "description: rect(description)",
            "actions: rect(actions)",
        ):
            with self.subTest(rectangle=rectangle):
                self.assertIn(rectangle, source)
        self.assertIn("upper.bottom <= lower.top + 1", source)
        self.assertIn("above(geometry.back, geometry.titleRow)", source)
        self.assertIn("above(geometry.heading, geometry.description)", source)
        self.assertIn("above(geometry.description, geometry.actions)", source)
        self.assertIn("Math.abs(geometry.titleRow.left - headerRect.left) <= 1", source)
        self.assertIn("Math.abs(geometry.titleRow.right - headerRect.right) <= 1", source)
        self.assertIn("geometry.titleRow.width >= headerRect.width - 2", source)
        self.assertIn('snapshot.get("visuallyOrdered") is True', source)
        self.assertIn('snapshot.get("titleRowFullWidth") is True', source)

    def test_progress_filled_content_retains_all_four_tone_colors(self) -> None:
        source = PROGRESS_STYLES_PATH.read_text(encoding="utf-8")
        expected_colors = {
            "accent": "--progress-filled-content-color: var(--ostudy-color-accent-on)",
            "success": "--progress-filled-content-color: var(--ostudy-color-text-inverse)",
            "warning": "--progress-filled-content-color: var(--ostudy-color-neutral-950)",
            "danger": "--progress-filled-content-color: var(--ostudy-color-status-danger-on)",
        }

        for tone, color in expected_colors.items():
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
