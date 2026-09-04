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


if __name__ == "__main__":
    unittest.main()
