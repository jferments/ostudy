from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MAIN_ENTRY_PATH = ROOT / "src" / "main.tsx"


class AndroidSmokeStartupContractTests(unittest.TestCase):
    def test_test_bridge_is_published_only_after_the_application_ui_mounts(self) -> None:
        source = MAIN_ENTRY_PATH.read_text(encoding="utf-8")

        self.assertIn('document.querySelector("main")', source)
        self.assertIn("async function installAndroidTestBridgeWhenUiReady(): Promise<void>", source)
        self.assertIn("await waitForAndroidTestUiReady();", source)

        bridge_import = source.index('await import("./testing/androidTestBridge")')
        readiness_wait = source.index("await waitForAndroidTestUiReady();")
        self.assertLess(readiness_wait, bridge_import)

        render = source.index("createRoot(root).render(")
        bridge_start = source.rindex("void installAndroidTestBridgeWhenUiReady()")
        self.assertLess(render, bridge_start)

    def test_bridge_readiness_has_a_bounded_failure_path(self) -> None:
        source = MAIN_ENTRY_PATH.read_text(encoding="utf-8")

        self.assertIn("30_000", source)
        self.assertIn("observer.disconnect();", source)
        self.assertIn('setAndroidTestBridgeDiagnostic("error", error)', source)


if __name__ == "__main__":
    unittest.main()
