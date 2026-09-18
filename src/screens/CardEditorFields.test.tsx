import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { RichTextEditorProps } from "../editor/RichTextEditor";
import type { RichDocument } from "../ipc/common";
import * as assetsIpc from "../ipc/assets";
import * as cardsIpc from "../ipc/cards";
import * as decksIpc from "../ipc/decks";
import * as templatesIpc from "../ipc/templates";
import { CardEditorScreen } from "./CardEditorScreen";

const editorState = vi.hoisted(() => ({ fields: new Map<string, RichTextEditorProps>() }));

vi.mock("../controllers/useDurableDraft", () => ({
  useDurableDraft: () => ({
    draftId: "d".repeat(32), assetDraftId: "7".repeat(32), revision: 1,
    status: "saved", errorMessage: null, recovered: false, hasUnsavedLocalChanges: false,
    flush: vi.fn().mockResolvedValue(undefined), discard: vi.fn().mockResolvedValue(undefined),
    prepareCanonicalCommit: vi.fn().mockResolvedValue({ draftId: "d".repeat(32), revision: 1 }),
    acceptCanonicalCommit: vi.fn(), commit: vi.fn().mockResolvedValue(undefined),
    restart: vi.fn().mockResolvedValue(undefined),
  }),
}));
vi.mock("../editor/RichTextEditor", () => ({
  RichTextEditor: (props: RichTextEditorProps) => {
    editorState.fields.set(props.label, props);
    return <div>{props.label} editor</div>;
  },
}));
vi.mock("../editor/RichTextView", () => ({ RichTextView: () => <div>Preview</div> }));
vi.mock("../ipc/assets", () => ({
  beginAssetDraft: vi.fn(), cancelAssetDraft: vi.fn(), syncAssetDraft: vi.fn(),
}));
vi.mock("../ipc/cards", () => ({
  createCardNote: vi.fn(), findDuplicateCards: vi.fn(), getCardNote: vi.fn(),
  listTags: vi.fn(), updateCardNote: vi.fn(),
}));
vi.mock("../ipc/common", () => ({
  getErrorCode: () => null,
  getErrorMessage: (error: unknown) => error instanceof Error ? error.message : "Unexpected error",
}));
vi.mock("../ipc/decks", () => ({ listDecks: vi.fn() }));
vi.mock("../ipc/templates", () => ({
  generateCardsFromTemplate: vi.fn(), getCardTemplate: vi.fn(),
  listCardTemplateSummaries: vi.fn(), previewTemplateCards: vi.fn(),
}));

function textDoc(text: string): RichDocument {
  return { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] };
}

const deck: decksIpc.DeckSummary = {
  id: "a".repeat(32), name: "Field regression", description: "", sourceNoteCount: 1,
  generatedCardCount: 1, studyableDueCount: 1, archivedAt: null,
  createdAt: 1, updatedAt: 1, revision: 1,
};
const audio: assetsIpc.Asset = {
  id: "b".repeat(32), mimeType: "audio/mpeg", originalFilename: "local.mp3",
  byteSize: 100, createdAt: 1,
};
const note: cardsIpc.CardNote = {
  cardType: "basic", noteId: "c".repeat(32), deckId: deck.id,
  frontDoc: textDoc("Question"), backDoc: textDoc("Answer"),
  frontText: "Question", backText: "Answer", reverseEnabled: false, assets: [audio], tags: [],
  generatedCardCount: 1, createdAt: 1, updatedAt: 1, revision: 1,
};
const template: templatesIpc.CardTemplate = {
  id: "e".repeat(32), name: "Pairs template", description: "", revision: 1,
  fields: [
    { id: "f".repeat(32), name: "Term", description: "", position: 0 },
    { id: "8".repeat(32), name: "Definition", description: "", position: 1 },
  ],
  rules: [{ id: "9".repeat(32), name: "Forward", position: 0,
    frontDoc: textDoc("Front"), backDoc: textDoc("Back") }],
  assets: [], createdAt: 1, updatedAt: 1,
};

function field(label: string): RichTextEditorProps {
  const result = editorState.fields.get(label);
  if (!result) throw new Error(`Missing test editor: ${label}`);
  return result;
}

function openEditor(card: cardsIpc.CardNote | null) {
  return render(<CardEditorScreen deck={deck} decks={[deck]} card={card}
    onCancel={vi.fn()} onSaved={vi.fn()} onTemplateGenerated={vi.fn()} onDirtyChange={vi.fn()} />);
}

beforeEach(() => {
  vi.clearAllMocks();
  editorState.fields.clear();
  vi.mocked(assetsIpc.syncAssetDraft).mockResolvedValue(undefined);
  vi.mocked(assetsIpc.cancelAssetDraft).mockResolvedValue(undefined);
  vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);
  vi.mocked(cardsIpc.listTags).mockResolvedValue([]);
  vi.mocked(cardsIpc.findDuplicateCards).mockResolvedValue([]);
  vi.mocked(cardsIpc.updateCardNote).mockResolvedValue(note);
  vi.mocked(templatesIpc.listCardTemplateSummaries).mockResolvedValue([]);
  vi.mocked(templatesIpc.getCardTemplate).mockResolvedValue(template);
  vi.mocked(templatesIpc.generateCardsFromTemplate).mockResolvedValue({
    createdCount: 1, skippedDuplicates: 0, createdNoteIds: [note.noteId],
  });
});

describe("shared authoring fields through the card editor", () => {
  it("keeps Basic math/media documents, reverse policy and the draft on the existing save boundary", async () => {
    const math: RichDocument = { type: "doc", attrs: { schemaVersion: 3 }, content: [
      { type: "paragraph", content: [{ type: "inlineMath", attrs: { latex: "x^2" } }] },
    ] };
    const media: RichDocument = { type: "doc", attrs: { schemaVersion: 3 }, content: [
      { type: "audio", attrs: { assetId: audio.id, src: `study-asset://${audio.id}` } },
    ] };
    openEditor(note);
    await screen.findByText("Question editor");
    act(() => {
      field("Question").onChange(math, "x squared");
      field("Answer").onChange(media, "[Audio]");
    });
    fireEvent.click(screen.getByRole("checkbox", { name: /Create reverse card/u }));
    fireEvent.click(screen.getByRole("button", { name: "Save card" }));
    await waitFor(() => expect(cardsIpc.updateCardNote).toHaveBeenCalledOnce());
    expect(cardsIpc.updateCardNote).toHaveBeenCalledWith(expect.objectContaining({
      cardType: "basic", noteId: note.noteId, revision: note.revision, deckId: deck.id,
      frontDoc: math, backDoc: media, reverseEnabled: true, assetDraftId: "7".repeat(32),
      editorDraft: { draftId: "d".repeat(32), revision: 1 },
    }));
    expect(field("Question").assets).toContainEqual(audio);
  });

  it("preserves both template fields when several deltas arrive before a rerender", async () => {
    vi.mocked(templatesIpc.listCardTemplateSummaries).mockResolvedValue([{
      id: template.id, name: template.name, description: "", fieldCount: 2,
      generatedCardRuleCount: 1, imageCount: 0, updatedAt: 1, revision: 1,
    }]);
    openEditor(null);
    await screen.findByRole("option", { name: template.name });
    fireEvent.change(screen.getByRole("combobox", { name: "Card template" }), {
      target: { value: template.id },
    });
    await screen.findByText("Term editor");
    await screen.findByText("Definition editor");
    const term = field("Term");
    const definition = field("Definition");
    const finalTerm = textDoc("Updated term");
    const finalDefinition = textDoc("Kept definition");
    act(() => {
      term.onChange(textDoc("First term"), "First term");
      definition.onChange(finalDefinition, "Kept definition");
      term.onChange(finalTerm, "Updated term");
    });
    fireEvent.click(screen.getByRole("button", { name: "Generate 1 card" }));
    await waitFor(() => expect(templatesIpc.generateCardsFromTemplate).toHaveBeenCalledOnce());
    expect(templatesIpc.generateCardsFromTemplate).toHaveBeenCalledWith(expect.objectContaining({
      templateId: template.id, templateRevision: template.revision, deckId: deck.id,
      values: [
        { fieldId: "f".repeat(32), doc: finalTerm, text: "Updated term" },
        { fieldId: "8".repeat(32), doc: finalDefinition, text: "Kept definition" },
      ],
      editorDraft: { draftId: "d".repeat(32), revision: 1 },
    }));
  });
});
