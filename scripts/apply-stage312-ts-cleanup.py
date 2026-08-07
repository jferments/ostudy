#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def update(path: str, transform) -> None:
    target = ROOT / path
    text = target.read_text(encoding="utf-8")
    next_text = transform(text)
    if next_text == text:
        raise SystemExit(f"{path}: expected cleanup change was not applied")
    target.write_text(next_text, encoding="utf-8")


update(
    "src/ipc/domain-contracts.test.ts",
    lambda text: text.replace("  listCardTemplateSummaries,\n", "", 1),
)


def card_editor_test(text: str) -> str:
    old = '''const card: cardsIpc.CardNote = {
  id: "b".repeat(32),
  noteId: "c".repeat(32),
  deckId: deck.id,
  direction: "forward",
  frontDoc: {
    type: "doc",
    content: [
      { type: "paragraph", content: [{ type: "text", text: "Question" }] },
    ],
  },
  backDoc: {
    type: "doc",
    content: [
      { type: "paragraph", content: [{ type: "text", text: "Answer" }] },
    ],
  },
  frontText: "Question",
  backText: "Answer",
  assets: [],
  tags: ["Medication"],
  dueAt: Date.now(),
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};'''
    new = '''const card: cardsIpc.CardNote = {
  cardType: "basic",
  noteId: "c".repeat(32),
  deckId: deck.id,
  frontDoc: {
    type: "doc",
    content: [
      { type: "paragraph", content: [{ type: "text", text: "Question" }] },
    ],
  },
  backDoc: {
    type: "doc",
    content: [
      { type: "paragraph", content: [{ type: "text", text: "Answer" }] },
    ],
  },
  frontText: "Question",
  backText: "Answer",
  reverseEnabled: false,
  assets: [],
  tags: ["Medication"],
  generatedCardCount: 1,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};'''
    if text.count(old) != 1:
        raise SystemExit("CardEditorScreen.test.tsx: CardNote fixture drifted")
    text = text.replace(old, new, 1)
    start = text.find("    vi.mocked(ipc.getCardTypeInfo).mockResolvedValue({")
    if start >= 0:
        end = text.find("    });\n", start)
        if end < 0:
            raise SystemExit("CardEditorScreen.test.tsx: type-info mock ending missing")
        text = text[:start] + text[end + len("    });\n"):]
    text = text.replace("      expect(ipc.getCardTypeInfo).toHaveBeenCalledWith(card.noteId);\n", "")
    if "ipc." in text:
        raise SystemExit("CardEditorScreen.test.tsx: stale aggregate ipc namespace remains")
    return text

update("src/screens/CardEditorScreen.test.tsx", card_editor_test)


def cards_screen(text: str) -> str:
    old = '''  function beginMoveCopy(cards: TransferableCard[]): void {
    const notes = uniqueNotes(cards);
    if (notes.length === 0) return;'''
    new = '''  function beginMoveCopy(cards: TransferableCard[]): void {
    const notes = [...new Map(cards.map((card) => [card.noteId, card])).values()];
    if (notes.length === 0) return;'''
    if text.count(old) != 1:
        raise SystemExit("CardsScreen.tsx: beginMoveCopy drifted")
    return text.replace(old, new, 1)

update("src/screens/CardsScreen.tsx", cards_screen)


def decks_screen(text: str) -> str:
    old = "      const deck = committedDeck ?? (await getDeck(deckId));\n      setSelectedDeck(deck);"
    new = "      const deck = committedDeck ?? (await controller.deck(deckId));\n      if (!deck) return;\n      setSelectedDeck(deck);"
    if text.count(old) != 1:
        raise SystemExit("DecksScreen.tsx: imported-deck refresh drifted")
    return text.replace(old, new, 1)

update("src/screens/DecksScreen.tsx", decks_screen)


def practice_test(text: str) -> str:
    old = '''const card: cardsIpc.CardPresentation = {
  id: "b".repeat(32),
  noteId: "c".repeat(32),
  deckId: deck.id,
  direction: "forward",
  frontDoc: { type: "doc", content: [] },
  backDoc: { type: "doc", content: [] },
  frontText: "Do dogs have fur?",
  backText: "Yes",
  assets: [],
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};
const practiceCard: practiceIpc.PracticeCard = {
  card,
  deckName: deck.name,'''
    new = '''const card: cardsIpc.CardPresentation = {
  cardId: "b".repeat(32),
  noteId: "c".repeat(32),
  deckId: deck.id,
  direction: "forward",
  frontDoc: { type: "doc", content: [] },
  backDoc: { type: "doc", content: [] },
  frontText: "Do dogs have fur?",
  backText: "Yes",
  assets: [],
  createdAt: 1,
  updatedAt: 1,
  noteRevision: 1,
};
const practiceCard: practiceIpc.PracticeCard = {
  presentation: card,
  deckName: deck.name,'''
    if text.count(old) != 1:
        raise SystemExit("PracticeScreen.test.tsx: presentation fixture drifted")
    text = text.replace(old, new, 1)
    text = text.replace("card.id", "card.cardId")
    return text

update("src/screens/PracticeScreen.test.tsx", practice_test)


def practice_screen(text: str) -> str:
    old = "import {\n  getRecentPracticeReviews,\n  type RecentReview,\n} from \"../ipc/analytics\";"
    if old in text:
        return text.replace(old, 'import { type RecentReview } from "../ipc/analytics";', 1)
    old_line = "  getRecentPracticeReviews,\n"
    if old_line in text:
        return text.replace(old_line, "", 1)
    raise SystemExit("PracticeScreen.tsx: stale recent-review import not found")

update("src/screens/PracticeScreen.tsx", practice_screen)

print("Applied Stage 3.12 TypeScript cleanup.")
