from __future__ import annotations

import ast
import copy
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SMOKE = ROOT / "tests/webdriver/smoke.py"
sys.path.insert(0, str(ROOT / "tests/webdriver"))
import ui_platform_smoke as ui


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
        self.assertFalse(any('"dialog ' in text or text.startswith("dialog ") for text in literals))

    def test_selection_and_open_target_actual_interactive_surface(self):
        source = SMOKE.read_text(encoding="utf-8")
        self.assertIn("//*[@role='checkbox' and @aria-label=", source)
        self.assertIn("//main//*[@role='button' and @aria-label=", source)
        self.assertNotIn("//input[@aria-label={json.dumps(f'Select", source)
        for invariant in ("set_input_text(", "set_rich_editor_text(",
                          "assert_internal_practice_shell(session_id, deck_name)",
                          "layout == {\"mode\": expected_mode, \"contained\": True}",
                          "verify_analytics_tag_search(session_id, question, tag_name, deadline)"):
            self.assertIn(invariant, source)

    def test_every_native_deck_creation_targets_semantic_dialogs(self):
        for filename in ("smoke.py", "ui_platform_smoke.py", "card_listing_followup_smoke.py",
                         "deck_toolbar_layout_smoke.py"):
            with self.subTest(filename=filename):
                source = (SMOKE.parent / filename).read_text(encoding="utf-8")
                self.assertIn("[role='dialog'] input[data-autofocus='true']", source)
                self.assertNotIn('"dialog input', source)
                self.assertNotIn('"dialog textarea', source)
                self.assertNotIn("HTMLDialogElement", source)


class Phase8ViewportTests(unittest.TestCase):
    def setUp(self):
        # Inset desktop main with a 1px border: route fills its usable client area.
        self.measurement = {
            "viewportWidth": 1280, "viewportHeight": 800,
            "shellPaddingTop": 12, "shellPaddingBottom": 12, "shellPaddingRight": 12,
            "shell": {"left": 0, "top": 0, "right": 1280, "bottom": 800},
            "main": {"left": 256, "top": 12, "right": 1268, "bottom": 788,
                     "width": 1012, "height": 776},
            "contentBox": {"left": 257, "top": 13, "width": 1010, "height": 774},
            "routeLayer": {"left": 257, "top": 13, "width": 1010, "height": 774},
            "route": {"width": 1010, "height": 774},
        }

    def check(self, measurement):
        with patch.object(ui, "wait_for_layout_stable"), patch.object(ui, "execute", return_value=measurement), \
                patch.object(ui, "assert_no_horizontal_overflow") as overflow:
            ui.assert_viewport_consumption("session", "inset desktop", "[aria-label='Study Guides']")
            overflow.assert_called_once_with("session", "inset desktop")

    def test_inset_main_fills_available_content_box(self):
        self.check(self.measurement)

    def test_main_cannot_leave_unexplained_right_gap(self):
        measurement = copy.deepcopy(self.measurement)
        measurement["main"]["right"] -= 40
        with self.assertRaisesRegex(AssertionError, "Main route layer"):
            self.check(measurement)

    def test_route_cannot_shrink_inside_correct_main(self):
        for axis in ("width", "height"):
            with self.subTest(axis=axis):
                measurement = copy.deepcopy(self.measurement)
                measurement["routeLayer"][axis] -= 40
                with self.assertRaisesRegex(AssertionError, "Main route layer"):
                    self.check(measurement)

    def test_maintained_route_cannot_leave_unused_vertical_space(self):
        measurement = copy.deepcopy(self.measurement)
        measurement["route"]["height"] -= 40
        with self.assertRaisesRegex(AssertionError, "Maintained route"):
            self.check(measurement)

    def test_modal_still_requires_full_backdrop_containment_and_scroll_owner(self):
        valid = {"viewportWidth": 390, "viewportHeight": 700,
                 "dialog": {"left": 0, "top": 0, "right": 390, "bottom": 700},
                 "card": {"left": 0, "top": 180, "right": 390, "bottom": 700},
                 "bodyOverflowY": "auto"}
        with patch.object(ui, "wait_for_layout_stable"), patch.object(ui, "assert_no_horizontal_overflow"):
            with patch.object(ui, "execute", return_value=valid):
                ui.assert_dialog_viewport_bounds("session", "sheet")
            for changed in ("dialog", "card", "scroll"):
                invalid = copy.deepcopy(valid)
                if changed == "scroll":
                    invalid["bodyOverflowY"] = "visible"
                else:
                    invalid[changed]["right"] += 40
                with self.subTest(changed=changed), patch.object(ui, "execute", return_value=invalid):
                    with self.assertRaisesRegex(AssertionError, "Dialog escaped"):
                        ui.assert_dialog_viewport_bounds("session", "sheet")


if __name__ == "__main__":
    unittest.main()
