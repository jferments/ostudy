import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ToastAction } from "../components/Toast";
import * as cardsIpc from "../ipc/cards";
import * as decksIpc from "../ipc/decks";
import * as settingsIpc from "../ipc/settings";
import { DecksScreen } from "./DecksScreen";

vi.mock("../ipc/cards", () => ({
  deleteCard: vi.fn(),
  getCardNote: vi.fn(),
  undoDeletion: vi.fn(),
}));
vi.mock("../ipc/common", () => ({
  createOperationId: vi.fn(() => "c".repeat(32)),
  getErrorCode: () => null,
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));
vi.mock("../ipc/decks", () => ({
  createDeck: vi.fn(),
  deleteDeck: vi.fn(),
  exportDeck: vi.fn(),
  getDeck: vi.fn(),
  importDeck: vi.fn(),
  listDeckCardSummaries: vi.fn(),
  setDeckArchived: vi.fn(),
  updateDeck: vi.fn(),
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
    vi.mocked(decksIpc.listDeckCardSummaries).mockResolvedValue({
      cards: [],
      totalCount: 0,
      offset: 0,
      hasMore: false,
    });
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
