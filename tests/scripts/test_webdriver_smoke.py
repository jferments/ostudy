from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
SMOKE_PATH = ROOT / "tests" / "webdriver" / "smoke.py"
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


if __name__ == "__main__":
    unittest.main()
