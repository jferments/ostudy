import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class WindowControlsContractTest(unittest.TestCase):
    def read(self, relative_path: str) -> str:
        return (ROOT / relative_path).read_text(encoding="utf-8")

    def test_main_window_has_only_the_required_mutation_permissions(self) -> None:
        capability = json.loads(self.read("src-tauri/capabilities/main.json"))
        permissions = set(capability["permissions"])

        self.assertEqual(capability["windows"], ["main"])
        self.assertTrue(
            {
                "core:window:allow-maximize",
                "core:window:allow-unmaximize",
                "core:window:allow-set-fullscreen",
            }.issubset(permissions)
        )
        self.assertNotIn("core:window:default", permissions)

    def test_native_capability_snapshot_is_coherent_across_targets(self) -> None:
        contract = self.read("src-tauri/src/platform/mod.rs")
        desktop = self.read("src-tauri/src/platform/desktop/capabilities.rs")
        android = self.read("src-tauri/src/platform/android.rs")
        mobile = self.read("src-tauri/src/platform/mobile_stub.rs")

        for field in ("window_maximize_restore", "window_full_screen"):
            self.assertIn(f"pub {field}: ServiceCapability", contract)
            self.assertIn(f"{field}: ServiceCapability::available()", desktop)
            self.assertIn(f"{field}: ServiceCapability::unavailable(", android)
            self.assertIn(f"{field}: ServiceCapability::unavailable(", mobile)
        for implementation in (desktop, android, mobile):
            self.assertIn("schema_version: 2", implementation)

    def test_tauri_window_access_stays_inside_the_platform_adapter(self) -> None:
        adapter = self.read("src/platform/windowControls.ts")
        controller = self.read("src/controllers/useWindowControlsController.ts")
        component = self.read("src/components/WindowControls.tsx")

        self.assertIn('from "@tauri-apps/api/window"', adapter)
        for native_call in (
            ".isMaximized()",
            ".isFullscreen()",
            ".maximize()",
            ".unmaximize()",
            ".setFullscreen(enabled)",
            ".onResized(",
        ):
            self.assertIn(native_call, adapter)
        self.assertNotIn("document.fullscreen", adapter)
        self.assertNotIn("requestFullscreen", adapter)
        self.assertNotIn("@tauri-apps", controller)
        self.assertNotIn("@tauri-apps", component)

    def test_native_smoke_measures_shell_routes_and_dialogs_in_window_states(self) -> None:
        smoke = self.read("tests/webdriver/ui_platform_smoke.py")

        for contract in (
            "assert_viewport_consumption",
            "assert_dialog_viewport_bounds",
            '"maximized desktop viewport"',
            '"full-screen desktop viewport"',
            '"[aria-label=\'Study Guides\']"',
            '"responsive create-deck dialog"',
        ):
            self.assertIn(contract, smoke)


if __name__ == "__main__":
    unittest.main()
