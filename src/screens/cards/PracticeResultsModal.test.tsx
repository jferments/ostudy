import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../../ipc";
import { makeDeck, resetTestEntityIds } from "../../test/factories";
import { PracticeResultsModal } from "./PracticeResultsModal";

vi.mock("../../ipc", () => ({
  createDeck: vi.fn(),
  deleteDeck: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  transferCards: vi.fn(),
}));

const cards: ipc.SelectionCard[] = [
  { cardId: "a".repeat(32), noteId: "b".repeat(32), deckId: "c".repeat(32) },
  { cardId: "d".repeat(32), noteId: "e".repeat(32), deckId: "c".repeat(32) },
];

function cleanupDeletionResult(): ipc.DeletionResult {
  return {
    affectedCount: 0,
    undoToken: "f".repeat(32),
    undoExpiresAt: Date.now() + 10_000,
    subjectLabel: "Temporary deck",
  };
}

describe("PracticeResultsModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTestEntityIds();
    vi.mocked(ipc.deleteDeck).mockResolvedValue(cleanupDeletionResult());
  });

  it("starts a temporary exact-card session without creating a deck", () => {
    const onPracticeCards = vi.fn();
    const onClose = vi.fn();
    render(
      <PracticeResultsModal
        cards={cards}
        sourceLabel="the current filtered results"
        refreshDecks={vi.fn().mockResolvedValue([])}
        onPracticeCards={onPracticeCards}
        onClose={onClose}
        showToast={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Start practice" }));

    expect(onPracticeCards).toHaveBeenCalledWith(cards.map((card) => card.cardId));
    expect(ipc.createDeck).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("creates a permanent deck by copying source notes with fresh schedules", async () => {
    const created = makeDeck({ name: "Weak topics" });
    vi.mocked(ipc.createDeck).mockResolvedValue(created);
    vi.mocked(ipc.transferCards).mockResolvedValue({
      affectedCount: 2,
      skippedDuplicates: 0,
    });
    const refreshDecks = vi.fn().mockResolvedValue([created]);
    const showToast = vi.fn();
    const onClose = vi.fn();
    render(
      <PracticeResultsModal
        cards={cards}
        sourceLabel="the selected cards"
        refreshDecks={refreshDecks}
        onPracticeCards={vi.fn()}
        onClose={onClose}
        showToast={showToast}
      />,
    );

    fireEvent.change(screen.getByRole("textbox", { name: "New deck name" }), {
      target: { value: "  Weak topics  " },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save as new deck" }));

    await waitFor(() => {
      expect(ipc.createDeck).toHaveBeenCalledWith({
        name: "Weak topics",
        description: "Created from the selected cards in Search/Filter.",
      });
      expect(ipc.transferCards).toHaveBeenCalledWith({
        noteIds: cards.map((card) => card.noteId),
        targetDeckId: created.id,
        mode: "copy",
        skipDuplicates: true,
      });
      expect(refreshDecks).toHaveBeenCalledOnce();
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Created “Weak topics” with 2 cards.",
      );
      expect(onClose).toHaveBeenCalledOnce();
    });
    expect(ipc.deleteDeck).not.toHaveBeenCalled();
  });

  it("removes the newly created empty deck when no cards can be copied", async () => {
    const created = makeDeck({ name: "Empty result" });
    vi.mocked(ipc.createDeck).mockResolvedValue(created);
    vi.mocked(ipc.transferCards).mockResolvedValue({
      affectedCount: 0,
      skippedDuplicates: 2,
    });
    render(
      <PracticeResultsModal
        cards={cards}
        sourceLabel="the current results"
        refreshDecks={vi.fn().mockResolvedValue([])}
        onPracticeCards={vi.fn()}
        onClose={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByRole("textbox", { name: "New deck name" }), {
      target: { value: "Empty result" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save as new deck" }));

    expect(
      await screen.findByText("No cards could be copied into the new deck."),
    ).toBeTruthy();
    expect(ipc.deleteDeck).toHaveBeenCalledWith(created.id);
  });

  it("compensates for a failed copy after deck creation", async () => {
    const created = makeDeck({ name: "Incomplete" });
    vi.mocked(ipc.createDeck).mockResolvedValue(created);
    vi.mocked(ipc.transferCards).mockRejectedValue(new Error("Copy failed."));
    const onClose = vi.fn();
    render(
      <PracticeResultsModal
        cards={cards}
        sourceLabel="the current results"
        refreshDecks={vi.fn().mockResolvedValue([])}
        onPracticeCards={vi.fn()}
        onClose={onClose}
        showToast={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByRole("textbox", { name: "New deck name" }), {
      target: { value: "Incomplete" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save as new deck" }));

    expect(await screen.findByText("Copy failed.")).toBeTruthy();
    expect(ipc.deleteDeck).toHaveBeenCalledWith(created.id);
    expect(onClose).not.toHaveBeenCalled();
  });
});
