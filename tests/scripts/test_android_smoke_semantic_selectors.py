from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ANDROID_SMOKE = ROOT / "src-tauri/gen/android/app/src/androidTest/java/io/github/jferments/ostudy/AndroidSmokeTest.kt"
QUERY_SELECTOR = re.compile(r"querySelector(?:All)?\(\s*(['\"])(.*?)\1", re.DOTALL)
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
            failures.append(f"{ANDROID_SMOKE.relative_to(ROOT)}:{line} uses presentation-class DOM selector {selector!r}")
        self.assertEqual([], failures, "\n" + "\n".join(failures))

    def test_nested_deck_back_stack_steps_through_the_workbench(self) -> None:
        source = ANDROID_SMOKE.read_text(encoding="utf-8")
        start = source.index("fun c_nestedDeckAndEditorBackReturnThroughTheStack()")
        end = source.index("\n    @Test\n    fun d_reloadsIntoInterruptedPracticeSession()", start)
        scenario = source[start:end]
        # The deck opens from the Workbench's tree by its name, not a retired deck row.
        self.assertNotIn("Open Android Emulator Smoke Deck", scenario)
        # A deck row can lead with its subject's badge, so any of its spans may carry the name.
        self.assertIn("Array.from(button.querySelectorAll('span')).some(span => span.textContent?.trim() === 'Android Emulator Smoke Deck')", scenario)
        # Back closes the editor, then the card list, then leaves Library for Home.
        self.assertEqual(3, scenario.count("device.pressBack()"))
        self.assertLess(scenario.index("=== 'Add card'"), scenario.index("device.pressBack()"))
        self.assertIn("!document.querySelector('main h1[data-testid=\\\"workbench-title\\\"]')", scenario)

    def test_empty_library_snapshot_waits_for_the_workbench_tree(self) -> None:
        source = ANDROID_SMOKE.read_text(encoding="utf-8")
        start = source.index("fun a1_freshLibraryUsesQuietNavigation()")
        end = source.index("fun a_seedLibraryPracticeAndAnalytics()", start)
        scenario = source[start:end]
        ready = scenario.index('library-workbench\\"] nav[aria-label=\\"Library\\"]')
        self.assertLess(scenario.index("waitUntil("), ready)
        self.assertLess(ready, scenario.index("val snapshot"))
        # Library opens on its subject tiles; All cards there leads to the Workbench.
        self.assertLess(scenario.index('library-all-cards'), ready)
        for key in ("libraryCurrent", "settingsGear", "importAction"):
            self.assertIn(f'assertTrue(snapshot.getBoolean("{key}"))', scenario)
        for key in ("cardList", "irrelevantControls", "subjectPages"):
            self.assertIn(f'assertFalse(snapshot.getBoolean("{key}"))', scenario)
        self.assertIn('assertEquals(1, snapshot.getInt("createActions"))', scenario)


if __name__ == "__main__":
    unittest.main()
