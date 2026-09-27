import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
vi.mock("../../ipc/decks", () => ({ saveSelectionAsDeck: vi.fn() }));

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
  render(<PracticeResultsModal cards={cards} sourceLabel="Search/Filter" {...props} />);
  return props;
}

function saveNamedDeck(name: string): void {
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

  it("saves through one native operation and reports actual study-card counts", async () => {
    const result = committed();
    result.deck.generatedCardCount = 3;
    vi.mocked(decksIpc.saveSelectionAsDeck).mockResolvedValue(result);
    const props = mount();
    saveNamedDeck("  Weak topics  ");
    await waitFor(() => expect(props.onClose).toHaveBeenCalledOnce());
    expect(decksIpc.saveSelectionAsDeck).toHaveBeenCalledWith({
      operationId: "1".repeat(32),
      name: "Weak topics",
      description: "Created from a card selection in Search/Filter.",
      noteIds: cards.map((card) => card.noteId).sort(),
    });
    expect(props.refreshDecks).toHaveBeenCalledOnce();
    expect(props.showToast).toHaveBeenCalledWith("success", "Created “Weak topics” with 3 cards.");
  });

  it("keeps a committed deck successful when the follow-up refresh fails", async () => {
    vi.mocked(decksIpc.saveSelectionAsDeck).mockResolvedValue(committed("Committed"));
    const props = mount({ refreshDecks: vi.fn().mockRejectedValue(new Error("Refresh unavailable.")) });
    saveNamedDeck("Committed");
    await waitFor(() => expect(props.onClose).toHaveBeenCalledOnce());
    expect(props.showToast).toHaveBeenCalledWith("success", "Created “Committed” with 2 cards.");
    expect(props.showToast).toHaveBeenCalledWith("info", expect.stringContaining("The deck was created, but refreshing"));
    fireEvent.click(screen.getByRole("button", { name: "Deck saved" }));
    expect(decksIpc.saveSelectionAsDeck).toHaveBeenCalledOnce();
  });

  it("shows a native failure without compensating in React", async () => {
    vi.mocked(decksIpc.saveSelectionAsDeck).mockRejectedValue(new Error("Copy failed."));
    const props = mount();
    saveNamedDeck("Incomplete");
    expect(await screen.findByText("Copy failed.")).toBeTruthy();
    expect(props.refreshDecks).not.toHaveBeenCalled();
    expect(props.onClose).not.toHaveBeenCalled();
  });

  it("retains the same identity and input when a lost acknowledgement is retried", async () => {
    vi.mocked(decksIpc.saveSelectionAsDeck)
      .mockRejectedValueOnce(new Error("IPC reply lost."))
      .mockResolvedValueOnce(committed("Retry safely"));
    const props = mount();
    saveNamedDeck("Retry safely");
    expect(await screen.findByText("IPC reply lost.")).toBeTruthy();
    const field = screen.getByRole("textbox", { name: "New deck name" });
    expect(field.hasAttribute("disabled")).toBe(true);
    fireEvent.change(field, { target: { value: "Different name" } });
    fireEvent.click(screen.getByRole("button", { name: "Retry save" }));
    await waitFor(() => expect(props.onClose).toHaveBeenCalledOnce());
    const calls = vi.mocked(decksIpc.saveSelectionAsDeck).mock.calls;
    expect(calls).toHaveLength(2);
    expect(calls[1]?.[0]).toEqual(calls[0]?.[0]);
    expect(calls[1]?.[0].name).toBe("Retry safely");
    expect(commonIpc.createOperationId).toHaveBeenCalledOnce();
  });

  it("cannot recreate committed content after notification and navigation callbacks throw", async () => {
    vi.mocked(decksIpc.saveSelectionAsDeck).mockResolvedValue(committed("Saved once"));
    const props = mount({
      showToast: vi.fn(() => { throw new Error("Notification failed"); }),
      onClose: vi.fn(() => { throw new Error("Navigation failed"); }),
    });
    saveNamedDeck("Saved once");
    expect(await screen.findByText("The deck is saved. Closing this panel failed: Navigation failed")).toBeTruthy();
    expect(screen.getByText("Created “Saved once” with 2 cards.")).toBeTruthy();
    const save = screen.getByRole("button", { name: "Deck saved" });
    expect(save.hasAttribute("disabled")).toBe(true);
    fireEvent.click(save);
    expect(decksIpc.saveSelectionAsDeck).toHaveBeenCalledOnce();
    expect(commonIpc.createOperationId).toHaveBeenCalledOnce();
    expect(props.refreshDecks).toHaveBeenCalledOnce();
  });

  it("does not issue concurrent mutations from repeated save gestures", async () => {
    let finish: ((result: decksIpc.SaveSelectionAsDeckResult) => void) | undefined;
    vi.mocked(decksIpc.saveSelectionAsDeck).mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    const props = mount();
    fireEvent.change(screen.getByRole("textbox", { name: "New deck name" }), { target: { value: "Once" } });
    const save = screen.getByRole("button", { name: "Save as new deck" });
    act(() => { fireEvent.click(save); fireEvent.click(save); });
    expect(decksIpc.saveSelectionAsDeck).toHaveBeenCalledOnce();
    if (!finish) throw new Error("Expected the pending native request");
    finish(committed("Once"));
    await waitFor(() => expect(props.onClose).toHaveBeenCalledOnce());
    expect(decksIpc.saveSelectionAsDeck).toHaveBeenCalledOnce();
  });
});
