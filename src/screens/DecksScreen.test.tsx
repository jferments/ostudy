import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { DecksScreen } from "./DecksScreen";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("../ipc", () => ({
  browseCards: vi.fn(),
  createDeck: vi.fn(),
  deleteCard: vi.fn(),
  deleteDeck: vi.fn(),
  exportDeck: vi.fn(),
  getDeck: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  importDeck: vi.fn(),
  updateDeck: vi.fn(),
}));

vi.mock("./CardEditorScreen", () => ({
  CardEditorScreen: () => <div>Card editor</div>,
}));

const deck: ipc.Deck = {
  id: "a".repeat(32),
  name: "Dog facts",
  description: "Dogs are cool animals",
  cardCount: 1,
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

const browserResult: ipc.CardBrowserResult = {
  items: [
    {
      card,
      tags: [{ id: "d".repeat(32), name: "animals" }],
      dueAt: Date.now() + 60_000,
      scheduleState: 2,
      reviewCount: 3,
    },
  ],
  total: 1,
  tags: [{ id: "d".repeat(32), name: "animals", cardCount: 1 }],
  limit: 100,
  offset: 0,
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
};

function renderDecks(onPracticeDeck = vi.fn()) {
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
  return onPracticeDeck;
}

async function openDogDeck(): Promise<void> {
  const deckButton = screen.getByText("Dog facts").closest("button");
  expect(deckButton).not.toBeNull();
  fireEvent.click(deckButton as HTMLButtonElement);
  await waitFor(() => {
    expect(screen.getByRole("heading", { name: "Dog facts" })).toBeTruthy();
    expect(screen.getByText("Do dogs have fur?")).toBeTruthy();
  });
}

describe("DecksScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.getDeck).mockResolvedValue(deck);
    vi.mocked(ipc.browseCards).mockResolvedValue(browserResult);
  });

  it("keeps Practice enabled for a non-empty deck with no cards due", async () => {
    const onPracticeDeck = renderDecks();

    const libraryPractice = screen.getByRole("button", { name: "Practice" });
    expect(libraryPractice.hasAttribute("disabled")).toBe(false);

    await openDogDeck();
    const detailPractice = screen.getByRole("button", { name: "Practice" });
    expect(detailPractice.hasAttribute("disabled")).toBe(false);
    fireEvent.click(detailPractice);

    expect(onPracticeDeck).toHaveBeenCalledWith(deck.id);
  });

  it("sends search text to the indexed browser command", async () => {
    renderDecks();
    await openDogDeck();

    fireEvent.change(screen.getByRole("searchbox", { name: "Search this deck" }), {
      target: { value: "fur" },
    });

    await waitFor(() => {
      expect(ipc.browseCards).toHaveBeenLastCalledWith(
        expect.objectContaining({
          deckId: deck.id,
          query: "fur",
          sort: "relevance",
        }),
      );
    });
  });

  it("reveals move and copy actions after keyboard-friendly selection", async () => {
    renderDecks();
    await openDogDeck();

    fireEvent.click(screen.getByRole("checkbox", { name: /Select Do dogs have fur/iu }));
    expect(screen.getByRole("button", { name: "Move" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Copy" })).toBeTruthy();
  });
});
