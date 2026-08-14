import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as cardsIpc from "../ipc/cards";
import * as commonIpc from "../ipc/common";
import * as decksIpc from "../ipc/decks";
import * as schedulingIpc from "../ipc/scheduling";
import { CardsScreen } from "./CardsScreen";

vi.mock("../controllers/useDurableDraft", () => ({
  useDurableDraft: () => ({
    draftId: "d".repeat(32),
    assetDraftId: "7".repeat(32),
    revision: 1,
    status: "saved",
    errorMessage: null,
    recovered: false,
    hasUnsavedLocalChanges: false,
    flush: vi.fn().mockResolvedValue(undefined),
    discard: vi.fn().mockResolvedValue(undefined),
    commit: vi.fn().mockResolvedValue(undefined),
    restart: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock("../ipc/cards", () => ({
  analyzeBulkCards: vi.fn(),
  bulkCreateCards: vi.fn(),
  bulkTagCards: vi.fn(),
  deleteCards: vi.fn(),
  getCardNote: vi.fn(),
  listMatchingCards: vi.fn(),
  listTags: vi.fn(),
  searchCards: vi.fn(),
  transferCards: vi.fn(),
}));
vi.mock("../ipc/common", () => ({
  createMutationId: vi.fn(() => "f".repeat(32)),
  getErrorCode: (error: unknown) => {
    if (typeof error !== "object" || error === null) return null;
    const code = (error as { code?: unknown }).code;
    return typeof code === "string" ? code : null;
  },
  getErrorMessage: vi.fn((error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  ),
}));
vi.mock("../ipc/decks", () => ({
  createDeck: vi.fn(),
  deleteDeck: vi.fn(),
}));
vi.mock("../ipc/practice", () => ({
  startCardPractice: vi.fn(),
}));
vi.mock("../ipc/scheduling", () => ({
  applyScheduleAction: vi.fn(),
}));

vi.mock("./CardEditorScreen", () => ({
  CardEditorScreen: ({
    deck,
    onSaved,
  }: {
    deck: decksIpc.DeckSummary;
    onSaved: (card: { deckId: string }, continueAdding: boolean) => Promise<void>;
  }) => (
    <div>
      <span>Card editor</span>
      <button type="button" onClick={() => void onSaved({ deckId: deck.id }, true)}>
        Mock save + another
      </button>
    </div>
  ),
}));

const deck: decksIpc.DeckSummary = {
  id: "a".repeat(32),
  name: "Pharmacology",
  description: "",
  sourceNoteCount: 2,
  generatedCardCount: 2,
  studyableDueCount: 1,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};
const secondDeck: decksIpc.DeckSummary = {
  ...deck,
  id: "e".repeat(32),
  name: "Pathophysiology",
};
const browserCard: cardsIpc.CardBrowserRow = {
  cardId: "b".repeat(32),
  noteId: "c".repeat(32),
  deckId: deck.id,
  deckName: deck.name,
  cardType: "basic",
  cardLabel: "Basic",
  frontText: "What reverses warfarin?",
  backText: "Vitamin K",
  tags: ["Anticoagulants"],
  dueAt: Date.now(),
  lastReviewedAt: null,
  reviewCount: 0,
  suspendedAt: null,
  buriedUntil: null,
  needsMorePractice: true,
  createdAt: 1,
  updatedAt: 1,
};
const reverseCard: cardsIpc.CardBrowserRow = {
  ...browserCard,
  cardId: "f".repeat(32),
  cardLabel: "Reverse",
  frontText: "Vitamin K",
  backText: "What reverses warfarin?",
  needsMorePractice: false,
};
const selection: cardsIpc.SelectionCard = {
  cardId: browserCard.cardId,
  noteId: browserCard.noteId,
  deckId: browserCard.deckId,
};

function actionResult(
  affectedCount = 1,
  requestedCount = 1,
): schedulingIpc.ScheduleActionResult {
  return {
    affectedCount,
    requestedCount,
    changedCount: affectedCount,
    unchangedCount: Math.max(0, requestedCount - affectedCount),
    rejectedCount: 0,
    action: "suspend",
    resolvedDueAt: null,
  };
}

function renderScreen(decks: decksIpc.DeckSummary[] = [deck, secondDeck]) {
  const showToast = vi.fn();
  const view = render(
    <CardsScreen
      decks={decks}
      refreshDecks={async () => decks}
      onPracticeCards={vi.fn()}
      onDirtyChange={vi.fn()}
      showToast={showToast}
    />,
  );
  return { ...view, showToast };
}

describe("CardsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(cardsIpc.listTags).mockResolvedValue([
      {
        id: "d".repeat(32),
        name: "Anticoagulants",
        sourceNoteCount: 1,
        generatedCardCount: 1,
        studyableDueCount: 1,
      },
    ]);
    vi.mocked(cardsIpc.searchCards).mockResolvedValue({
      cards: [browserCard],
      totalCount: 1,
    });
    vi.mocked(cardsIpc.listMatchingCards).mockResolvedValue([selection]);
    vi.mocked(schedulingIpc.applyScheduleAction).mockResolvedValue(actionResult());
  });

  it("sends status filters and displays Needs more practice", async () => {
    renderScreen();
    expect(await screen.findByText("Needs more practice")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /All filters/u }));
    const dialog = await screen.findByRole("dialog", { name: "Filter flashcards" });
    fireEvent.change(within(dialog).getByLabelText("Status"), {
      target: { value: "suspended" },
    });
    await waitFor(() =>
      expect(cardsIpc.searchCards).toHaveBeenLastCalledWith(
        expect.objectContaining({ statusFilter: "suspended" }),
      ),
    );
  });

  it("applies an exact-card suspend action from the selection menu", async () => {
    renderScreen();
    fireEvent.click(
      await screen.findByRole("checkbox", {
        name: "Select What reverses warfarin?",
      }),
    );
    fireEvent.change(
      screen.getByRole("combobox", { name: "Actions for selected cards" }),
      { target: { value: "suspend" } },
    );
    await waitFor(() =>
      expect(schedulingIpc.applyScheduleAction).toHaveBeenCalledWith({
        mutationId: "f".repeat(32),
        cardIds: [browserCard.cardId],
        action: "suspend",
      }),
    );
  });

  it("submits every selected presentation in one transactional request", async () => {
    vi.mocked(cardsIpc.searchCards).mockResolvedValue({
      cards: [browserCard, reverseCard],
      totalCount: 2,
    });
    vi.mocked(schedulingIpc.applyScheduleAction).mockResolvedValue(actionResult(1, 2));
    const { showToast } = renderScreen();

    fireEvent.click(await screen.findByRole("button", { name: "Select visible" }));
    fireEvent.change(
      screen.getByRole("combobox", { name: "Actions for selected cards" }),
      { target: { value: "suspend" } },
    );

    await waitFor(() => {
      expect(schedulingIpc.applyScheduleAction).toHaveBeenCalledOnce();
      expect(schedulingIpc.applyScheduleAction).toHaveBeenCalledWith({
        mutationId: "f".repeat(32),
        cardIds: [browserCard.cardId, reverseCard.cardId].sort(),
        action: "suspend",
      });
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Suspended 1 card. 1 card was already suspended.",
      );
    });
  });

  it("reuses one mutation ID after an ambiguous selection response", async () => {
    vi.mocked(schedulingIpc.applyScheduleAction)
      .mockRejectedValueOnce(new Error("Native response was lost."))
      .mockResolvedValueOnce(actionResult());
    renderScreen();
    fireEvent.click(
      await screen.findByRole("checkbox", {
        name: "Select What reverses warfarin?",
      }),
    );
    const actions = screen.getByRole("combobox", {
      name: "Actions for selected cards",
    });

    fireEvent.change(actions, { target: { value: "suspend" } });
    expect(await screen.findByText(/safely retry the complete selection/u)).toBeTruthy();
    fireEvent.change(actions, { target: { value: "suspend" } });

    await waitFor(() => {
      expect(schedulingIpc.applyScheduleAction).toHaveBeenCalledTimes(2);
      expect(commonIpc.createMutationId).toHaveBeenCalledOnce();
      expect(vi.mocked(schedulingIpc.applyScheduleAction).mock.calls[1]?.[0]).toEqual(
        vi.mocked(schedulingIpc.applyScheduleAction).mock.calls[0]?.[0],
      );
    });
  });

  it("selects reverse and forward presentations independently", async () => {
    vi.mocked(cardsIpc.searchCards).mockResolvedValue({
      cards: [browserCard, reverseCard],
      totalCount: 2,
    });
    renderScreen();
    fireEvent.click(await screen.findByRole("checkbox", { name: "Select Vitamin K" }));
    expect(screen.getByText("1 selected")).toBeTruthy();
    expect(
      (
        screen.getByRole("checkbox", {
          name: "Select What reverses warfarin?",
        }) as HTMLInputElement
      ).checked,
    ).toBe(false);
  });

  it("selects all filtered results through the backend", async () => {
    vi.mocked(cardsIpc.searchCards).mockResolvedValue({
      cards: [browserCard],
      totalCount: 12,
    });
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Select all (12)" }));
    await waitFor(() => expect(cardsIpc.listMatchingCards).toHaveBeenCalled());
  });

  it("enables Add card only for one selected active deck", async () => {
    renderScreen();
    await screen.findByText("What reverses warfarin?");
    fireEvent.click(screen.getByRole("button", { name: "All active decks" }));
    const dialog = await screen.findByRole("dialog", { name: "Filter flashcards" });
    fireEvent.click(within(dialog).getByRole("checkbox", { name: /Pharmacology/u }));
    await waitFor(() =>
      expect(cardsIpc.searchCards).toHaveBeenLastCalledWith(
        expect.objectContaining({ deckIds: [deck.id] }),
      ),
    );
    fireEvent.click(screen.getByRole("button", { name: "Add card" }));
    expect(await screen.findByText("Card editor")).toBeTruthy();
  });
});
