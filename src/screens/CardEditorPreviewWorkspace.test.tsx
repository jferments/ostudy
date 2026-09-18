import type { ComponentProps } from "react";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { RichDocument } from "../ipc/common";
import type { DeckSummary } from "../ipc/decks";
import type { BasicCardFields } from "./card-editor/BasicCardFields";
import { CardEditorScreen } from "./CardEditorScreenBase";

const mocks = vi.hoisted(() => ({
  back: null as (() => boolean) | null,
  topId: null as string | null,
  save: vi.fn(), discard: vi.fn(), prepare: vi.fn(), confirm: vi.fn(),
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
  findDuplicateCards: vi.fn().mockResolvedValue([]), listTags: vi.fn().mockResolvedValue([]),
}));
vi.mock("../ipc/decks", () => ({ listDecks: vi.fn().mockResolvedValue([]) }));
vi.mock("../ipc/templates", () => ({
  generateCardsFromTemplate: mocks.save, getCardTemplate: vi.fn(),
  listCardTemplateSummaries: vi.fn().mockResolvedValue([]), previewTemplateCards: vi.fn(),
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

beforeEach(() => { vi.clearAllMocks(); mocks.topId = null; });

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
    expect(within(preview).getByText(/Unsaved question/u)).toBeTruthy();
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    fireEvent.click(within(preview).getByRole("button", { name: "Reveal answer" }));
    expect(within(preview).getByText(/Unsaved answer/u)).toBeTruthy();
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
    fireEvent.change(within(preview).getByRole("combobox", { name: "Presentation" }), { target: { value: "reverse" } });
    expect(within(preview).getByText(/Forward answer/u)).toBeTruthy();
    act(() => { expect(mocks.back?.()).toBe(true); });
    expect(screen.queryByRole("region", { name: "Card preview" })).toBeNull();
    expect(onCancel).not.toHaveBeenCalled();
    expect(mocks.discard).not.toHaveBeenCalled();
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
    fireEvent.click(screen.getByRole("button", { name: "Preview card" }));
    fireEvent.keyDown(window, { key: "Escape", isComposing: true });
    expect(screen.getByRole("region", { name: "Card preview" })).toBeTruthy();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.queryByRole("region", { name: "Card preview" })).toBeNull();
  });
});
