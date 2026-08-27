from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

ROOT = Path(__file__).resolve().parents[2]
SMOKE_PATH = ROOT / "tests" / "webdriver" / "smoke.py"
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

    def test_android_smoke_entry_point_installs_test_bridge_only_when_requested(self) -> None:
        source = MAIN_ENTRY_PATH.read_text(encoding="utf-8")
        self.assertIn('import.meta.env.VITE_OSTUDY_ANDROID_TEST_BRIDGE === "1"', source)
        self.assertIn('import("./testing/androidTestBridge")', source)
        self.assertIn("installAndroidTestBridge();", source)
        self.assertIn("setAndroidTestBridgeDiagnostic", source)


if __name__ == "__main__":
    unittest.main()
