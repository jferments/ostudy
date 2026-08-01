import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../../ipc";
import { BulkEntryModal } from "./BulkEntryModal";

vi.mock("../../ipc", () => ({
  bulkCreateCards: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));

const deck: ipc.Deck = {
  id: "a".repeat(32),
  name: "Biology",
  description: "",
  cardCount: 0,
  dueCount: 0,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

describe("BulkEntryModal", () => {
  beforeEach(() => {
    vi.mocked(ipc.bulkCreateCards).mockResolvedValue({
      createdCount: 1,
      skippedDuplicates: 0,
    });
  });

  it("submits populated rows and parsed tags", async () => {
    render(
      <BulkEntryModal
        decks={[deck]}
        onClose={vi.fn()}
        onSaved={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    fireEvent.change(screen.getByLabelText("Question 1"), {
      target: { value: "What is ATP?" },
    });
    fireEvent.change(screen.getByLabelText("Answer 1"), {
      target: { value: "Energy currency" },
    });
    fireEvent.change(screen.getByLabelText("Tags 1"), {
      target: { value: "Cell, Energy" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Add 1 card" }));

    await waitFor(() => {
      expect(ipc.bulkCreateCards).toHaveBeenCalledWith({
        deckId: deck.id,
        skipDuplicates: true,
        rows: [
          {
            question: "What is ATP?",
            answer: "Energy currency",
            tags: ["Cell", "Energy"],
          },
        ],
      });
    });
  });
});
