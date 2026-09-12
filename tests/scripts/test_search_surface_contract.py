"""The native search check measures the visual result, not the CSS paint owner."""
from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tests/webdriver"))
import card_listing_followup_smoke as listing


class SearchSurfaceContractTests(unittest.TestCase):
    def setUp(self):
        self.input_owned = {
            "iconInsideSurface": True, "iconVisible": True,
            "inputBackground": "rgb(26, 32, 42)", "surfaceBackground": "rgba(0, 0, 0, 0)",
            "inputRect": {"left": 240, "top": 100, "right": 850, "bottom": 144},
            "surfaceRect": {"left": 240, "top": 100, "right": 850, "bottom": 144},
            "iconRect": {"left": 252, "top": 113, "right": 270, "bottom": 131},
        }

    def check(self, measurement):
        # Exercise the native entry point, including its measurement validation.
        with patch.object(listing.ui, "execute", return_value=measurement):
            listing.assert_search_surface("session")

    def test_shadcn_input_may_own_the_single_background(self):
        self.check(self.input_owned)

    def test_wrapper_may_own_the_single_background(self):
        measurement = copy.deepcopy(self.input_owned)
        measurement["inputBackground"] = "transparent"
        measurement["surfaceBackground"] = "rgb(26, 32, 42)"
        self.check(measurement)

    def test_two_painted_backgrounds_fail_even_when_colors_match(self):
        measurement = copy.deepcopy(self.input_owned)
        measurement["surfaceBackground"] = measurement["inputBackground"]
        with self.assertRaisesRegex(AssertionError, "exactly one background"):
            self.check(measurement)

    def test_no_painted_background_fails(self):
        measurement = copy.deepcopy(self.input_owned)
        measurement["inputBackground"] = "transparent"
        with self.assertRaisesRegex(AssertionError, "exactly one background"):
            self.check(measurement)

    def test_icon_must_fit_inside_the_actual_painted_input(self):
        for edge in ("left", "top", "right", "bottom"):
            measurement = copy.deepcopy(self.input_owned)
            measurement["iconRect"][edge] = measurement["inputRect"][edge] + (-8 if edge in ("left", "top") else 8)
            with self.subTest(edge=edge), self.assertRaisesRegex(AssertionError, "painted surface"):
                self.check(measurement)

    def test_input_cannot_escape_its_wrapper(self):
        measurement = copy.deepcopy(self.input_owned)
        measurement["inputRect"]["right"] += 12
        with self.assertRaisesRegex(AssertionError, "escapes its wrapper"):
            self.check(measurement)

    def test_missing_hidden_or_zero_area_icon_fails(self):
        for field in ("iconInsideSurface", "iconVisible", "iconRect"):
            measurement = copy.deepcopy(self.input_owned)
            if field == "iconRect":
                measurement[field]["right"] = measurement[field]["left"]
            else:
                measurement[field] = False
            with self.subTest(field=field), self.assertRaises(AssertionError):
                self.check(measurement)

    def test_missing_measurements_fail_instead_of_passing(self):
        for measurement in (None, {}, {**self.input_owned, "inputBackground": None},
                            {**self.input_owned, "inputRect": None}):
            with self.subTest(measurement=measurement), self.assertRaises(AssertionError):
                self.check(measurement)


if __name__ == "__main__":
    unittest.main()
