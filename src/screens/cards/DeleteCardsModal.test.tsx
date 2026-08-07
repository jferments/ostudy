import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ToastAction } from "../../components/Toast";
import * as cardsIpc from "../../ipc/cards";
import * as commonIpc from "../../ipc/common";
import { DeleteCardsModal } from "./DeleteCardsModal";

vi.mock("../../ipc/cards", () => ({
  deleteCards: vi.fn(),
  undoDeletion: vi.fn(),
}));
vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));

const cards: cardsIpc.SelectionCard[] = [
  { cardId: "a".repeat(32), noteId: "b".repeat(32), deckId: "c".repeat(32) },
  { cardId: "d".repeat(32), noteId: "e".repeat(32), deckId: "c".repeat(32) },
];
const undoToken = "f".repeat(32);

function deletionResult(affectedCount: number): commonIpc.DeletionResult {
  return {
    affectedCount,
    undoToken,
    undoExpiresAt: Date.now() + 10_000,
    subjectLabel: affectedCount === 1 ? "CardPresentation" : `${affectedCount} flashcards`,
  };
}

describe("DeleteCardsModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(cardsIpc.deleteCards).mockResolvedValue(deletionResult(2));
    vi.mocked(cardsIpc.undoDeletion).mockResolvedValue({
      restoredCount: 2,
      entityKind: "cards",
      subjectLabel: "2 flashcards",
    });
  });

  it("deletes unique note identities and publishes an exact undo action", async () => {
    const onComplete = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    const showToast = vi.fn();
    render(
      <DeleteCardsModal
        cards={cards}
        onClose={onClose}
        onComplete={onComplete}
        showToast={showToast}
      />,
    );

    expect(screen.getByText(/brief Undo window/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Delete 2 cards" }));

    await waitFor(() => {
      expect(cardsIpc.deleteCards).toHaveBeenCalledWith(
        cards.map((card) => card.noteId),
      );
      expect(onComplete).toHaveBeenCalledOnce();
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Deleted 2 cards.",
        expect.objectContaining({ label: "Undo" }),
      );
      expect(onClose).toHaveBeenCalledOnce();
    });

    const action = showToast.mock.calls[0]?.[2] as ToastAction;
    await action.run();
    expect(cardsIpc.undoDeletion).toHaveBeenCalledWith(undoToken);
    expect(onComplete).toHaveBeenCalledTimes(2);
  });

  it("uses singular confirmation and undo copy for one source note", async () => {
    vi.mocked(cardsIpc.deleteCards).mockResolvedValue(deletionResult(1));
    const showToast = vi.fn();
    render(
      <DeleteCardsModal
        cards={[cards[0] as cardsIpc.SelectionCard]}
        onClose={vi.fn()}
        onComplete={vi.fn().mockResolvedValue(undefined)}
        showToast={showToast}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Delete 1 card" }));

    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Deleted 1 card.",
        expect.objectContaining({ label: "Undo" }),
      );
    });
  });

  it("surfaces a deletion failure without closing or refreshing", async () => {
    vi.mocked(cardsIpc.deleteCards).mockRejectedValue(new Error("Deletion failed."));
    const onComplete = vi.fn();
    const onClose = vi.fn();
    render(
      <DeleteCardsModal
        cards={cards}
        onClose={onClose}
        onComplete={onComplete}
        showToast={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Delete 2 cards" }));

    expect(await screen.findByText("Deletion failed.")).toBeTruthy();
    expect(onComplete).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("does not issue a command for an empty selection", () => {
    render(
      <DeleteCardsModal
        cards={[]}
        onClose={vi.fn()}
        onComplete={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Delete 0 cards" }));
    expect(cardsIpc.deleteCards).not.toHaveBeenCalled();
  });
});
