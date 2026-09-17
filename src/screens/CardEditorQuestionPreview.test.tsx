import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { DeckSummary } from "../ipc/decks";
import { CardEditorScreen } from "./CardEditorScreen";

vi.mock("../controllers/useDurableDraft", () => ({
  useDurableDraft: () => ({
    draftId: "d".repeat(32), assetDraftId: "7".repeat(32), revision: 1,
    status: "saved", errorMessage: null, recovered: false, hasUnsavedLocalChanges: false,
    flush: vi.fn().mockResolvedValue(undefined), discard: vi.fn().mockResolvedValue(undefined),
    prepareCanonicalCommit: vi.fn().mockResolvedValue({ draftId: "d".repeat(32), revision: 1 }),
    acceptCanonicalCommit: vi.fn(), restart: vi.fn().mockResolvedValue(undefined),
  }),
}));
vi.mock("./card-editor/InteractiveQuestionEditor", () => ({
  InteractiveQuestionEditor: ({ draft }: { draft: { kind: string } }) => <div>Editor {draft.kind}</div>,
}));
vi.mock("./card-editor/QuestionDraftPreview", () => ({
  QuestionDraftPreview: ({ draft }: { draft: { kind: string } }) => <aside>Preview {draft.kind}</aside>,
}));
vi.mock("../ipc/assets", () => ({ beginAssetDraft: vi.fn(), cancelAssetDraft: vi.fn(), syncAssetDraft: vi.fn().mockResolvedValue(undefined) }));
vi.mock("../ipc/cards", () => ({
  createCardNote: vi.fn(), findDuplicateCards: vi.fn().mockResolvedValue([]), getCardNote: vi.fn(),
  listTags: vi.fn().mockResolvedValue([]), updateCardNote: vi.fn(),
}));
vi.mock("../ipc/decks", () => ({ listDecks: vi.fn().mockResolvedValue([]) }));
vi.mock("../ipc/templates", () => ({
  generateCardsFromTemplate: vi.fn(), getCardTemplate: vi.fn(),
  listCardTemplateSummaries: vi.fn().mockResolvedValue([]), previewTemplateCards: vi.fn(),
}));
vi.mock("../ipc/questions", () => ({ saveQuestionNote: vi.fn(), getQuestionNote: vi.fn() }));

const deck: DeckSummary = {
  id: "a".repeat(32), name: "Preview deck", description: "", sourceNoteCount: 0,
  generatedCardCount: 0, studyableDueCount: 0, archivedAt: null, createdAt: 1, updatedAt: 1, revision: 1,
};

beforeEach(() => vi.clearAllMocks());

describe("interactive editor card preview entry point", () => {
  it("offers the same Preview card action for every interactive question kind", async () => {
    render(<CardEditorScreen deck={deck} decks={[deck]} card={null} onCancel={vi.fn()} onSaved={vi.fn()}
      onTemplateGenerated={vi.fn()} onDirtyChange={vi.fn()} />);
    const selector = await screen.findByRole("combobox", { name: "Card template" });
    for (const kind of ["single_choice", "select_all", "true_false", "matching", "grouping", "ordering", "image_occlusion"]) {
      fireEvent.change(selector, { target: { value: kind } });
      await waitFor(() => expect(screen.getByText(`Editor ${kind}`)).toBeTruthy());
      const preview = screen.getByRole("button", { name: "Preview card" });
      fireEvent.click(preview);
      expect(await screen.findByText(`Preview ${kind}`)).toBeTruthy();
      fireEvent.click(screen.getByRole("button", { name: "Hide preview" }));
      expect(screen.queryByText(`Preview ${kind}`)).toBeNull();
    }
  });
});
