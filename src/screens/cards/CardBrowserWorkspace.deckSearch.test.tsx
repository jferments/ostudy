import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { CardBrowserRow, CardSearchInput } from "../../ipc/cards";
import type { DeckSummary } from "../../ipc/decks";
import { CardBrowserWorkspace } from "./CardBrowserWorkspace";

const controller = vi.hoisted(() => ({
  search: vi.fn(),
  tags: vi.fn(),
  note: vi.fn(),
  matching: vi.fn(),
  schedule: vi.fn(),
  invalidate: vi.fn(),
}));

vi.mock("../../controllers/cardsController", () => ({
  CardsController: class {
    search = controller.search;
    tags = controller.tags;
    note = controller.note;
    matching = controller.matching;
    schedule = controller.schedule;
    invalidate = controller.invalidate;
  },
}));

const deck: DeckSummary = {
  id: "a".repeat(32),
  name: "Cardiovascular pharmacology",
  description: "NCLEX medication safety",
  sourceNoteCount: 1,
  generatedCardCount: 1,
  studyableDueCount: 1,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

const card: CardBrowserRow = {
  cardId: "b".repeat(32),
  noteId: "c".repeat(32),
  deckId: deck.id,
  deckName: deck.name,
  cardType: "basic",
  cardLabel: "Basic",
  frontText: "What reverses warfarin?",
  backText: "Vitamin K",
  tags: ["anticoagulants"],
  dueAt: Date.now(),
  lastReviewedAt: null,
  reviewCount: 0,
  suspendedAt: null,
  buriedUntil: null,
  needsMorePractice: false,
  createdAt: 1,
  updatedAt: 1,
};

beforeEach(() => {
  vi.clearAllMocks();
  controller.tags.mockResolvedValue([]);
  controller.matching.mockResolvedValue([]);
  controller.search.mockImplementation(async (input: CardSearchInput) =>
    input.query.trim()
      ? { cards: [], totalCount: 0 }
      : { cards: [card], totalCount: 1 },
  );
});

describe("CardBrowserWorkspace deck search", () => {
  it("places the magnifying-glass search action beside the table selection actions", async () => {
    renderDeckBrowser();

    expect(await screen.findByText("What reverses warfarin?")).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Search this deck" })).toBeNull();

    const controls = screen.getByRole("group", { name: "Card table controls" });
    expect(within(controls).getAllByRole("button").map((button) => button.getAttribute("aria-label") ?? button.textContent)).toEqual([
      "Select visible",
      "Select all (1)",
      "Search cards in this deck",
    ]);

    const heading = screen.getByRole("heading", { name: "Cards" });
    expect(heading.parentElement?.textContent).toBe("Cards");
    expect(screen.getByText("1 of 1")).toBeTruthy();
    expect(screen.getByText("cards shown")).toBeTruthy();
  });

  it("expands, focuses, searches, and collapses in the same table-control group", async () => {
    renderDeckBrowser();
    await screen.findByText("What reverses warfarin?");

    const controls = screen.getByRole("group", { name: "Card table controls" });
    fireEvent.click(
      within(controls).getByRole("button", { name: "Search cards in this deck" }),
    );

    const search = within(controls).getByRole("searchbox", {
      name: "Search cards in this deck",
    });
    expect(document.activeElement).toBe(search);
    fireEvent.change(search, { target: { value: "aspirin" } });

    await waitFor(() =>
      expect(controller.search).toHaveBeenLastCalledWith(
        expect.objectContaining({ deckIds: [deck.id], query: "aspirin" }),
      ),
    );
    expect(await screen.findByRole("heading", { name: "No matching cards" })).toBeTruthy();
    expect(
      within(controls).getByRole("searchbox", { name: "Search cards in this deck" }),
    ).toBeTruthy();

    fireEvent.keyDown(search, { key: "Escape" });
    expect(
      within(controls).queryByRole("searchbox", { name: "Search cards in this deck" }),
    ).toBeNull();
    expect(
      within(controls).getByRole("button", { name: "Search cards in this deck" }),
    ).toBeTruthy();
  });
});

function renderDeckBrowser() {
  return render(
    <CardBrowserWorkspace
      decks={[deck]}
      fixedDeckId={deck.id}
      controls="deck"
      showPageHeader={false}
      refreshDecks={async () => [deck]}
      onPracticeCards={async () => undefined}
      onDirtyChange={vi.fn()}
      showToast={vi.fn()}
    />,
  );
}
