import { act, render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as cardsIpc from "../ipc/cards";
import type { DeckSummary } from "../ipc/decks";
import * as templatesIpc from "../ipc/templates";
import {
  dispatchPlatformBack,
  platformBackHandlerCountForTests,
} from "../platform/backNavigation";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { CardEditorScreen } from "./CardEditorScreen";

vi.mock("../controllers/useDurableDraft", () => ({
  useDurableDraft: () => ({
    draftId: "d".repeat(32),
    assetDraftId: "7".repeat(32),
    revision: 1,
    status: "saved",
    errorMessage: null,
    recovered: false,
    hasUnsavedLocalChanges: false,
    flush: vi.fn().mockResolvedValue(undefined),
    discard: vi.fn().mockResolvedValue(undefined),
    prepareCanonicalCommit: vi.fn().mockResolvedValue({ draftId: "d".repeat(32), revision: 1 }),
    acceptCanonicalCommit: vi.fn(),
    commit: vi.fn().mockResolvedValue(undefined),
    restart: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock("../editor/RichTextEditor", () => ({
  RichTextEditor: ({ label }: { label: string }) => <div>{label} editor</div>,
}));
vi.mock("../editor/RichTextView", () => ({
  RichTextView: () => <div>Preview</div>,
}));
vi.mock("../ipc/cards", () => ({
  createCardNote: vi.fn(),
  findDuplicateCards: vi.fn(),
  getCardNote: vi.fn(),
  listTags: vi.fn(),
  updateCardNote: vi.fn(),
}));
vi.mock("../ipc/common", () => ({
  getErrorCode: vi.fn(() => null),
  getErrorMessage: vi.fn((error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  ),
}));
vi.mock("../ipc/templates", () => ({
  generateCardsFromTemplate: vi.fn(),
  getCardTemplate: vi.fn(),
  listCardTemplateSummaries: vi.fn(),
  previewTemplateCards: vi.fn(),
}));

const deck: DeckSummary = {
  id: "a".repeat(32),
  name: "Pharmacology",
  description: "",
  sourceNoteCount: 0,
  generatedCardCount: 0,
  studyableDueCount: 0,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

describe("CardEditorScreen platform Back", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(cardsIpc.listTags).mockResolvedValue([]);
    vi.mocked(templatesIpc.listCardTemplateSummaries).mockResolvedValue([]);
  });

  it("uses the same cancel path as the visible editor controls", async () => {
    const onCancel = vi.fn();
    const view = render(
      <OverlayProvider>
        <CardEditorScreen
          deck={deck}
          decks={[deck]}
          card={null}
          onCancel={onCancel}
          onSaved={vi.fn()}
          onTemplateGenerated={vi.fn()}
          onDirtyChange={vi.fn()}
        />
      </OverlayProvider>,
    );

    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(1));

    act(() => {
      expect(dispatchPlatformBack()).toBe(true);
    });
    await waitFor(() => expect(onCancel).toHaveBeenCalledOnce());

    view.unmount();
    expect(platformBackHandlerCountForTests()).toBe(0);
  });
});
