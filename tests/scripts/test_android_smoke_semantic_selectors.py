from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ANDROID_SMOKE = (
    ROOT
    / "src-tauri"
    / "gen"
    / "android"
    / "app"
    / "src"
    / "androidTest"
    / "java"
    / "io"
    / "github"
    / "jferments"
    / "ostudy"
    / "AndroidSmokeTest.kt"
)
QUERY_SELECTOR = re.compile(
    r"querySelector(?:All)?\(\s*(['\"])(.*?)\1",
    re.DOTALL,
)
CLASS_SELECTOR = re.compile(r"(?<![A-Za-z0-9_-])\.[A-Za-z_][A-Za-z0-9_-]*")


class AndroidSmokeSemanticSelectorTests(unittest.TestCase):
    def test_android_smoke_does_not_depend_on_presentation_classes(self) -> None:
        source = ANDROID_SMOKE.read_text(encoding="utf-8")
        failures: list[str] = []

        for match in QUERY_SELECTOR.finditer(source):
            selector = match.group(2)
            if not CLASS_SELECTOR.search(selector):
                continue
            line = source.count("\n", 0, match.start()) + 1
            failures.append(
                f"{ANDROID_SMOKE.relative_to(ROOT)}:{line} uses presentation-class DOM selector {selector!r}"
            )

        self.assertEqual([], failures, "\n" + "\n".join(failures))

    def test_nested_deck_back_stack_uses_accessible_open_control(self) -> None:
        source = ANDROID_SMOKE.read_text(encoding="utf-8")
        start = source.index("fun c_nestedDeckAndEditorBackReturnThroughTheStack()")
        end = source.index("\n    @Test\n    fun d_reloadsIntoInterruptedPracticeSession()", start)
        scenario = source[start:end]
        open_selector = (
            '[role=\\"button\\"][aria-label=\\"Open Android Emulator Smoke Deck\\"]'
        )

        self.assertNotIn("button.querySelector('strong')", scenario)
        self.assertNotIn(
            'button[aria-label=\\"Open Android Emulator Smoke Deck\\"]',
            scenario,
        )
        self.assertEqual(3, scenario.count(open_selector))
        self.assertIn(
            "document.querySelector('main "
            + open_selector
            + "')?.click(); true",
            scenario,
        )


if __name__ == "__main__":
    unittest.main()
