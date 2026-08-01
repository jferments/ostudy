import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { CardsScreen } from "./CardsScreen";

vi.mock("../ipc", () => ({
  analyzeBulkCards: vi.fn(),
  bulkCreateCards: vi.fn(),
  bulkTagCards: vi.fn(),
  createDeck: vi.fn(),
  deleteCards: vi.fn(),
  deleteDeck: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  getOrganizedCard: vi.fn(),
  listMatchingCards: vi.fn(),
  listTags: vi.fn(),
  searchCards: vi.fn(),
  transferCards: vi.fn(),
}));

vi.mock("./CardEditorScreen", () => ({
  CardEditorScreen: ({
    deck,
    onSaved,
  }: {
    deck: ipc.Deck;
    onSaved: (card: { deckId: string }, continueAdding: boolean) => Promise<void>;
  }) => (
    <div>
      <span>Card editor</span>
      <button
        type="button"
        onClick={() => void onSaved({ deckId: deck.id }, true)}
      >
        Mock save + another
      </button>
    </div>
  ),
}));

const deck: ipc.Deck = {
  id: "a".repeat(32),
  name: "Pharmacology",
  description: "",
  cardCount: 2,
  dueCount: 1,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

const secondDeck: ipc.Deck = {
  ...deck,
  id: "e".repeat(32),
  name: "Pathophysiology",
};

const browserCard: ipc.CardBrowserRow = {
  cardId: "b".repeat(32),
  noteId: "c".repeat(32),
  deckId: deck.id,
  deckName: deck.name,
  frontText: "What reverses warfarin?",
  backText: "Vitamin K",
  tags: ["Anticoagulants"],
  dueAt: Date.now(),
  lastReviewedAt: null,
  reviewCount: 0,
  createdAt: 1,
  updatedAt: 1,
};

const selection: ipc.SelectionCard = {
  cardId: browserCard.cardId,
  noteId: browserCard.noteId,
  deckId: browserCard.deckId,
};

function renderScreen(decks: ipc.Deck[] = [deck, secondDeck]) {
  return render(
    <CardsScreen
      decks={decks}
      refreshDecks={async () => decks}
      onPracticeCards={vi.fn()}
      onDirtyChange={vi.fn()}
      showToast={vi.fn()}
    />,
  );
}

describe("CardsScreen", () => {
  beforeEach(() => {
    vi.mocked(ipc.listTags).mockResolvedValue([
      {
        id: "d".repeat(32),
        name: "Anticoagulants",
        cardCount: 1,
      },
    ]);
    vi.mocked(ipc.searchCards).mockResolvedValue({
      cards: [browserCard],
      totalCount: 1,
    });
    vi.mocked(ipc.listMatchingCards).mockResolvedValue([selection]);
  });

  it("renders Search/Filter and sends search and recency criteria to the backend", async () => {
    renderScreen();

    expect(await screen.findByRole("heading", { name: "Search/Filter" })).toBeTruthy();
    expect(await screen.findByText("What reverses warfarin?")).toBeTruthy();
    expect(screen.getAllByText("Anticoagulants").length).toBeGreaterThan(0);

    fireEvent.change(screen.getByRole("searchbox", { name: "Search cards" }), {
      target: { value: "warfarin" },
    });
    fireEvent.change(screen.getByLabelText("Last studied"), {
      target: { value: "last_7_days" },
    });

    await waitFor(() => {
      expect(ipc.searchCards).toHaveBeenLastCalledWith(
        expect.objectContaining({
          deckIds: [],
          query: "warfarin",
          reviewedFilter: "last_7_days",
          sort: "relevance",
        }),
      );
    });
  });

  it("uses the per-card checkbox for selection without opening the editor", async () => {
    renderScreen();

    await screen.findByText("What reverses warfarin?");
    fireEvent.click(
      screen.getByRole("checkbox", { name: "Select What reverses warfarin?" }),
    );

    expect(screen.getByText("1 selected")).toBeTruthy();
    expect(ipc.getOrganizedCard).not.toHaveBeenCalled();
    expect(
      screen.getByRole("combobox", { name: "Actions for selected cards" }),
    ).toBeTruthy();
  });

  it("selects all filtered results through the backend instead of only visible rows", async () => {
    vi.mocked(ipc.searchCards).mockResolvedValue({
      cards: [browserCard],
      totalCount: 12,
    });
    renderScreen();

    fireEvent.click(await screen.findByRole("button", { name: "Select all (12)" }));

    await waitFor(() => {
      expect(ipc.listMatchingCards).toHaveBeenCalled();
      expect(screen.getByText("1 selected")).toBeTruthy();
    });
  });

  it("allows multiple deck filters and enables Add card only for one selected deck", async () => {
    renderScreen();
    await screen.findByText("What reverses warfarin?");

    fireEvent.click(screen.getByText("All decks"));
    fireEvent.click(screen.getByRole("checkbox", { name: /Pharmacology/u }));

    await waitFor(() => {
      expect(ipc.searchCards).toHaveBeenLastCalledWith(
        expect.objectContaining({ deckIds: [deck.id] }),
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Add card" }));
    expect(await screen.findByText("Card editor")).toBeTruthy();
  });
});
