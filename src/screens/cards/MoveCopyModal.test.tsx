import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../../ipc";
import { makeDeck, resetTestEntityIds } from "../../test/factories";
import { MoveCopyModal, type TransferableCard } from "./MoveCopyModal";

vi.mock("../../ipc", () => ({
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  transferCards: vi.fn(),
}));

describe("MoveCopyModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTestEntityIds();
    vi.mocked(ipc.transferCards).mockResolvedValue({
      affectedCount: 2,
      skippedDuplicates: 0,
    });
  });

  it("excludes every selected source deck from the destination list", () => {
    const first = makeDeck({ name: "First" });
    const second = makeDeck({ name: "Second" });
    const target = makeDeck({ name: "Target" });
    const cards: TransferableCard[] = [
      { noteId: "a".repeat(32), deckId: first.id },
      { noteId: "b".repeat(32), deckId: second.id },
    ];

    render(
      <MoveCopyModal
        cards={cards}
        decks={[first, second, target]}
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />,
    );

    const destination = screen.getByRole("combobox", {
      name: "Destination deck",
    }) as HTMLSelectElement;
    expect([...destination.options].map((option) => option.textContent)).toEqual([
      "Target",
    ]);
  });

  it("submits copy mode and the explicit duplicate policy", async () => {
    const source = makeDeck({ name: "Source" });
    const target = makeDeck({ name: "Target" });
    const cards: TransferableCard[] = [
      { noteId: "a".repeat(32), deckId: source.id },
      { noteId: "b".repeat(32), deckId: source.id },
    ];
    const onComplete = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    const showToast = vi.fn();

    render(
      <MoveCopyModal
        cards={cards}
        decks={[source, target]}
        onClose={onClose}
        onComplete={onComplete}
        showToast={showToast}
      />,
    );

    fireEvent.click(screen.getByRole("radio", { name: /Copy/u }));
    fireEvent.click(
      screen.getByRole("checkbox", {
        name: "Skip exact duplicates already in the destination",
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Copy 2 cards" }));

    await waitFor(() => {
      expect(ipc.transferCards).toHaveBeenCalledWith({
        noteIds: cards.map((card) => card.noteId),
        targetDeckId: target.id,
        mode: "copy",
        skipDuplicates: false,
      });
      expect(onComplete).toHaveBeenCalledOnce();
      expect(showToast).toHaveBeenCalledWith("success", "Copied 2 cards.");
      expect(onClose).toHaveBeenCalledOnce();
    });
  });

  it("reports duplicate omissions from the authoritative result", async () => {
    vi.mocked(ipc.transferCards).mockResolvedValue({
      affectedCount: 1,
      skippedDuplicates: 1,
    });
    const source = makeDeck({ name: "Source" });
    const target = makeDeck({ name: "Target" });
    const showToast = vi.fn();
    render(
      <MoveCopyModal
        cards={[
          { noteId: "a".repeat(32), deckId: source.id },
          { noteId: "b".repeat(32), deckId: source.id },
        ]}
        decks={[source, target]}
        onClose={vi.fn()}
        onComplete={vi.fn().mockResolvedValue(undefined)}
        showToast={showToast}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Move 2 cards" }));

    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Moved 1 card. 1 duplicate was skipped.",
      );
    });
  });

  it("treats a failed refresh as committed transfer success", async () => {
    vi.mocked(ipc.transferCards).mockResolvedValue({
      affectedCount: 1,
      skippedDuplicates: 0,
    });
    const source = makeDeck({ name: "Source" });
    const target = makeDeck({ name: "Target" });
    const onComplete = vi.fn().mockRejectedValue(new Error("Search refresh failed."));
    const onClose = vi.fn();
    const showToast = vi.fn();

    render(
      <MoveCopyModal
        cards={[{ noteId: "a".repeat(32), deckId: source.id }]}
        decks={[source, target]}
        onClose={onClose}
        onComplete={onComplete}
        showToast={showToast}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Move 1 card" }));

    await waitFor(() => {
      expect(ipc.transferCards).toHaveBeenCalledOnce();
      expect(onComplete).toHaveBeenCalledOnce();
      expect(showToast).toHaveBeenNthCalledWith(1, "success", "Moved 1 card.");
      expect(showToast).toHaveBeenNthCalledWith(
        2,
        "info",
        "The cards were moved, but the current view could not refresh: Search refresh failed.",
      );
      expect(onClose).toHaveBeenCalledOnce();
    });
  });

  it("shows an empty state when no different active destination exists", () => {
    const onlyDeck = makeDeck({ name: "Only deck" });
    render(
      <MoveCopyModal
        cards={[{ noteId: "a".repeat(32), deckId: onlyDeck.id }]}
        decks={[onlyDeck]}
        onClose={vi.fn()}
        onComplete={vi.fn()}
      />,
    );

    expect(
      screen.getByRole("heading", { name: "Create or unarchive another deck first" }),
    ).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Move 1 card/u })).toBeNull();
  });

  it("keeps the modal open and displays a transfer failure", async () => {
    vi.mocked(ipc.transferCards).mockRejectedValue(new Error("Destination changed."));
    const source = makeDeck({ name: "Source" });
    const target = makeDeck({ name: "Target" });
    const onClose = vi.fn();
    render(
      <MoveCopyModal
        cards={[{ noteId: "a".repeat(32), deckId: source.id }]}
        decks={[source, target]}
        onClose={onClose}
        onComplete={vi.fn()}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Move 1 card" }));

    expect(await screen.findByText("Destination changed.")).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });
});
