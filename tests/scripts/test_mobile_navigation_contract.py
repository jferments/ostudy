import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class MobileNavigationContractTests(unittest.TestCase):
    def test_compact_learning_destinations_and_settings_app_bar_are_disjoint(self) -> None:
        contract = json.loads((ROOT / "scripts/route-smoke-contract.json").read_text())
        compact = contract["compactNavigation"]
        self.assertEqual(
            compact["primary"],
            ["decks", "study-guides", "documents", "analytics"],
        )
        self.assertEqual(compact["more"], [])
        self.assertEqual(compact["appBar"], ["settings"])
        all_destinations = compact["primary"] + compact["more"] + compact["appBar"]
        self.assertEqual(len(all_destinations), len(set(all_destinations)))

        navigation = (ROOT / "src/components/NavigationSidebar.tsx").read_text()
        app_bar = (ROOT / "src/components/MobileAppBar.tsx").read_text()
        self.assertIn('"documents",\n  "analytics"', navigation)
        self.assertNotIn("nav-more", navigation)
        self.assertIn('data-testid="app-settings"', app_bar)
        self.assertIn('onNavigate("settings")', app_bar)

    def test_fresh_library_android_smoke_runs_before_library_seeding(self) -> None:
        runner = (ROOT / "scripts/android-emulator-smoke.sh").read_text()
        healthy_sequence = (
            "else\n"
            "  record_case a1_freshLibraryUsesQuietNavigation || true\n\n"
            "  seed_ok=1\n"
            "  record_case a_seedLibraryPracticeAndAnalytics || seed_ok=0"
        )
        self.assertIn(healthy_sequence, runner)

        launch_failure_block = runner.split("if ((launch_ok == 0)); then", 1)[1].split(
            "else", 1
        )[0]
        self.assertIn("a1_freshLibraryUsesQuietNavigation", launch_failure_block)


if __name__ == "__main__":
    unittest.main()
