import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { DecksScreen } from "./DecksScreen";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("../ipc", () => ({
  createDeck: vi.fn(),
  deleteCard: vi.fn(),
  deleteDeck: vi.fn(),
  exportDeck: vi.fn(),
  getDeck: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  importDeck: vi.fn(),
  listCards: vi.fn(),
  updateDeck: vi.fn(),
}));

vi.mock("./CardEditorScreen", () => ({
  CardEditorScreen: () => <div>Card editor</div>,
}));

const deck: ipc.Deck = {
  id: "a".repeat(32),
  name: "Dog facts",
  description: "Dogs are cool animals",
  cardCount: 2,
  dueCount: 0,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

const card: ipc.Flashcard = {
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

const settings: ipc.AppSettings = {
  theme: "synthwave",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  reduceMotion: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

describe("DecksScreen", () => {
  beforeEach(() => {
    vi.mocked(ipc.getDeck).mockResolvedValue(deck);
    vi.mocked(ipc.listCards).mockResolvedValue([card]);
  });

  it("keeps Practice enabled for a non-empty deck with no cards due", async () => {
    const onPracticeDeck = vi.fn();
    render(
      <DecksScreen
        decks={[deck]}
        settings={settings}
        refreshDecks={async () => [deck]}
        onPracticeDeck={onPracticeDeck}
        onDirtyChange={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    const libraryPractice = screen.getByRole("button", { name: "Practice" });
    expect(libraryPractice.hasAttribute("disabled")).toBe(false);

    const deckButton = screen.getByText("Dog facts").closest("button");
    expect(deckButton).not.toBeNull();
    fireEvent.click(deckButton as HTMLButtonElement);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Dog facts" })).toBeTruthy();
    });

    const detailPractice = screen.getByRole("button", { name: "Practice" });
    expect(detailPractice.hasAttribute("disabled")).toBe(false);
    fireEvent.click(detailPractice);

    expect(onPracticeDeck).toHaveBeenCalledWith(deck.id);
  });
});