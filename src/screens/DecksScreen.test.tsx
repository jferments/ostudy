import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as generatedImport from "../ipc/generated-import";
import * as commonIpc from "../ipc/common";
import * as decksIpc from "../ipc/decks";
import * as settingsIpc from "../ipc/settings";
import * as platformFiles from "../platform/files";
import { DecksScreen } from "./DecksScreen";

vi.mock("../platform/files", () => ({
  openFile: vi.fn(),
  saveFile: vi.fn(),
}));

vi.mock("../ipc/generated-import", () => ({
  importGeneratedFlashcards: vi.fn(),
}));

const importOperationId = "9".repeat(32);

vi.mock("../ipc/cards", () => ({
  deleteCard: vi.fn(),
  getCardNote: vi.fn(),
  undoDeletion: vi.fn(),
}));
vi.mock("../ipc/common", () => ({
  createOperationId: vi.fn(() => importOperationId),
  getErrorCode: (error: unknown) =>
    typeof error === "object" && error !== null && "code" in error
      ? String((error as { code: unknown }).code)
      : null,
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

vi.mock("./CardEditorScreen", () => ({
  CardEditorScreen: () => <div>Card editor</div>,
}));

const deck: decksIpc.DeckSummary = {
  id: "a".repeat(32),
  name: "Dog facts",
  description: "Dogs are cool animals",
  sourceNoteCount: 2,
  generatedCardCount: 2,
  studyableDueCount: 0,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

const card: decksIpc.DeckCardSummary = {
  noteId: "c".repeat(32),
  primaryCardId: "b".repeat(32),
  questionExcerpt: "Do dogs have fur?",
  answerExcerpt: "Yes",
  generatedCardCount: 1,
  imageCount: 0,
  updatedAt: 1,
};

const settings: settingsIpc.AppSettings = {
  theme: "synthwave",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  reduceMotion: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

function renderDecks(showToast = vi.fn()) {
  render(
    <DecksScreen
      decks={[deck]}
      settings={settings}
      refreshDecks={async () => [deck]}
      onPracticeDeck={vi.fn()}
      onDirtyChange={vi.fn()}
      showToast={showToast}
    />,
  );
  return showToast;
}

describe("DecksScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(platformFiles.openFile).mockResolvedValue(null);
    vi.mocked(platformFiles.saveFile).mockResolvedValue(null);
    vi.mocked(decksIpc.getDeck).mockResolvedValue(deck);
    vi.mocked(decksIpc.listDeckCardSummaries).mockResolvedValue({
      cards: [card],
      totalCount: 1,
      offset: 0,
      hasMore: false,
    });
  });

  it("keeps Practice enabled for a non-empty deck with no cards due", async () => {
    const onPracticeDeck = vi.fn();
    render(
      <DecksScreen
        decks={[deck]}
        settings={settings}
        refreshDecks={async () => [deck]}
        onPracticeDeck={onPracticeDeck}
        onDirtyChange={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    const libraryPractice = screen.getByRole("button", { name: "Practice" });
    expect(libraryPractice.hasAttribute("disabled")).toBe(false);

    const deckButton = screen.getByText("Dog facts").closest("button");
    expect(deckButton).not.toBeNull();
    fireEvent.click(deckButton as HTMLButtonElement);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Dog facts" })).toBeTruthy();
    });

    const detailPractice = screen.getByRole("button", { name: "Practice" });
    expect(detailPractice.hasAttribute("disabled")).toBe(false);
    fireEvent.click(detailPractice);

    expect(onPracticeDeck).toHaveBeenCalledWith(deck.id);
  });

  it("imports generated JSON through the ordinary Import deck button", async () => {
    vi.mocked(platformFiles.openFile).mockResolvedValue("/tmp/nclex.ostudy.json");
    vi.mocked(generatedImport.importGeneratedFlashcards).mockResolvedValue({
      deckId: deck.id,
      createdNoteCount: 15,
      createdCardCount: 15,
      skippedDuplicateCount: 0,
    });
    const showToast = renderDecks();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(generatedImport.importGeneratedFlashcards).toHaveBeenCalledWith(
        "/tmp/nclex.ostudy.json",
      );
    });
    expect(decksIpc.importDeck).not.toHaveBeenCalled();
    expect(platformFiles.openFile).toHaveBeenCalledWith(
      expect.objectContaining({
        filters: [expect.objectContaining({ extensions: ["studydeck", "json"] })],
      }),
    );
    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Dog facts" })).toBeTruthy();
    });
    expect(showToast).toHaveBeenCalledWith(
      "success",
      "Imported 15 card entries as 15 scheduled cards.",
    );
  });

  it("continues to import studydeck packages through the same button", async () => {
    vi.mocked(platformFiles.openFile).mockResolvedValue("/tmp/dog-facts.studydeck");
    vi.mocked(decksIpc.importDeck).mockResolvedValue({
      deck,
      cardCount: 2,
      assetCount: 0,
    });
    renderDecks();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(decksIpc.importDeck).toHaveBeenCalledWith(
        "/tmp/dog-facts.studydeck",
        importOperationId,
      );
    });
    expect(commonIpc.createOperationId).toHaveBeenCalledOnce();
    expect(generatedImport.importGeneratedFlashcards).not.toHaveBeenCalled();
  });
});
