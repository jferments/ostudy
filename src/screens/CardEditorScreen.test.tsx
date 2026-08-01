import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { CardEditorScreen } from "./CardEditorScreen";

vi.mock("../editor/RichTextEditor", () => ({
  RichTextEditor: ({ label }: { label: string }) => <div>{label} editor</div>,
}));
vi.mock("../editor/RichTextView", () => ({
  RichTextView: () => <div>Preview</div>,
}));
vi.mock("../ipc", () => ({
  createOrganizedCard: vi.fn(),
  findDuplicateCards: vi.fn(),
  getErrorCode: (error: unknown) =>
    typeof error === "object" && error !== null && "code" in error
      ? String((error as { code: unknown }).code)
      : null,
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  getOrganizedCard: vi.fn(),
  listDecks: vi.fn(),
  listTags: vi.fn(),
  transferCards: vi.fn(),
  updateOrganizedCard: vi.fn(),
}));

const deck: ipc.Deck = {
  id: "a".repeat(32),
  name: "Pharmacology",
  description: "",
  cardCount: 2,
  dueCount: 1,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

const card: ipc.OrganizedCard = {
  id: "b".repeat(32),
  noteId: "c".repeat(32),
  deckId: deck.id,
  direction: "forward",
  frontDoc: {
    type: "doc",
    content: [
      { type: "paragraph", content: [{ type: "text", text: "Question" }] },
    ],
  },
  backDoc: {
    type: "doc",
    content: [
      { type: "paragraph", content: [{ type: "text", text: "Answer" }] },
    ],
  },
  frontText: "Question",
  backText: "Answer",
  assets: [],
  tags: ["Medication"],
  dueAt: Date.now(),
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

describe("CardEditorScreen", () => {
  beforeEach(() => {
    vi.mocked(ipc.listDecks).mockResolvedValue([deck]);
    vi.mocked(ipc.listTags).mockResolvedValue([]);
    vi.mocked(ipc.findDuplicateCards).mockResolvedValue([
      {
        noteId: "d".repeat(32),
        deckId: deck.id,
        frontText: "Question",
        backText: "Answer",
      },
    ]);
    vi.mocked(ipc.updateOrganizedCard).mockResolvedValue(card);
  });

  it("warns before saving an exact duplicate and permits an explicit override", async () => {
    const onSaved = vi.fn();
    render(
      <CardEditorScreen
        deck={deck}
        card={card}
        onCancel={vi.fn()}
        onSaved={onSaved}
        onDirtyChange={vi.fn()}
      />,
    );

    expect(await screen.findByText("Medication")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Save card" }));

    expect(
      await screen.findByRole("heading", {
        name: "This looks like an exact duplicate",
      }),
    ).toBeTruthy();

    fireEvent.click(
      screen.getByRole("button", { name: "Save duplicate anyway" }),
    );

    await waitFor(() => {
      expect(ipc.updateOrganizedCard).toHaveBeenCalledWith(
        expect.objectContaining({
          noteId: card.noteId,
          tags: ["Medication"],
          allowDuplicate: true,
        }),
      );
      expect(onSaved).toHaveBeenCalled();
    });
  });
});
