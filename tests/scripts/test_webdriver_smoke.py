from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SMOKE_PATH = ROOT / "tests" / "webdriver" / "smoke.py"
MAIN_ENTRY_PATH = ROOT / "src" / "main.tsx"
ANDROID_PACKAGE_VERIFIER_PATH = ROOT / "scripts" / "verify-android-package.sh"
PRACTICE_COMPLETE_PATH = ROOT / "src" / "screens" / "practice" / "PracticeComplete.tsx"
CREATE_CARDS_PATH = ROOT / "src" / "screens" / "flashcards" / "CreateCardsWorkspace.tsx"
TOUR_PROVIDER_PATH = ROOT / "src" / "ui" / "tutorials" / "TourProvider.tsx"
SPEC = importlib.util.spec_from_file_location("ostudy_webdriver_smoke", SMOKE_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"Could not load native WebDriver harness from {SMOKE_PATH}")
smoke = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(smoke)


class WebDriverSmokeHarnessTests(unittest.TestCase):
    def test_tutorial_versions_are_discovered_from_the_registry(self) -> None:
        versions = smoke.tutorial_versions()
        self.assertTrue({"orientation", "search", "practice", "documents"}.issubset(versions))
        self.assertTrue(all(version >= 1 for version in versions.values()))

    def test_w3c_protocol_errors_keep_their_named_error_and_message(self) -> None:
        error = smoke._protocol_error(
            {
                "value": {
                    "error": "element click intercepted",
                    "message": "Another element would receive the click",
                }
            },
            400,
        )
        self.assertIsInstance(error, smoke.WebDriverProtocolError)
        assert error is not None
        self.assertEqual(error.status, 400)
        self.assertEqual(error.error, "element click intercepted")
        self.assertIn("Another element", error.message)

    def test_automation_profile_completes_current_tutorials_before_reload(self) -> None:
        with mock.patch.object(smoke, "request", return_value=True) as request:
            smoke.prepare_automation_profile("session-1")

        request.assert_called_once()
        method, path, payload = request.call_args.args
        self.assertEqual(method, "POST")
        self.assertEqual(path, "/session/session-1/execute/sync")
        self.assertIn("window.location.reload()", payload["script"])
        self.assertEqual(payload["args"][0], smoke.TUTORIAL_STORAGE_KEY)
        state = payload["args"][1]
        self.assertEqual(state["schemaVersion"], 1)
        self.assertEqual(state["completed"], smoke.tutorial_versions())
        self.assertEqual(state["dismissed"], {})

    def test_click_requires_two_stable_hit_tests_before_w3c_click(self) -> None:
        hit = {
            "ready": True,
            "rect": {"left": 10, "top": 20, "width": 120, "height": 44},
        }
        with (
            mock.patch.object(smoke, "_element_hit_test", side_effect=[hit, hit]) as hit_test,
            mock.patch.object(smoke, "request", return_value=None) as request,
            mock.patch.object(smoke.time, "sleep"),
        ):
            smoke.click("session-1", "element-1")

        self.assertEqual(hit_test.call_count, 2)
        request.assert_called_once_with(
            "POST",
            "/session/session-1/element/element-1/click",
            {},
        )

    def test_native_smoke_uses_structural_practice_completion_contract(self) -> None:
        harness = SMOKE_PATH.read_text(encoding="utf-8")
        completion = PRACTICE_COMPLETE_PATH.read_text(encoding="utf-8")
        self.assertIn('data-testid="practice-complete"', completion)
        self.assertIn("PRACTICE_COMPLETE_SELECTOR", harness)
        self.assertNotIn('"Session complete"', harness)

    def test_create_cards_exposes_stable_bulk_add_automation_hook(self) -> None:
        source = CREATE_CARDS_PATH.read_text(encoding="utf-8")
        self.assertIn('data-testid="bulk-add-cards"', source)

    def test_android_smoke_entry_point_installs_test_bridge_only_when_requested(self) -> None:
        source = MAIN_ENTRY_PATH.read_text(encoding="utf-8")
        self.assertIn('import.meta.env.VITE_OSTUDY_ANDROID_TEST_BRIDGE === "1"', source)
        self.assertIn('import("./testing/androidTestBridge")', source)
        self.assertIn("installAndroidTestBridge();", source)
        self.assertNotIn('import { installAndroidTestBridge }', source)

    def test_android_package_verifier_requires_bridge_for_flagged_debug_builds(self) -> None:
        source = ANDROID_PACKAGE_VERIFIER_PATH.read_text(encoding="utf-8")
        self.assertIn('VITE_OSTUDY_ANDROID_TEST_BRIDGE', source)
        self.assertIn('Android smoke APK is missing the emulator-only test bridge.', source)
        self.assertIn('Release APK contains the emulator-only Android test bridge.', source)

    def test_android_smoke_build_cannot_auto_start_contextual_tutorials(self) -> None:
        source = TOUR_PROVIDER_PATH.read_text(encoding="utf-8")
        self.assertIn("VITE_OSTUDY_ANDROID_TEST_BRIDGE", source)
        self.assertIn("CONTEXTUAL_AUTO_START_ENABLED", source)


if __name__ == "__main__":
    unittest.main()
