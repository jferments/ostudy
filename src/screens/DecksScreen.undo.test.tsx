import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ToastAction } from "../components/Toast";
import * as ipc from "../ipc";
import { DecksScreen } from "./DecksScreen";

vi.mock("../ipc", () => ({
  createDeck: vi.fn(),
  createOperationId: vi.fn(() => "c".repeat(32)),
  deleteCard: vi.fn(),
  deleteDeck: vi.fn(),
  exportDeck: vi.fn(),
  getDeck: vi.fn(),
  getErrorCode: () => null,
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  getCardNote: vi.fn(),
  importDeck: vi.fn(),
  listDeckCardSummaries: vi.fn(),
  setDeckArchived: vi.fn(),
  undoDeletion: vi.fn(),
  updateDeck: vi.fn(),
}));

vi.mock("../generatedImport", () => ({
  importGeneratedFlashcards: vi.fn(),
}));

vi.mock("./CardEditorScreen", () => ({
  CardEditorScreen: () => <div>Card editor</div>,
}));

const deck: ipc.DeckSummary = {
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

const settings: ipc.AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
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
    vi.mocked(ipc.listDeckCardSummaries).mockResolvedValue({
      cards: [],
      totalCount: 0,
      offset: 0,
      hasMore: false,
    });
    vi.mocked(ipc.deleteDeck).mockResolvedValue({
      affectedCount: 2,
      undoToken,
      undoExpiresAt: Date.now() + 10_000,
      subjectLabel: "Deck “Temporary deck”",
    });
    vi.mocked(ipc.undoDeletion).mockResolvedValue({
      restoredCount: 2,
      entityKind: "deck",
      subjectLabel: "Deck “Temporary deck”",
    });
  });

  it("publishes an Undo action that restores and refreshes the deck library", async () => {
    const refreshDecks = vi.fn().mockResolvedValue([deck]);
    const showToast = vi.fn();
    render(
      <DecksScreen
        decks={[deck]}
        settings={settings}
        refreshDecks={refreshDecks}
        onPracticeDeck={vi.fn()}
        onDirtyChange={vi.fn()}
        showToast={showToast}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    await waitFor(() => {
      expect(ipc.deleteDeck).toHaveBeenCalledWith(deck.id);
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Deleted “Temporary deck”.",
        expect.objectContaining({ label: "Undo" }),
      );
    });

    const action = showToast.mock.calls[0]?.[2] as ToastAction;
    await action.run();
    expect(ipc.undoDeletion).toHaveBeenCalledWith(undoToken);
    expect(refreshDecks).toHaveBeenCalled();
  });
});
