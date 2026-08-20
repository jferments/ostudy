import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ToastAction } from "../components/Toast";
import * as cardsIpc from "../ipc/cards";
import * as decksIpc from "../ipc/decks";
import * as settingsIpc from "../ipc/settings";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { CardPracticeProvider } from "./cards/CardPracticeContext";
import { DecksScreen } from "./DecksScreen";

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
    commit: vi.fn().mockResolvedValue(undefined),
    restart: vi.fn().mockResolvedValue(undefined),
  }),
}));

vi.mock("../ipc/cards", () => ({
  analyzeBulkCards: vi.fn(),
  bulkCreateCards: vi.fn(),
  bulkTagCards: vi.fn(),
  deleteCards: vi.fn(),
  getCardNote: vi.fn(),
  listMatchingCards: vi.fn(),
  listTags: vi.fn(),
  searchCards: vi.fn(),
  transferCards: vi.fn(),
  undoDeletion: vi.fn(),
}));
vi.mock("../ipc/common", () => ({
  createMutationId: vi.fn(() => "f".repeat(32)),
  createOperationId: vi.fn(() => "c".repeat(32)),
  getErrorCode: () => null,
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));
vi.mock("../ipc/decks", () => ({
  createDeck: vi.fn(),
  deleteDeck: vi.fn(),
  prepareDeckExport: vi.fn(),
  getDeck: vi.fn(),
  importDeck: vi.fn(),
  setDeckArchived: vi.fn(),
  updateDeck: vi.fn(),
}));
vi.mock("../ipc/scheduling", () => ({
  applyScheduleAction: vi.fn(),
}));
vi.mock("../ipc/generated-import", () => ({
  importGeneratedFlashcards: vi.fn(),
}));
vi.mock("./CardEditorScreen", () => ({
  CardEditorScreen: () => <div>Card editor</div>,
}));

const deck: decksIpc.DeckSummary = {
  id: "a".repeat(32),
  name: "Temporary deck",
  description: "",
  sourceNoteCount: 2,
  generatedCardCount: 2,
  studyableDueCount: 1,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};
const undoToken = "b".repeat(32);

const settings: settingsIpc.AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  animationsEnabled: true,
  reduceMotion: false,
  confirmDeletions: false,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

describe("DecksScreen deletion undo", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(cardsIpc.listTags).mockResolvedValue([]);
    vi.mocked(cardsIpc.searchCards).mockResolvedValue({ cards: [], totalCount: 0 });
    vi.mocked(decksIpc.deleteDeck).mockResolvedValue({
      affectedCount: 2,
      undoToken,
      undoExpiresAt: Date.now() + 10_000,
      subjectLabel: "Deck “Temporary deck”",
    });
    vi.mocked(cardsIpc.undoDeletion).mockResolvedValue({
      restoredCount: 2,
      entityKind: "deck",
      subjectLabel: "Deck “Temporary deck”",
    });
  });

  it("confirms deletion, then publishes an Undo action that restores and refreshes the deck library", async () => {
    const refreshDecks = vi.fn().mockResolvedValue([deck]);
    const showToast = vi.fn();
    render(
      <OverlayProvider>
        <CardPracticeProvider onPracticeCards={vi.fn()}>
          <DecksScreen
            decks={[deck]}
            settings={settings}
            refreshDecks={refreshDecks}
            onPracticeDeck={vi.fn()}
            onDirtyChange={vi.fn()}
            showToast={showToast}
          />
        </CardPracticeProvider>
      </OverlayProvider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    const confirmDialog = await screen.findByRole("alertdialog", {
      name: "Delete “Temporary deck”?",
    });
    expect(decksIpc.deleteDeck).not.toHaveBeenCalled();
    fireEvent.click(within(confirmDialog).getByRole("button", { name: "Delete deck" }));

    await waitFor(() => {
      expect(decksIpc.deleteDeck).toHaveBeenCalledWith(deck.id);
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Deleted “Temporary deck”.",
        expect.objectContaining({ label: "Undo" }),
      );
    });

    const action = showToast.mock.calls[0]?.[2] as ToastAction;
    await action.run();
    expect(cardsIpc.undoDeletion).toHaveBeenCalledWith(undoToken);
    expect(refreshDecks).toHaveBeenCalled();
  });
});
