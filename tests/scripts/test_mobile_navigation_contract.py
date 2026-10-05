import json
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


class MobileNavigationContractTests(unittest.TestCase):
    def test_phone_navigation_names_five_destinations_beside_header_search_and_settings(self) -> None:
        contract = json.loads((ROOT / "scripts/route-smoke-contract.json").read_text())
        navigation = contract["navigation"]
        destinations = [entry["id"] for entry in navigation["destinations"]]
        actions = [entry["id"] for entry in navigation["headerActions"]]
        self.assertEqual(destinations, ["home", "library", "review", "tutor", "progress"])
        self.assertEqual(actions, ["search", "settings"])
        self.assertFalse(set(destinations) & set(actions))
        main_navigation = (ROOT / "src/components/MainNavigation.tsx").read_text()
        header = (ROOT / "src/components/AppHeaderBar.tsx").read_text()
        # One list serves every screen size; a phone gets no overflow "More" item.
        self.assertNotIn('"more"', main_navigation)
        self.assertNotIn("COMPACT_", main_navigation)
        self.assertIn('data-testid="app-search"', header)
        self.assertIn('data-testid="app-settings"', header)
        app = (ROOT / "src/App.tsx").read_text()
        self.assertIn('onSettings={() => navigate("settings")}', app)
        self.assertIn("onSearch={() => void controller.openSearch()}", app)

    def test_fresh_library_android_smoke_runs_before_library_seeding(self) -> None:
        runner = (ROOT / "scripts/android-emulator-smoke.sh").read_text()
        healthy_sequence = (
            "else\n"
            "  record_case a1_freshLibraryUsesQuietNavigation || true\n\n"
            "  seed_ok=1\n"
            "  record_case a_seedLibraryPracticeAndAnalytics || seed_ok=0"
        )
        self.assertIn(healthy_sequence, runner)
        launch_failure_block = runner.split("if ((launch_ok == 0)); then", 1)[1].split("else", 1)[0]
        self.assertIn("a1_freshLibraryUsesQuietNavigation", launch_failure_block)

    def test_compact_study_guide_search_resets_desktop_flex_bases(self) -> None:
        source = (ROOT / "src/screens/documents/presentation.ts").read_text()
        match = re.search(r"export const guideLibraryStyles = (\{.*?\}) as const;", source, re.S)
        self.assertIsNotNone(match)
        recipe = json.loads(match.group(1))["search"]
        for utility in (
            "tw:min-w-0", "tw:flex-wrap", "tw:max-sm:w-full", "tw:max-sm:flex-[0_1_auto]",
            "tw:max-sm:[&>label]:w-full", "tw:max-sm:[&>label]:max-w-none",
            "tw:max-sm:[&>label]:flex-[0_1_auto]",
        ):
            self.assertIn(utility, recipe.split())

    def test_compact_deck_toolbar_keeps_actions_intrinsic_and_import_labeled(self) -> None:
        recipes = (ROOT / "src/screens/flashcards/deck-library/libraryPresentation.ts").read_text()
        root = (ROOT / "src/screens/flashcards/deck-library/DeckLibraryRoot.tsx").read_text()
        toolbar = recipes.split('toolbarActions: "', 1)[1].split('"', 1)[0]
        for utility in ("tw:w-auto", "tw:justify-end", "tw:[&>button]:flex-none", "tw:[&>button]:whitespace-nowrap"):
            self.assertIn(utility, toolbar.split())
        self.assertNotIn("importAction:", recipes)
        self.assertIn("Import deck", root)
        self.assertNotIn('label="Import deck from file"', root)


if __name__ == "__main__":
    unittest.main()
