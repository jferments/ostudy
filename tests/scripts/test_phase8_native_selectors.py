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

    def test_workbench_flow_targets_tree_entries_and_card_rows(self):
        source = SMOKE.read_text(encoding="utf-8")
        self.assertIn("{WORKBENCH_TREE_XPATH}//button[.//span[normalize-space(.)=", source)
        self.assertIn("//main//*[@data-card-id]//button[contains(normalize-space(.), ", source)
        self.assertIn("choose_menu_item(session_id, f\"{deck_name} actions\", \"Practice with options…\", deadline)", source)
        for invariant in ("set_input_text(", "set_rich_editor_text(",
                          "assert_internal_practice_shell(session_id, deck_name)",
                          "layout == {\"mode\": expected_mode, \"contained\": True}",
                          "verify_analytics_tag_search(session_id, question, tag_name, deadline)"):
            self.assertIn(invariant, source)

    def test_every_native_deck_creation_targets_semantic_dialogs(self):
        for filename in ("smoke.py", "ui_platform_smoke.py", "card_listing_followup_smoke.py"):
            with self.subTest(filename=filename):
                source = (SMOKE.parent / filename).read_text(encoding="utf-8")
                self.assertIn("[role='dialog'] input[data-autofocus='true']", source)
                self.assertNotIn('"dialog input', source)
                self.assertNotIn('"dialog textarea', source)
                self.assertNotIn("HTMLDialogElement", source)


class Phase8ViewportTests(unittest.TestCase):
    def setUp(self):
        # Desktop main beside the in-flow side rail, with a 1px border: the route fills its
        # usable client area.
        self.measurement = {
            "viewportWidth": 1280, "viewportHeight": 800,
            "shellPaddingTop": 0, "shellPaddingBottom": 0, "shellPaddingRight": 0,
            "shellPaddingLeft": 0, "headerRight": 88,
            "shell": {"left": 0, "top": 0, "right": 1280, "bottom": 800},
            "main": {"left": 88, "top": 0, "right": 1280, "bottom": 800,
                     "width": 1192, "height": 800},
            "contentBox": {"left": 89, "top": 1, "width": 1190, "height": 798},
            "routeLayer": {"left": 89, "top": 1, "width": 1190, "height": 798},
            "route": {"width": 1190, "height": 798},
        }

    def check(self, measurement):
        with patch.object(ui, "wait_for_layout_stable"), patch.object(ui, "execute", return_value=measurement), \
                patch.object(ui, "assert_no_horizontal_overflow") as overflow:
            ui.assert_viewport_consumption("session", "desktop", "[data-testid='library']")
            overflow.assert_called_once_with("session", "desktop")

    def test_main_fills_the_space_beside_the_side_rail(self):
        self.check(self.measurement)

    def test_main_cannot_slide_under_the_side_rail(self):
        measurement = copy.deepcopy(self.measurement)
        measurement["main"]["left"] = 0
        with self.assertRaisesRegex(AssertionError, "Main route layer"):
            self.check(measurement)

    def test_phone_main_sits_between_the_fixed_bars(self):
        # A phone's header and bottom navigation are fixed; the shell pads main between them.
        measurement = copy.deepcopy(self.measurement)
        measurement.update({
            "viewportWidth": 390, "viewportHeight": 700,
            "shellPaddingTop": 56, "shellPaddingBottom": 73, "headerRight": None,
            "shell": {"left": 0, "top": 0, "right": 390, "bottom": 700},
            "main": {"left": 0, "top": 56, "right": 390, "bottom": 627,
                     "width": 390, "height": 571},
            "contentBox": {"left": 0, "top": 56, "width": 390, "height": 571},
            "routeLayer": {"left": 0, "top": 56, "width": 390, "height": 571},
            "route": {"width": 390, "height": 571},
        })
        self.check(measurement)
        measurement["main"]["left"] = 72
        with self.assertRaisesRegex(AssertionError, "Main route layer"):
            self.check(measurement)

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
