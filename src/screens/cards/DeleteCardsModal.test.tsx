import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../../ipc";
import { DeleteCardsModal } from "./DeleteCardsModal";

vi.mock("../../ipc", () => ({
  deleteCards: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));

const cards: ipc.SelectionCard[] = [
  { cardId: "a".repeat(32), noteId: "b".repeat(32), deckId: "c".repeat(32) },
  { cardId: "d".repeat(32), noteId: "e".repeat(32), deckId: "c".repeat(32) },
];

describe("DeleteCardsModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.deleteCards).mockResolvedValue({ affectedCount: 2 });
  });

  it("deletes unique note identities and refreshes after the durable mutation", async () => {
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

    expect(screen.getByText(/local scheduling state/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Delete 2 cards" }));

    await waitFor(() => {
      expect(ipc.deleteCards).toHaveBeenCalledWith(
        cards.map((card) => card.noteId),
      );
      expect(onComplete).toHaveBeenCalledOnce();
      expect(showToast).toHaveBeenCalledWith("success", "Deleted 2 cards.");
      expect(onClose).toHaveBeenCalledOnce();
    });
  });

  it("uses singular confirmation copy for one source note", async () => {
    vi.mocked(ipc.deleteCards).mockResolvedValue({ affectedCount: 1 });
    const showToast = vi.fn();
    render(
      <DeleteCardsModal
        cards={[cards[0] as ipc.SelectionCard]}
        onClose={vi.fn()}
        onComplete={vi.fn().mockResolvedValue(undefined)}
        showToast={showToast}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Delete 1 card" }));

    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith("success", "Deleted 1 card.");
    });
  });

  it("surfaces a deletion failure without closing or refreshing", async () => {
    vi.mocked(ipc.deleteCards).mockRejectedValue(new Error("Deletion failed."));
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
    expect(ipc.deleteCards).not.toHaveBeenCalled();
  });
});
