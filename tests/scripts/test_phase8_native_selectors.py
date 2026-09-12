from __future__ import annotations

import ast
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SMOKE = ROOT / "tests/webdriver/smoke.py"


class Phase8NativeSelectorTests(unittest.TestCase):
    def test_durable_flow_uses_dialog_roles_instead_of_retired_element_tag(self):
        tree = ast.parse(SMOKE.read_text(encoding="utf-8"))
        flow = next(node for node in tree.body if isinstance(node, ast.FunctionDef)
                    and node.name == "create_and_review_card")
        literals = [node.value for node in ast.walk(flow)
                    if isinstance(node, ast.Constant) and isinstance(node.value, str)]
        self.assertIn("[role='dialog'] input[data-autofocus='true']", literals)
        self.assertIn("[role='dialog'] button[aria-label='Close dialog']", literals)
        self.assertTrue(any("[role='dialog'] [data-layout-mode]" in text for text in literals))
        self.assertFalse(any('\"dialog ' in text or text.startswith("dialog ") for text in literals))

    def test_selection_and_open_target_actual_interactive_surface(self):
        source = SMOKE.read_text(encoding="utf-8")
        self.assertIn("//*[@role='checkbox' and @aria-label=", source)
        self.assertIn("//main//*[@role='button' and @aria-label=", source)
        self.assertNotIn("//input[@aria-label={json.dumps(f'Select", source)
        # Persistence, review submission and responsive-layout assertions remain.
        for invariant in ("set_input_text(", "set_rich_editor_text(",
                          "assert_internal_practice_shell(session_id, deck_name)",
                          "layout == {\"mode\": expected_mode, \"contained\": True}",
                          "verify_analytics_tag_search(session_id, question, tag_name, deadline)"):
            self.assertIn(invariant, source)


if __name__ == "__main__":
    unittest.main()
