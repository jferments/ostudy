import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { SelectionCard } from "../../ipc/cards";
import * as commonIpc from "../../ipc/common";
import * as decksIpc from "../../ipc/decks";
import { makeDeck, resetTestEntityIds } from "../../test/factories";
import { PracticeResultsModal } from "./PracticeResultsModal";

vi.mock("../../ipc/common", () => ({
  createOperationId: vi.fn(() => "1".repeat(32)),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));
vi.mock("../../ipc/decks", () => ({
  saveSelectionAsDeck: vi.fn(),
}));

const cards: SelectionCard[] = [
  { cardId: "a".repeat(32), noteId: "b".repeat(32), deckId: "c".repeat(32) },
  { cardId: "d".repeat(32), noteId: "e".repeat(32), deckId: "c".repeat(32) },
];

function committed(name = "Weak topics"): decksIpc.SaveSelectionAsDeckResult {
  return {
    operationId: "1".repeat(32),
    deck: makeDeck({ name, sourceNoteCount: 2, generatedCardCount: 2 }),
    affectedCount: 2,
    skippedDuplicates: 0,
  };
}

function mount(overrides: Partial<{
  refreshDecks: () => Promise<decksIpc.DeckSummary[]>;
  onPracticeCards: (cardIds: string[]) => void;
  onClose: () => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}> = {}) {
  const props = {
    refreshDecks: vi.fn().mockResolvedValue([]),
    onPracticeCards: vi.fn(),
    onClose: vi.fn(),
    showToast: vi.fn(),
    ...overrides,
  };
  render(
    <PracticeResultsModal
      cards={cards}
      sourceLabel="Search/Filter"
      refreshDecks={props.refreshDecks}
      onPracticeCards={props.onPracticeCards}
      onClose={props.onClose}
      showToast={props.showToast}
    />,
  );
  return props;
}

async function saveNamedDeck(name: string): Promise<void> {
  fireEvent.change(screen.getByRole("textbox", { name: "New deck name" }), {
    target: { value: name },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save as new deck" }));
}

describe("PracticeResultsModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTestEntityIds();
    vi.mocked(commonIpc.createOperationId).mockReturnValue("1".repeat(32));
  });

  it("starts a temporary exact-card session without creating a deck", () => {
    const props = mount();

    fireEvent.click(screen.getByRole("button", { name: "Start practice" }));

    expect(props.onPracticeCards).toHaveBeenCalledWith(cards.map((card) => card.cardId));
    expect(decksIpc.saveSelectionAsDeck).not.toHaveBeenCalled();
    expect(props.onClose).toHaveBeenCalledOnce();
  });

  it("saves a permanent deck through one native atomic mutation", async () => {
    const result = committed();
    vi.mocked(decksIpc.saveSelectionAsDeck).mockResolvedValue(result);
    const refreshDecks = vi.fn().mockResolvedValue([result.deck]);
    const showToast = vi.fn();
    const onClose = vi.fn();
    mount({ refreshDecks, showToast, onClose });

    await saveNamedDeck("  Weak topics  ");

    await waitFor(() => {
      expect(decksIpc.saveSelectionAsDeck).toHaveBeenCalledWith({
        operationId: "1".repeat(32),
        name: "Weak topics",
        description: "Created from a card selection in Search/Filter.",
        noteIds: cards.map((card) => card.noteId).sort(),
      });
      expect(refreshDecks).toHaveBeenCalledOnce();
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Created “Weak topics” with 2 cards.",
      );
      expect(onClose).toHaveBeenCalledOnce();
    });
  });

  it("keeps a committed deck successful when the follow-up refresh fails", async () => {
    const result = committed("Committed");
    vi.mocked(decksIpc.saveSelectionAsDeck).mockResolvedValue(result);
    const refreshDecks = vi.fn().mockRejectedValue(new Error("Refresh unavailable."));
    const showToast = vi.fn();
    const onClose = vi.fn();
    mount({ refreshDecks, showToast, onClose });

    await saveNamedDeck("Committed");

    await waitFor(() => {
      expect(decksIpc.saveSelectionAsDeck).toHaveBeenCalledOnce();
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Created “Committed” with 2 cards.",
      );
      expect(showToast).toHaveBeenCalledWith(
        "info",
        "The deck was created, but refreshing the deck list failed: Refresh unavailable. Refresh the view before creating it again.",
      );
      expect(onClose).toHaveBeenCalledOnce();
    });
  });

  it("shows a native mutation failure without compensating in React", async () => {
    vi.mocked(decksIpc.saveSelectionAsDeck).mockRejectedValue(new Error("Copy failed."));
    const props = mount();

    await saveNamedDeck("Incomplete");

    expect(await screen.findByText("Copy failed.")).toBeTruthy();
    expect(props.refreshDecks).not.toHaveBeenCalled();
    expect(props.onClose).not.toHaveBeenCalled();
  });

  it("reuses the same operation identity when a lost reply is retried", async () => {
    const result = committed("Retry safely");
    vi.mocked(decksIpc.saveSelectionAsDeck)
      .mockRejectedValueOnce(new Error("IPC reply lost."))
      .mockResolvedValueOnce(result);
    const onClose = vi.fn();
    mount({ onClose });

    await saveNamedDeck("Retry safely");
    expect(await screen.findByText("IPC reply lost.")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Save as new deck" }));

    await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
    expect(decksIpc.saveSelectionAsDeck).toHaveBeenCalledTimes(2);
    const first = vi.mocked(decksIpc.saveSelectionAsDeck).mock.calls[0]?.[0];
    const second = vi.mocked(decksIpc.saveSelectionAsDeck).mock.calls[1]?.[0];
    expect(first?.operationId).toBe("1".repeat(32));
    expect(second?.operationId).toBe(first?.operationId);
    expect(commonIpc.createOperationId).toHaveBeenCalledOnce();
  });
});
