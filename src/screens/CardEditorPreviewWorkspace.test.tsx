import type { ComponentProps } from "react";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { RichDocument } from "../ipc/common";
import type { DeckSummary } from "../ipc/decks";
import type { BasicCardFields } from "./card-editor/BasicCardFields";
import type { TemplateEntryForm } from "./card-editor/TemplateEntryForm";
import { CardEditorScreen } from "./CardEditorScreenBase";

const mocks = vi.hoisted(() => ({
  back: null as (() => boolean) | null,
  topId: null as string | null,
  save: vi.fn(), discard: vi.fn(), prepare: vi.fn(), confirm: vi.fn(),
  previewRecall: vi.fn(), templateSummaries: vi.fn(), getTemplate: vi.fn(), previewTemplate: vi.fn(),
}));
vi.mock("../platform/backNavigation", () => ({
  registerPlatformBackHandler: (handler: () => boolean) => {
    mocks.back = handler;
    return () => { mocks.back = null; };
  },
}));
vi.mock("../ui/overlays/overlayContext", () => ({
  useOverlayContext: () => ({ topId: mocks.topId }), useConfirm: () => mocks.confirm,
}));
vi.mock("../ui/appearance/AppearanceProvider", () => ({ useAppearance: () => ({ motion: "reduced" }) }));
vi.mock("../controllers/useDurableDraft", () => ({
  useDurableDraft: () => ({
    assetDraftId: "7".repeat(32), hasUnsavedLocalChanges: true,
    flush: vi.fn().mockResolvedValue(undefined), discard: mocks.discard,
    prepareCanonicalCommit: mocks.prepare, acceptCanonicalCommit: vi.fn(), restart: vi.fn(),
  }),
}));
vi.mock("../ipc/assets", () => ({
  beginAssetDraft: vi.fn(), cancelAssetDraft: vi.fn(), syncAssetDraft: vi.fn().mockResolvedValue(undefined),
}));
vi.mock("../ipc/cards", () => ({
  createCardNote: mocks.save, updateCardNote: mocks.save, getCardNote: vi.fn(),
  previewCardDraft: mocks.previewRecall,
  findDuplicateCards: vi.fn().mockResolvedValue([]), listTags: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/decks", () => ({ listDecks: vi.fn().mockResolvedValue([]) }));
vi.mock("../ipc/templates", () => ({
  generateCardsFromTemplate: mocks.save, getCardTemplate: mocks.getTemplate,
  listCardTemplateSummaries: mocks.templateSummaries, previewTemplateCards: mocks.previewTemplate,
}));
vi.mock("./card-editor/TemplateEntryForm", () => ({
  // Like the real form, do not expose a field before its value has initialized.
  TemplateEntryForm: ({ values, disabled, onValueChange }: ComponentProps<typeof TemplateEntryForm>) => <>
    {values.map((value) => <label key={value.fieldId}>Template value<input
      disabled={disabled} value={value.text} onChange={(event) =>
        onValueChange(value.fieldId, textDoc(event.target.value), event.target.value)} /></label>)}
  </>,
}));
vi.mock("./card-editor/BasicCardFields", () => ({
  BasicCardFields: (props: ComponentProps<typeof BasicCardFields>) => <>
    <label>Question draft<input onChange={(event) => props.onFieldChange("front", textDoc(event.target.value), event.target.value)} /></label>
    <label>Answer draft<input onChange={(event) => props.onFieldChange("back", textDoc(event.target.value), event.target.value)} /></label>
    <button onClick={() => props.onReverseChange(true)}>Enable reverse</button>
  </>,
}));
vi.mock("../editor/RichTextView", () => ({
  RichTextView: ({ document }: { document: RichDocument }) => <div>{JSON.stringify(document)}</div>,
}));

function textDoc(text: string): RichDocument {
  return { type: "doc", attrs: { schemaVersion: 3 }, content: [{ type: "paragraph", content: [{ type: "text", text }] }] };
}
const deck: DeckSummary = {
  id: "a".repeat(32), name: "Preview deck", description: "", sourceNoteCount: 0,
  generatedCardCount: 0, studyableDueCount: 0, archivedAt: null,
  createdAt: 1, updatedAt: 1, revision: 1,
};
function mount() {
  const onCancel = vi.fn();
  render(<CardEditorScreen deck={deck} decks={[deck]} card={null} onCancel={onCancel}
    onSaved={vi.fn()} onTemplateGenerated={vi.fn()} onDirtyChange={vi.fn()} />);
  return { onCancel };
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.topId = null;
  mocks.templateSummaries.mockResolvedValue([{
    id: "t".repeat(32), name: "Reusable template", description: "", fieldCount: 1,
    generatedCardRuleCount: 2, imageCount: 0, updatedAt: 1, revision: 1,
  }]);
  mocks.getTemplate.mockResolvedValue({
    id: "t".repeat(32), name: "Reusable template", description: "",
    fields: [{ id: "f".repeat(32), name: "Term", description: "", position: 0 }],
    rules: [
      { id: "1".repeat(32), name: "Forward rule", frontDoc: textDoc("{{Term}}"), backDoc: textDoc("Answer"), position: 0 },
      { id: "2".repeat(32), name: "Reverse rule", frontDoc: textDoc("Answer"), backDoc: textDoc("{{Term}}"), position: 1 },
    ],
    assets: [], createdAt: 1, updatedAt: 1, revision: 1,
  });
  mocks.previewRecall.mockImplementation(async (input: {
    cardType: "basic" | "cloze"; reverseEnabled?: boolean; frontDoc?: RichDocument; backDoc?: RichDocument;
  }) => input.cardType === "basic" ? [
    {
      presentationKey: "forward", presentationLabel: "Forward", cardLabel: "Basic",
      frontDoc: input.frontDoc!, backDoc: input.backDoc!, frontText: "Forward question", backText: "Forward answer",
    },
    ...(input.reverseEnabled ? [{
      presentationKey: "reverse", presentationLabel: "Reverse", cardLabel: "Reverse",
      frontDoc: input.backDoc!, backDoc: input.frontDoc!, frontText: "Forward answer", backText: "Forward question",
    }] : []),
  ] : [
    {
      presentationKey: "cloze:1", presentationLabel: "Blank group 1", cardLabel: "Fill in the blank",
      frontDoc: textDoc("First cloze"), backDoc: textDoc("First answer"), frontText: "First cloze", backText: "First answer",
    },
    {
      presentationKey: "cloze:2", presentationLabel: "Blank group 2", cardLabel: "Fill in the blank",
      frontDoc: textDoc("Second cloze"), backDoc: textDoc("Second answer"), frontText: "Second cloze", backText: "Second answer",
    },
  ]);
  mocks.previewTemplate.mockResolvedValue([
    {
      ruleId: "1".repeat(32), ruleName: "Forward rule", questionDoc: textDoc("Generated forward"),
      answerDoc: textDoc("Generated answer"), questionText: "Generated forward", answerText: "Generated answer", assets: [],
    },
    {
      ruleId: "2".repeat(32), ruleName: "Reverse rule", questionDoc: textDoc("Generated reverse"),
      answerDoc: textDoc("Generated term"), questionText: "Generated reverse", answerText: "Generated term", assets: [],
    },
  ]);
});

describe("full-size editor preview workspace", () => {
  it("keeps the unsaved editor mounted, restores field selection, and never saves or discards from preview", async () => {
    const { onCancel } = mount();
    const field = await screen.findByRole("textbox", { name: "Question draft" }) as HTMLInputElement;
    fireEvent.change(field, { target: { value: "Unsaved question" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Answer draft" }), { target: { value: "Unsaved answer" } });
    field.focus(); field.setSelectionRange(2, 7, "backward"); fireEvent.blur(field);
    fireEvent.click(screen.getByRole("button", { name: "Preview card" }));
    const preview = screen.getByRole("region", { name: "Card preview" });
    expect(preview.querySelector("aside")).toBeNull();
    expect(screen.queryByRole("textbox", { name: "Question draft" })).toBeNull();
    expect(field.isConnected).toBe(true);
    expect(await within(preview).findByText(/Unsaved question/u)).toBeTruthy();
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    fireEvent.click(within(preview).getByRole("button", { name: "Reveal answer" }));
    expect(await within(preview).findByText(/Unsaved answer/u)).toBeTruthy();
    fireEvent.click(within(preview).getByRole("button", { name: "Close preview" }));
    expect(screen.getByRole("textbox", { name: "Question draft" })).toBe(field);
    expect(document.activeElement).toBe(field);
    expect([field.selectionStart, field.selectionEnd, field.selectionDirection]).toEqual([2, 7, "backward"]);
    expect(mocks.save).not.toHaveBeenCalled();
    expect(mocks.prepare).not.toHaveBeenCalled();
    expect(mocks.discard).not.toHaveBeenCalled();
    expect(mocks.confirm).not.toHaveBeenCalled();
    expect(onCancel).not.toHaveBeenCalled();
  });

  it("uses the reverse content in the same recall surface and closes on native Back without leaving the editor", async () => {
    const { onCancel } = mount();
    fireEvent.change(await screen.findByRole("textbox", { name: "Question draft" }), { target: { value: "Forward question" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Answer draft" }), { target: { value: "Forward answer" } });
    fireEvent.click(screen.getByRole("button", { name: "Enable reverse" }));
    fireEvent.click(screen.getByRole("button", { name: "Preview card" }));
    const preview = screen.getByRole("region", { name: "Card preview" });
    fireEvent.change(await within(preview).findByRole("combobox", { name: "Presentation" }), { target: { value: "reverse" } });
    expect(within(preview).getByText(/Forward answer/u)).toBeTruthy();
    act(() => { expect(mocks.back?.()).toBe(true); });
    expect(screen.queryByRole("region", { name: "Card preview" })).toBeNull();
    expect(onCancel).not.toHaveBeenCalled();
    expect(mocks.discard).not.toHaveBeenCalled();
  });

  it("previews each cloze group in the full workspace from the native recall projection", async () => {
    mount();
    const selector = await screen.findByRole("combobox", { name: "Card template" });
    fireEvent.change(selector, { target: { value: "cloze" } });
    const source = await screen.findByRole("textbox", { name: "Complete statement" });
    fireEvent.change(source, { target: { value: "The {{c1::heart}} moves {{c2::blood}}." } });
    fireEvent.click(screen.getByRole("button", { name: "Preview card" }));
    const preview = screen.getByRole("region", { name: "Card preview" });
    const presentations = await within(preview).findByRole("combobox", { name: "Presentation" });
    expect(within(preview).getByText(/First cloze/u)).toBeTruthy();
    fireEvent.change(presentations, { target: { value: "cloze:2" } });
    expect(within(preview).getByText(/Second cloze/u)).toBeTruthy();
    expect(mocks.previewRecall).toHaveBeenCalledWith(expect.objectContaining({
      cardType: "cloze",
      clozeSource: "The {{c1::heart}} moves {{c2::blood}}.",
    }));
  });

  it("opens reusable-template outputs in the same full Practice-style preview", async () => {
    mount();
    const selector = await screen.findByRole("combobox", { name: "Card template" });
    await screen.findByRole("option", { name: "Reusable template" });
    fireEvent.change(selector, { target: { value: "t".repeat(32) } });
    const value = await screen.findByRole("textbox", { name: "Template value" });
    fireEvent.change(value, { target: { value: "alveolus" } });
    fireEvent.click(screen.getByRole("button", { name: "Preview card" }));
    const preview = screen.getByRole("region", { name: "Card preview" });
    const presentations = await within(preview).findByRole("combobox", { name: "Presentation" });
    expect(within(preview).getByText(/Generated forward/u)).toBeTruthy();
    fireEvent.change(presentations, { target: { value: `template:${"2".repeat(32)}` } });
    expect(within(preview).getByText(/Generated reverse/u)).toBeTruthy();
    expect(mocks.previewTemplate).toHaveBeenCalledTimes(1);
    expect(mocks.previewTemplate).toHaveBeenCalledWith(expect.objectContaining({
      values: [{ fieldId: "f".repeat(32), doc: textDoc("alveolus"), text: "alveolus" }],
    }));
  });

  it("does not consume Escape from an overlay or input composition", async () => {
    mount();
    await screen.findByRole("textbox", { name: "Question draft" });
    mocks.topId = "open-sheet";
    fireEvent.click(screen.getByRole("button", { name: "Preview card" }));
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.getByRole("region", { name: "Card preview" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Close preview" }));
    mocks.topId = null;
    await waitFor(() => expect(
      (screen.getByRole("button", { name: "Preview card" }) as HTMLButtonElement).disabled,
    ).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "Preview card" }));
    fireEvent.keyDown(window, { key: "Escape", isComposing: true });
    expect(screen.getByRole("region", { name: "Card preview" })).toBeTruthy();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("region", { name: "Card preview" })).toBeNull();
  });
});
