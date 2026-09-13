"""Unit tests for shared accessible tab selection in every native UI consumer."""
from __future__ import annotations

import ast
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests/webdriver"))
import card_listing_followup_smoke as listing
import ui_platform_os_chrome_smoke as current
import ui_platform_smoke as ui


class WorkspaceTabSelectorTests(unittest.TestCase):
    def test_shared_helper_targets_explicit_name_in_the_workspace_tablist(self):
        for label in ("Decks", "Search/Filter"):
            with self.subTest(label=label), patch.object(ui.webdriver, "find_element", return_value="tab") as find:
                self.assertEqual(ui.find_workspace_tab("session", label, 123.0), "tab")
                find.assert_called_once_with(
                    "session", "xpath",
                    "//main//*[@role='tablist' and @aria-label='Flashcards workspaces']"
                    f"//button[@role='tab' and @aria-label={json.dumps(label)}]",
                    123.0,
                )

    def test_missing_accessible_name_remains_a_failure(self):
        with patch.object(ui.webdriver, "find_element", side_effect=TimeoutError("missing tab name")):
            with self.assertRaisesRegex(TimeoutError, "missing tab name"):
                ui.find_workspace_tab("session", "Search/Filter", 123.0)

    def test_exact_count_order_and_full_search_name_remain_required(self):
        with patch.object(ui.webdriver, "find_element", return_value="tab"):
            with patch.object(ui, "execute", return_value=["Decks", "Search/Filter"]):
                ui.assert_flashcards_tabs("session", 123.0)
            for names in (["Decks", "Search"], ["Search/Filter", "Decks"], ["Decks"],
                          ["Decks", "Search/Filter", "Import/Export"]):
                with self.subTest(names=names), patch.object(ui, "execute", return_value=names):
                    with self.assertRaisesRegex(AssertionError, "exactly Decks and Search/Filter"):
                        ui.assert_flashcards_tabs("session", 123.0)

    def test_card_listing_and_platform_runner_share_the_helper(self):
        self.assertIs(listing.ui, ui)
        self.assertIs(current.ui, ui)
        self.assertIs(listing.ui.find_workspace_tab, ui.find_workspace_tab)
        self.assertIs(current.ui.find_workspace_tab, ui.find_workspace_tab)

    def test_real_card_listing_path_calls_shared_helper_before_clicking(self):
        # Stop at the next native action after capturing the selector actually used.
        with patch.object(ui.webdriver, "find_element", return_value="search-tab") as find, \
                patch.object(listing.webdriver, "click", side_effect=RuntimeError("native click boundary")) as click:
            with self.assertRaisesRegex(RuntimeError, "native click boundary"):
                listing.assert_search_browser("session", "Question", "Answer", {}, 123.0)
            find.assert_called_once_with(
                "session", "xpath",
                "//main//*[@role='tablist' and @aria-label='Flashcards workspaces']"
                '//button[@role=\'tab\' and @aria-label="Search/Filter"]',
                123.0,
            )
            click.assert_called_once_with("session", "search-tab")

    def test_platform_entrypoint_does_not_replace_the_shared_selector(self):
        source = (ROOT / "tests/webdriver/ui_platform_os_chrome_smoke.py").read_text()
        self.assertNotIn("ui.find_workspace_tab =", source)
        self.assertFalse(any(isinstance(node, ast.FunctionDef) and node.name == "find_workspace_tab"
                             for node in ast.walk(ast.parse(source))))


if __name__ == "__main__":
    unittest.main()
