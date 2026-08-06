import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../../ipc";
import { makeDeck, resetTestEntityIds } from "../../test/factories";
import { BulkEntryModal } from "./BulkEntryModal";

vi.mock("../../ipc", () => ({
  analyzeBulkCards: vi.fn(),
  bulkCreateCards: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));

let deck: ipc.DeckSummary;

function fillFirstRow(): void {
  fireEvent.change(screen.getByLabelText("Question 1"), {
    target: { value: "What is ATP?" },
  });
  fireEvent.change(screen.getByLabelText("Answer 1"), {
    target: { value: "Energy currency" },
  });
  fireEvent.change(screen.getByLabelText("Tags 1"), {
    target: { value: "Cell, Energy" },
  });
}

describe("BulkEntryModal", () => {
  beforeEach(() => {
    resetTestEntityIds();
    deck = makeDeck({
      name: "Biology",
      sourceNoteCount: 0,
      generatedCardCount: 0,
      studyableDueCount: 0,
    });
    vi.mocked(ipc.analyzeBulkCards).mockResolvedValue({
      uniqueCount: 1,
      duplicateCount: 0,
      duplicateRowIndices: [],
    });
    vi.mocked(ipc.bulkCreateCards).mockResolvedValue({
      createdCount: 1,
      skippedDuplicates: 0,
    });
  });

  it("analyzes and submits populated rows without a duplicate-policy checkbox", async () => {
    render(
      <BulkEntryModal
        decks={[deck]}
        onClose={vi.fn()}
        onSaved={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    fillFirstRow();
    expect(screen.queryByText("Skip exact duplicates")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Add 1 card" }));

    const expectedRows = [
      {
        question: "What is ATP?",
        answer: "Energy currency",
        tags: ["Cell", "Energy"],
      },
    ];
    await waitFor(() => {
      expect(ipc.analyzeBulkCards).toHaveBeenCalledWith({
        deckId: deck.id,
        rows: expectedRows,
      });
      expect(ipc.bulkCreateCards).toHaveBeenCalledWith({
        deckId: deck.id,
        rows: expectedRows,
      });
    });
  });

  it("stops for confirmation and removes exact duplicates before saving", async () => {
    vi.mocked(ipc.analyzeBulkCards).mockResolvedValue({
      uniqueCount: 0,
      duplicateCount: 1,
      duplicateRowIndices: [0],
    });
    render(
      <BulkEntryModal
        decks={[deck]}
        onClose={vi.fn()}
        onSaved={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    fillFirstRow();
    fireEvent.click(screen.getByRole("button", { name: "Add 1 card" }));

    expect(await screen.findByText("Remove 1 exact duplicate?")).toBeTruthy();
    expect(
      screen.getByText("Every populated row is already present.", { exact: false }),
    ).toBeTruthy();
    expect(ipc.bulkCreateCards).not.toHaveBeenCalled();
  });
});
