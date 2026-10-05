import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class WindowControlsContractTest(unittest.TestCase):
    def read(self, relative_path: str) -> str:
        return (ROOT / relative_path).read_text(encoding="utf-8")

    def test_webview_has_no_window_state_mutation_permissions(self) -> None:
        capability = json.loads(self.read("src-tauri/capabilities/main.json"))
        permissions = set(capability["permissions"])

        self.assertEqual(capability["windows"], ["main"])
        for permission in (
            "core:window:allow-maximize",
            "core:window:allow-unmaximize",
            "core:window:allow-set-fullscreen",
            "core:window:default",
        ):
            self.assertNotIn(permission, permissions)

    def test_shell_relies_on_operating_system_window_chrome(self) -> None:
        app = self.read("src/App.tsx")
        shell_sources = [
            self.read(path)
            for path in (
                "src/components/AppHeaderBar.tsx",
                "src/components/MainNavigation.tsx",
                "src/ui/layout/AppFrame.tsx",
                "src/ui/layout/AppNavigation.tsx",
            )
        ]

        for source in (app, *shell_sources):
            self.assertNotIn("WindowControls", source)
            self.assertNotIn("windowControls", source)
            self.assertNotIn("useWindowControlsController", source)
        self.assertNotIn('from "@tauri-apps/api/window"', app)

    def test_native_capability_snapshot_remains_truthful_across_targets(self) -> None:
        contract = self.read("src-tauri/src/platform/mod.rs")
        desktop = self.read("src-tauri/src/platform/desktop/capabilities.rs")
        android = self.read("src-tauri/src/platform/android.rs")
        mobile = self.read("src-tauri/src/platform/mobile_stub.rs")

        # Platform capability reporting remains available for diagnostics and
        # native smoke; it no longer authorizes app-rendered mutation controls.
        for field in ("window_maximize_restore", "window_full_screen"):
            self.assertIn(f"pub {field}: ServiceCapability", contract)
            self.assertIn(f"{field}: ServiceCapability::available()", desktop)
            self.assertIn(f"{field}: ServiceCapability::unavailable(", android)
            self.assertIn(f"{field}: ServiceCapability::unavailable(", mobile)
        for implementation in (desktop, android, mobile):
            self.assertIn("schema_version: 2", implementation)

    def test_native_smoke_measures_shell_routes_and_dialogs_in_window_states(self) -> None:
        smoke = self.read("tests/webdriver/ui_platform_smoke.py")

        for contract in (
            "assert_viewport_consumption",
            "assert_dialog_viewport_bounds",
            "route_selector = LIBRARY_ROUTE_SELECTOR",
            '"resized desktop viewport"',
            '"maximized desktop viewport"',
            '"restored desktop viewport after maximize"',
            '"full-screen desktop viewport"',
            '"desktop viewport after exiting full screen"',
            'LIBRARY_ROUTE_SELECTOR = "[data-testid=\'library\']"',
            '"responsive create-deck dialog"',
        ):
            self.assertIn(contract, smoke)


if __name__ == "__main__":
    unittest.main()
