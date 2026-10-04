import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { requiredTestValue as required } from "../test/factories";
import type { RichTextEditorProps } from "../editor/RichTextEditor";
import { extractRichDocumentText } from "../editor/richText";
import type { SaveQuestionNoteInput, QuestionDefinition, QuestionNote } from "../ipc/questions";
import type { DeckSummary } from "../ipc/decks";
import { Textarea } from "../ui/primitives/FormControls";
import { CardEditorScreen } from "./CardEditorScreen";

const mocks = vi.hoisted(() => ({
  save: vi.fn(), pick: vi.fn(), prepare: vi.fn(), flush: vi.fn(), restart: vi.fn(),
  accepted: vi.fn(), saved: vi.fn(), payload: vi.fn(), sync: vi.fn(),
}));
vi.mock("../controllers/useDurableDraft", () => ({
  useDurableDraft: (options: { payload: unknown }) => {
    mocks.payload(options.payload);
    return {
      draftId: "d".repeat(32), assetDraftId: "7".repeat(32), revision: 1,
      status: "saved", errorMessage: null, recovered: false, hasUnsavedLocalChanges: false,
      flush: mocks.flush, discard: vi.fn().mockResolvedValue(undefined),
      prepareCanonicalCommit: vi.fn().mockResolvedValue({ draftId: "d".repeat(32), revision: 1 }),
      acceptCanonicalCommit: mocks.accepted, restart: mocks.restart,
    };
  },
}));
vi.mock("../editor/RichTextEditor", () => ({
  RichTextEditor: (props: RichTextEditorProps) => <Textarea aria-label={props.label}
    disabled={props.disabled} value={extractRichDocumentText(props.document)}
    onChange={(event) => {
      const text = event.currentTarget.value;
      props.onChange({ type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text }] }] }, text);
    }} />,
}));
vi.mock("../editor/RichTextView", () => ({ RichTextView: () => <span>Rich preview</span> }));
vi.mock("../platform/assets", () => ({ managedAssetUrl: (id: string) => `blob:managed-${id}` }));
vi.mock("../ipc/externalFiles", () => ({ selectAndStageExternalImport: mocks.pick }));
vi.mock("../ipc/occlusion", () => ({ prepareOcclusionImage: mocks.prepare, previewOcclusionImage: vi.fn() }));
vi.mock("../ipc/questions", () => ({ saveQuestionNote: mocks.save, getQuestionNote: vi.fn() }));
vi.mock("../ipc/assets", () => ({ beginAssetDraft: vi.fn(), cancelAssetDraft: vi.fn(), syncAssetDraft: mocks.sync }));
vi.mock("../ipc/cards", () => ({
  createCardNote: vi.fn(), findDuplicateCards: vi.fn().mockResolvedValue([]), getCardNote: vi.fn(),
  listTags: vi.fn().mockResolvedValue([]), updateCardNote: vi.fn(),
}));
vi.mock("../ipc/decks", () => ({ listDecks: vi.fn().mockResolvedValue([]) }));
vi.mock("../ipc/templates", () => ({
  generateCardsFromTemplate: vi.fn(), getCardTemplate: vi.fn(),
  listCardTemplateSummaries: vi.fn().mockResolvedValue([]), previewTemplateCards: vi.fn(),
}));

const deck: DeckSummary = {
  id: "a".repeat(32), name: "Image editor", description: "", sourceNoteCount: 0,
  generatedCardCount: 0, studyableDueCount: 0, archivedAt: null, createdAt: 1, updatedAt: 1, revision: 1,
};
const asset = { id: "b".repeat(32), originalFilename: "occlusion-source.png", mimeType: "image/png", byteSize: 200, createdAt: 1 };
function editor(card: QuestionNote | null = null) {
  return render(<CardEditorScreen deck={deck} decks={[deck]} card={card}
    onCancel={vi.fn()} onSaved={vi.fn()} onQuestionSaved={mocks.saved}
    onTemplateGenerated={vi.fn()} onDirtyChange={vi.fn()} />);
}
async function createRegion() {
  fireEvent.change(await screen.findByRole("combobox", { name: "Card template" }), { target: { value: "image_occlusion" } });
  fireEvent.click(await screen.findByRole("button", { name: "Choose image" }));
  fireEvent.click(await screen.findByRole("button", { name: "Add rectangle" }));
  expect(screen.getByRole("button", { name: "Save question" })).toHaveProperty("disabled", false);
  expect(screen.queryByRole("textbox", { name: "Answer notes (optional)" })).toBeNull();
}
async function openDetails() {
  fireEvent.click(await screen.findByRole("button", { name: "Optional question details" }));
  return screen.findByRole("textbox", { name: "Answer notes (optional)" });
}
function savedDefinition(): QuestionDefinition {
  return JSON.parse(mocks.save.mock.calls.at(-1)?.[0].definitionJson) as QuestionDefinition;
}

beforeEach(() => {
  for (const mock of Object.values(mocks)) mock.mockReset();
  mocks.flush.mockResolvedValue(undefined);
  mocks.restart.mockResolvedValue(undefined);
  mocks.sync.mockResolvedValue(undefined);
  mocks.saved.mockResolvedValue(undefined);
  mocks.pick.mockResolvedValue({ id: "c".repeat(32) });
  mocks.prepare.mockResolvedValue({ asset, width: 600, height: 400, pixelFingerprint: "f".repeat(64) });
  mocks.save.mockImplementation(async (input: SaveQuestionNoteInput) => ({
    operationId: input.operationId, noteId: input.noteId ?? "e".repeat(32),
    noteRevision: (input.revision ?? 0) + 1, cardIds: ["8".repeat(32)], committedAt: 1000,
  }));
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
  vi.stubGlobal("Image", class {
    naturalWidth = 600;
    naturalHeight = 400;
    onload: (() => void) | null = null;
    onerror: (() => void) | null = null;
    set src(value: string) { if (value) queueMicrotask(() => this.onload?.()); }
  });
});
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

describe("image occlusion in the shared Create/Edit Card workflow", () => {
  it("creates a real region, retains the per-type draft and saves only canonical authored content", async () => {
    editor();
    await createRegion();
    fireEvent.change(await openDetails(), { target: { value: "A labeled structure" } });
    const selector = screen.getByRole("combobox", { name: "Card template" });
    fireEvent.change(selector, { target: { value: "basic" } });
    fireEvent.change(selector, { target: { value: "image_occlusion" } });
    expect(await openDetails()).toHaveProperty("value", "A labeled structure");
    expect(screen.getByText("1 regions · 1 study cards")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Save question" }));
    await waitFor(() => expect(mocks.saved).toHaveBeenCalledExactlyOnceWith("e".repeat(32), false));
    expect(mocks.accepted).toHaveBeenCalledTimes(1);
    const input = required(mocks.save.mock.calls[0]?.[0], "save question input");
    expect(input.assetDraftId).toBe("7".repeat(32));
    expect(input.editorDraft).toEqual({ draftId: "d".repeat(32), revision: 1 });
    const definition = savedDefinition();
    if (definition.question.kind !== "image_occlusion") throw new Error("Expected image definition");
    expect(definition.question.imageAssetId).toBe(asset.id);
    expect(definition.question.masks).toHaveLength(1);
    expect(required(definition.question.questions[0], "saved occlusion question").maskIds).toEqual([required(definition.question.masks[0], "saved occlusion mask").id]);
    expect(extractRichDocumentText(required(definition.question.questions[0], "saved occlusion question").answer)).toBe("A labeled structure");
    expect(Object.keys(definition.question).sort()).toEqual(["imageAssetId", "kind", "masks", "questions"]);
    expect(mocks.flush).toHaveBeenCalledTimes(2);
  });

  it("reopens an existing note, adjusts a mask and preserves its question identity", async () => {
    const note: QuestionNote = {
      noteId: "e".repeat(32), deckId: deck.id, assets: [asset], tags: [], cardIds: ["8".repeat(32)],
      createdAt: 1, updatedAt: 1, revision: 4,
      definition: { schemaVersion: 1, question: {
        kind: "image_occlusion", imageAssetId: asset.id,
        masks: [{ id: "1".repeat(32), rect: { x: 0.2, y: 0.2, width: 0.2, height: 0.2 } }],
        questions: [{ id: "2".repeat(32), maskIds: ["1".repeat(32)], mode: "hide_target",
          prompt: { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: "Identify" }] }] },
          answer: { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: "Saved answer" }] }] }, explanation: null }],
      } },
    };
    editor(note);
    expect(await openDetails()).toHaveProperty("value", "Saved answer");
    expect(screen.getByRole("combobox", { name: "Card template" })).toHaveProperty("disabled", true);
    fireEvent.click(screen.getByRole("button", { name: "Move right" }));
    fireEvent.click(screen.getByRole("button", { name: "Save question" }));
    await waitFor(() => expect(mocks.saved).toHaveBeenCalledTimes(1));
    const definition = savedDefinition();
    if (definition.question.kind !== "image_occlusion") throw new Error("Expected image definition");
    expect(definition.question.questions).toEqual(note.definition.question.kind === "image_occlusion" ? note.definition.question.questions : []);
    expect(required(definition.question.masks[0], "saved occlusion mask").id).toBe("1".repeat(32));
    expect(required(definition.question.masks[0], "saved occlusion mask").rect.x).toBeGreaterThan(0.2);
    expect(required(mocks.save.mock.calls[0]?.[0], "save question input")).toMatchObject({ noteId: note.noteId, revision: 4 });
    expect(mocks.pick).not.toHaveBeenCalled();
  });

  it("Save + another accepts a rectangle-only card and creates an empty next draft", async () => {
    editor();
    await createRegion();
    fireEvent.click(screen.getByRole("button", { name: "Save + another" }));
    await waitFor(() => expect(mocks.saved).toHaveBeenCalledExactlyOnceWith("e".repeat(32), true));
    const definition = savedDefinition();
    if (definition.question.kind !== "image_occlusion") throw new Error("Expected image definition");
    expect(extractRichDocumentText(required(definition.question.questions[0], "image answer").answer))
      .toBe("Check your recall against the uncovered image.");
    expect(await screen.findByRole("button", { name: "Choose image" })).toBeTruthy();
    expect(screen.queryByRole("textbox", { name: "Answer notes (optional)" })).toBeNull();
    expect(screen.getByText("0 regions · 0 study cards")).toBeTruthy();
    expect(mocks.restart).toHaveBeenCalledTimes(1);
    expect(required(mocks.restart.mock.calls[0]?.[0], "restart input").questionDrafts.image_occlusion).toMatchObject({
      imageAssetId: null, masks: [], questions: [], pendingImport: null,
    });
  });
});
