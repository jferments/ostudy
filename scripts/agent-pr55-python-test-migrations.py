#!/usr/bin/env python3
from __future__ import annotations

import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]


def read(path: str) -> str:
    return (ROOT / path).read_text()


def write(path: str, content: str) -> None:
    (ROOT / path).write_text(content)


def replace_once(path: str, old: str, new: str) -> None:
    text = read(path)
    if old not in text:
        if new in text:
            return
        raise RuntimeError(f"{path}: missing migration anchor {old[:120]!r}")
    if text.count(old) != 1:
        raise RuntimeError(f"{path}: ambiguous migration anchor {old[:120]!r}")
    write(path, text.replace(old, new, 1))


# Create Cards no longer exists as a workspace. The stable automation boundary is
# now the current deck's card browser, which exposes Bulk add and fixes the modal
# destination to that deck.
path = "tests/scripts/test_webdriver_smoke.py"
text = read(path)
text = text.replace(
    'CREATE_CARDS_PATH = ROOT / "src" / "screens" / "flashcards" / "CreateCardsWorkspace.tsx"\n',
    'DECKS_SCREEN_PATH = ROOT / "src" / "screens" / "DecksScreen.tsx"\n'
    'BULK_ENTRY_PATH = ROOT / "src" / "screens" / "cards" / "BulkEntryModal.tsx"\n',
)
text = text.replace(
    '''    def test_create_cards_exposes_stable_bulk_add_automation_hook(self) -> None:\n        source = CREATE_CARDS_PATH.read_text(encoding="utf-8")\n        self.assertIn('data-testid="bulk-add-cards"', source)\n''',
    '''    def test_deck_browser_exposes_stable_bulk_add_automation_hook(self) -> None:\n        decks = DECKS_SCREEN_PATH.read_text(encoding="utf-8")\n        bulk = BULK_ENTRY_PATH.read_text(encoding="utf-8")\n        self.assertIn('data-testid="bulk-add-cards"', decks)\n        self.assertIn('destinationMode="fixed"', decks)\n        self.assertIn('destinationMode?: "fixed" | "selectable"', bulk)\n''',
)
text = text.replace(
    '''    def test_android_smoke_build_cannot_auto_start_contextual_tutorials(self) -> None:\n        source = TOUR_PROVIDER_PATH.read_text(encoding="utf-8")\n        self.assertIn("VITE_OSTUDY_ANDROID_TEST_BRIDGE", source)\n        self.assertIn("CONTEXTUAL_AUTO_START_ENABLED", source)\n''',
    '''    def test_tutorials_require_explicit_start_on_every_platform(self) -> None:\n        source = TOUR_PROVIDER_PATH.read_text(encoding="utf-8")\n        self.assertIn('ostudy:start-tutorial', source)\n        self.assertNotIn("CONTEXTUAL_AUTO_START_ENABLED", source)\n        self.assertNotIn("findContextualModule", source)\n        self.assertNotIn("autoStartTimer", source)\n''',
)
write(path, text)

# The full qualification process reports its baseline once and exports the flag.
# A nested unit test must not inherit that presentation-only state or the function
# under test will deliberately remain silent.
replace_once(
    "tests/scripts/test_check_state.py",
    '''        environment = os.environ.copy()\n        environment["OSTUDY_ROOT_DIR"] = str(repo)\n''',
    '''        environment = os.environ.copy()\n        environment["OSTUDY_ROOT_DIR"] = str(repo)\n        environment.pop("OSTUDY_WORKTREE_BASELINE_REPORTED", None)\n''',
)
