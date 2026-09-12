"""Unit tests for accessible Flashcards tab selection in the native UI harness."""
from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests/webdriver"))
import ui_platform_os_chrome_smoke as current
import ui_platform_smoke as ui


class WorkspaceTabSelectorTests(unittest.TestCase):
    def test_targets_explicit_name_in_the_actual_workspace_tablist(self):
        for label in ("Decks", "Search/Filter"):
            with self.subTest(label=label), patch.object(current.webdriver, "find_element", return_value="tab") as find:
                self.assertEqual(current.find_workspace_tab("session", label, 123.0), "tab")
                find.assert_called_once_with(
                    "session", "xpath",
                    "//main//*[@role='tablist' and @aria-label='Flashcards workspaces']"
                    f"//button[@role='tab' and @aria-label={json.dumps(label)}]",
                    123.0,
                )

    def test_missing_accessible_name_remains_a_failure(self):
        with patch.object(current.webdriver, "find_element", side_effect=TimeoutError("missing tab name")):
            with self.assertRaisesRegex(TimeoutError, "missing tab name"):
                current.find_workspace_tab("session", "Search/Filter", 123.0)

    def test_exact_count_order_and_full_search_name_remain_required(self):
        with patch.object(ui, "find_workspace_tab", current.find_workspace_tab), \
                patch.object(current.webdriver, "find_element", return_value="tab"):
            with patch.object(ui, "execute", return_value=["Decks", "Search/Filter"]):
                ui.assert_flashcards_tabs("session", 123.0)
            for names in (["Decks", "Search"], ["Search/Filter", "Decks"], ["Decks"],
                          ["Decks", "Search/Filter", "Import/Export"]):
                with self.subTest(names=names), patch.object(ui, "execute", return_value=names):
                    with self.assertRaisesRegex(AssertionError, "exactly Decks and Search/Filter"):
                        ui.assert_flashcards_tabs("session", 123.0)

    def test_current_native_entrypoint_installs_the_selector(self):
        source = (ROOT / "tests/webdriver/ui_platform_os_chrome_smoke.py").read_text()
        self.assertIn('ui.find_workspace_tab = find_workspace_tab', source)
        self.assertLess(source.index('ui.find_workspace_tab = find_workspace_tab'),
                        source.index('raise SystemExit(ui.main())'))


if __name__ == "__main__":
    unittest.main()
