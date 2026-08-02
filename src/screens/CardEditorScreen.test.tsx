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
  updateOrganizedCard: vi.fn(),
  findDuplicateCards: vi.fn(),
  generateCardsFromTemplate: vi.fn(),
  getOrganizedCard: vi.fn(),
  getErrorCode: (error: unknown) =>
    typeof error === "object" && error !== null && "code" in error
      ? String((error as { code: unknown }).code)
      : null,
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  listCardTemplates: vi.fn(),
  listDecks: vi.fn(),
  listTags: vi.fn(),
  transferCards: vi.fn(),
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
  cardType: "basic",
  cardLabel: "Basic",
  generatedCardCount: 1,
  reverseEnabled: false,
  clozeText: "",
  clozeMode: "one_at_a_time",
  frontDoc: {
    type: "doc",
    content: [{ type: "paragraph", content: [{ type: "text", text: "Question" }] }],
  },
  backDoc: {
    type: "doc",
    content: [{ type: "paragraph", content: [{ type: "text", text: "Answer" }] }],
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

const template: ipc.CardTemplate = {
  id: "f".repeat(32),
  name: "Pharmacology entry",
  description: "Create common medication cards.",
  fields: [
    { id: "1".repeat(32), name: "name", description: "", position: 0 },
    { id: "2".repeat(32), name: "drug class", description: "", position: 1 },
  ],
  rules: [
    {
      id: "3".repeat(32),
      name: "Drug class",
      frontPattern: "What is the drug class of {{name}}?",
      backPattern: "{{drug class}}",
      position: 0,
    },
  ],
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

describe("CardEditorScreen", () => {
  beforeEach(() => {
    vi.mocked(ipc.listDecks).mockResolvedValue([deck, secondDeck]);
    vi.mocked(ipc.listTags).mockResolvedValue([]);
    vi.mocked(ipc.listCardTemplates).mockResolvedValue([template]);
    vi.mocked(ipc.findDuplicateCards).mockResolvedValue([]);
    vi.mocked(ipc.updateOrganizedCard).mockResolvedValue({ ...card, deckId: secondDeck.id });
    vi.mocked(ipc.generateCardsFromTemplate).mockResolvedValue({
      createdCount: 1,
      skippedDuplicates: 0,
      createdNoteIds: [card.noteId],
    });
    vi.mocked(ipc.getOrganizedCard).mockResolvedValue(card);
  });

  it("keeps tags below the two Basic sides and saves the selected deck", async () => {
    const onSaved = vi.fn();
    render(
      <CardEditorScreen
        deck={deck}
        decks={[deck, secondDeck]}
        card={card}
        onCancel={vi.fn()}
        onSaved={onSaved}
        onDirtyChange={vi.fn()}
      />,
    );

    expect(await screen.findByText("Medication")).toBeTruthy();
    const question = screen.getByText("Question editor");
    const answer = screen.getByText("Answer editor");
    const tagInput = screen.getByText("Tags");
    expect(question.compareDocumentPosition(answer) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(answer.compareDocumentPosition(tagInput) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    fireEvent.change(screen.getByRole("combobox", { name: "Deck" }), {
      target: { value: secondDeck.id },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save card" }));

    await waitFor(() => {
      expect(ipc.updateOrganizedCard).toHaveBeenCalledWith(
        expect.objectContaining({
          noteId: card.noteId,
          deckId: secondDeck.id,
          cardType: "basic",
          tags: ["Medication"],
        }),
      );
      expect(onSaved).toHaveBeenCalledWith(
        expect.objectContaining({ noteId: card.noteId }),
        false,
        expect.objectContaining({ source: "card" }),
      );
    });
  });

  it("uses a saved template as a bulk-entry shortcut for ordinary Basic cards", async () => {
    const onSaved = vi.fn();
    render(
      <CardEditorScreen
        deck={deck}
        decks={[deck]}
        card={null}
        onCancel={vi.fn()}
        onSaved={onSaved}
        onDirtyChange={vi.fn()}
      />,
    );

    fireEvent.change(await screen.findByRole("combobox", { name: "Card template" }), {
      target: { value: `template:${template.id}` },
    });
    fireEvent.change(screen.getByLabelText("name"), {
      target: { value: "lisinopril" },
    });
    fireEvent.change(screen.getByLabelText("drug class"), {
      target: { value: "ACE inhibitor" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create 1 card" }));

    await waitFor(() => {
      expect(ipc.generateCardsFromTemplate).toHaveBeenCalledWith({
        deckId: deck.id,
        templateId: template.id,
        values: [
          { fieldId: template.fields[0]?.id, value: "lisinopril" },
          { fieldId: template.fields[1]?.id, value: "ACE inhibitor" },
        ],
        tags: [],
      });
      expect(onSaved).toHaveBeenCalledWith(
        card,
        false,
        { source: "template", createdCount: 1, skippedDuplicates: 0 },
      );
    });
  });
});
