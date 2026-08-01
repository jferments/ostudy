import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { CardsScreen } from "./CardsScreen";

vi.mock("../ipc", () => ({
  bulkCreateCards: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  getOrganizedCard: vi.fn(),
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

const browserCard: ipc.CardBrowserRow = {
  cardId: "b".repeat(32),
  noteId: "c".repeat(32),
  deckId: deck.id,
  deckName: deck.name,
  frontText: "What reverses warfarin?",
  backText: "Vitamin K",
  tags: ["Anticoagulants"],
  dueAt: Date.now(),
  createdAt: 1,
  updatedAt: 1,
};

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
  });

  it("uses backend search and renders tags from the card library", async () => {
    render(
      <CardsScreen
        decks={[deck]}
        refreshDecks={async () => [deck]}
        onDirtyChange={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    expect(await screen.findByText("What reverses warfarin?")).toBeTruthy();
    expect(screen.getByText("Anticoagulants")).toBeTruthy();

    fireEvent.change(screen.getByRole("searchbox", { name: "Search cards" }), {
      target: { value: "warfarin" },
    });

    await waitFor(() => {
      expect(ipc.searchCards).toHaveBeenLastCalledWith(
        expect.objectContaining({
          query: "warfarin",
          sort: "relevance",
        }),
      );
    });
  });

  it("supports keyboard selection before move or copy", async () => {
    render(
      <CardsScreen
        decks={[deck]}
        refreshDecks={async () => [deck]}
        onDirtyChange={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    const question = await screen.findByText("What reverses warfarin?");
    const rowButton = question.closest("button");
    expect(rowButton).not.toBeNull();

    fireEvent.keyDown(rowButton as HTMLButtonElement, { key: " " });

    expect(screen.getByText("1 selected")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Move / copy" })).toBeTruthy();
  });

  it("keeps the add-card editor open after Save + another", async () => {
    render(
      <CardsScreen
        decks={[deck]}
        refreshDecks={async () => [deck]}
        onDirtyChange={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    await screen.findByText("What reverses warfarin?");
    fireEvent.change(screen.getByLabelText("Deck"), {
      target: { value: deck.id },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add card" }));
    expect(await screen.findByText("Card editor")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Mock save + another" }));

    await waitFor(() => {
      expect(screen.getByText("Card editor")).toBeTruthy();
    });
  });
});
