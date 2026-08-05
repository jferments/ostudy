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
  beginAssetDraft: vi.fn(),
  cancelAssetDraft: vi.fn(),
  createGeneratedCard: vi.fn(),
  findDuplicateCards: vi.fn(),
  generateCardsFromTemplate: vi.fn(),
  getCardTypeInfo: vi.fn(),
  getErrorCode: (error: unknown) =>
    typeof error === "object" && error !== null && "code" in error
      ? String((error as { code: unknown }).code)
      : null,
  getErrorMessage: vi.fn((error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  ),
  getOrganizedCard: vi.fn(),
  getCardTemplate: vi.fn(),
  listCardTemplateSummaries: vi.fn(),
  listDecks: vi.fn(),
  listTags: vi.fn(),
  previewTemplateCards: vi.fn(),
  syncAssetDraft: vi.fn(),
  transferCards: vi.fn(),
  updateGeneratedCard: vi.fn(),
}));

const deck: ipc.Deck = {
  id: "a".repeat(32),
  name: "Pharmacology",
  description: "",
  sourceNoteCount: 2,
  generatedCardCount: 2,
  studyableDueCount: 1,
  createdAt: 1,
  updatedAt: 1,
  archivedAt: null,
  revision: 1,
};

const secondDeck: ipc.Deck = {
  ...deck,
  id: "e".repeat(32),
  name: "Pathophysiology",
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
  archivedAt: null,
  revision: 1,
};

const draftId = "9".repeat(32);

describe("CardEditorScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.beginAssetDraft).mockResolvedValue({
      id: draftId,
      purpose: "card",
      expiresAt: Date.now() + 10_000,
    });
    vi.mocked(ipc.cancelAssetDraft).mockResolvedValue(undefined);
    vi.mocked(ipc.syncAssetDraft).mockResolvedValue(undefined);
    vi.mocked(ipc.listDecks).mockResolvedValue([deck, secondDeck]);
    vi.mocked(ipc.listTags).mockResolvedValue([]);
    vi.mocked(ipc.listCardTemplateSummaries).mockResolvedValue([]);
    vi.mocked(ipc.getCardTypeInfo).mockResolvedValue({
      cardType: "basic",
      reverseEnabled: false,
      clozeSource: "",
      clozeMode: "one_at_a_time",
      generatedCardCount: 1,
    });
    vi.mocked(ipc.findDuplicateCards).mockResolvedValue([
      {
        noteId: "d".repeat(32),
        deckId: secondDeck.id,
        frontText: "Question",
        backText: "Answer",
      },
    ]);
    vi.mocked(ipc.updateGeneratedCard).mockResolvedValue({
      ...card,
      deckId: secondDeck.id,
    });
  });

  it("opens a managed draft, loads the saved Basic type, and commits that draft with the selected deck", async () => {
    const onSaved = vi.fn();
    render(
      <CardEditorScreen
        deck={deck}
        decks={[deck, secondDeck]}
        card={card}
        onCancel={vi.fn()}
        onSaved={onSaved}
        onTemplateGenerated={vi.fn()}
        onDirtyChange={vi.fn()}
      />,
    );

    expect(await screen.findByText("Medication")).toBeTruthy();
    await waitFor(() => {
      expect(ipc.beginAssetDraft).toHaveBeenCalledWith("card", card.noteId);
      expect(ipc.getCardTypeInfo).toHaveBeenCalledWith(card.noteId);
      expect(
        (screen.getByRole("combobox", { name: "Card template" }) as HTMLSelectElement)
          .value,
      ).toBe("basic");
    });

    const question = screen.getByText("Question editor");
    const answer = screen.getByText("Answer editor");
    const tagInput = screen.getByText("Tags");
    expect(
      question.compareDocumentPosition(answer) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      answer.compareDocumentPosition(tagInput) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();

    fireEvent.change(screen.getByRole("combobox", { name: "Deck" }), {
      target: { value: secondDeck.id },
    });
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
      expect(ipc.findDuplicateCards).toHaveBeenCalledWith(
        expect.objectContaining({ deckId: secondDeck.id }),
      );
      expect(ipc.updateGeneratedCard).toHaveBeenCalledWith(
        expect.objectContaining({
          noteId: card.noteId,
          deckId: secondDeck.id,
          cardType: "basic",
          reverseEnabled: false,
          tags: ["Medication"],
          allowDuplicate: true,
          assetDraftId: draftId,
        }),
      );
      expect(onSaved).toHaveBeenCalled();
    });
  });
});
