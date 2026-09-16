import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { RichTextEditorProps } from "../editor/RichTextEditor";
import * as assetsIpc from "../ipc/assets";
import * as cardsIpc from "../ipc/cards";
import * as decksIpc from "../ipc/decks";
import * as templatesIpc from "../ipc/templates";
import { CardEditorScreen } from "./CardEditorScreen";

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
  RichTextEditor: (props: RichTextEditorProps) => <div>{props.label} editor</div>,
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
  createOperationId: vi.fn(() => "0".repeat(32)),
  getErrorCode: () => null,
  getErrorMessage: (error: unknown) => error instanceof Error ? error.message : "Unexpected error",
}));
vi.mock("../ipc/decks", () => ({ listDecks: vi.fn() }));
vi.mock("../ipc/templates", () => ({
  generateCardsFromTemplate: vi.fn(), getCardTemplate: vi.fn(),
  listCardTemplateSummaries: vi.fn(), previewTemplateCards: vi.fn(),
}));

const deck: decksIpc.DeckSummary = {
  id: "a".repeat(32), name: "Interactive editor", description: "", sourceNoteCount: 0,
  generatedCardCount: 0, studyableDueCount: 0, archivedAt: null,
  createdAt: 1, updatedAt: 1, revision: 1,
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(assetsIpc.syncAssetDraft).mockResolvedValue(undefined);
  vi.mocked(assetsIpc.cancelAssetDraft).mockResolvedValue(undefined);
  vi.mocked(decksIpc.listDecks).mockResolvedValue([deck]);
  vi.mocked(cardsIpc.listTags).mockResolvedValue([]);
  vi.mocked(cardsIpc.findDuplicateCards).mockResolvedValue([]);
  vi.mocked(templatesIpc.listCardTemplateSummaries).mockResolvedValue([]);
});

describe("interactive question editor entry point", () => {
  it("opens dedicated Grouping and Ordering editors from the existing Template selector", async () => {
    render(<CardEditorScreen deck={deck} decks={[deck]} card={null}
      onCancel={vi.fn()} onSaved={vi.fn()} onTemplateGenerated={vi.fn()} onDirtyChange={vi.fn()} />);

    const template = await screen.findByRole("combobox", { name: "Card template" });
    expect(screen.getByRole("option", { name: "Grouping" })).toBeTruthy();
    expect(screen.getByRole("option", { name: "Ordering" })).toBeTruthy();

    fireEvent.change(template, { target: { value: "grouping" } });
    expect(await screen.findByRole("heading", { name: "Grouping" })).toBeTruthy();
    expect(screen.getByText("Category 1")).toBeTruthy();
    expect(screen.getByText("Item 1")).toBeTruthy();
    expect(screen.getAllByText("Correct category")).toHaveLength(2);

    fireEvent.change(template, { target: { value: "ordering" } });
    expect(await screen.findByRole("heading", { name: "Ordering" })).toBeTruthy();
    expect(screen.getByText("Position 1")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Move up" }).length).toBeGreaterThan(0);
    expect(screen.getAllByRole("button", { name: "Move down" }).length).toBeGreaterThan(0);
  });
});
