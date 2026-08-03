import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../../ipc";
import { BulkTagModal } from "./BulkTagModal";

vi.mock("../../ipc", () => ({
  bulkTagCards: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));

const cards: ipc.SelectionCard[] = [
  { cardId: "a".repeat(32), noteId: "b".repeat(32), deckId: "c".repeat(32) },
  { cardId: "d".repeat(32), noteId: "e".repeat(32), deckId: "c".repeat(32) },
];
const suggestions: ipc.TagSummary[] = [
  { id: "f".repeat(32), name: "Cardiology", cardCount: 12 },
];

describe("BulkTagModal", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.bulkTagCards).mockResolvedValue({ affectedCount: 2 });
  });

  it("adds normalized tags to note-level selections", async () => {
    const onComplete = vi.fn().mockResolvedValue(undefined);
    const onClose = vi.fn();
    const showToast = vi.fn();
    render(
      <BulkTagModal
        cards={cards}
        mode="add"
        suggestions={suggestions}
        onClose={onClose}
        onComplete={onComplete}
        showToast={showToast}
      />,
    );

    const input = screen.getByRole("combobox", { name: "Tags" });
    fireEvent.change(input, { target: { value: "  Medication   safety  " } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.click(screen.getByRole("button", { name: "Add tags" }));

    await waitFor(() => {
      expect(ipc.bulkTagCards).toHaveBeenCalledWith({
        noteIds: cards.map((card) => card.noteId),
        tags: ["Medication safety"],
        mode: "add",
      });
      expect(onComplete).toHaveBeenCalledOnce();
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Added tags to 2 cards.",
      );
      expect(onClose).toHaveBeenCalledOnce();
    });
  });

  it("uses remove semantics without changing unrelated tags", async () => {
    render(
      <BulkTagModal
        cards={[cards[0] as ipc.SelectionCard]}
        mode="remove"
        suggestions={suggestions}
        onClose={vi.fn()}
        onComplete={vi.fn().mockResolvedValue(undefined)}
        showToast={vi.fn()}
      />,
    );

    const input = screen.getByRole("combobox", { name: "Tags" });
    fireEvent.change(input, { target: { value: "Cardiology" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.click(screen.getByRole("button", { name: "Remove tags" }));

    await waitFor(() => {
      expect(ipc.bulkTagCards).toHaveBeenCalledWith({
        noteIds: [cards[0]?.noteId],
        tags: ["Cardiology"],
        mode: "remove",
      });
    });
  });

  it("keeps the modal open and reports a backend failure", async () => {
    vi.mocked(ipc.bulkTagCards).mockRejectedValue(new Error("A card was deleted."));
    const onClose = vi.fn();
    render(
      <BulkTagModal
        cards={cards}
        mode="add"
        suggestions={suggestions}
        onClose={onClose}
        onComplete={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    const input = screen.getByRole("combobox", { name: "Tags" });
    fireEvent.change(input, { target: { value: "Cardiology" } });
    fireEvent.keyDown(input, { key: "Enter" });
    fireEvent.click(screen.getByRole("button", { name: "Add tags" }));

    expect(await screen.findByText("A card was deleted.")).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });

  it("does not submit without both a selection and at least one tag", () => {
    render(
      <BulkTagModal
        cards={[]}
        mode="add"
        suggestions={suggestions}
        onClose={vi.fn()}
        onComplete={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    expect(
      (screen.getByRole("button", { name: "Add tags" }) as HTMLButtonElement).disabled,
    ).toBe(true);
    expect(ipc.bulkTagCards).not.toHaveBeenCalled();
  });
});
