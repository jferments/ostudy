import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as generatedImport from "../generatedImport";
import * as ipc from "../ipc";
import { DecksScreen } from "./DecksScreen";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
}));

vi.mock("../generatedImport", () => ({
  importGeneratedFlashcards: vi.fn(),
}));

const importOperationId = "9".repeat(32);

vi.mock("../ipc", () => ({
  archiveDeck: vi.fn(),
  createDeck: vi.fn(),
  createOperationId: vi.fn(() => importOperationId),
  deleteCard: vi.fn(),
  deleteDeck: vi.fn(),
  exportDeck: vi.fn(),
  getDeck: vi.fn(),
  getOrganizedCard: vi.fn(),
  getErrorCode: (error: unknown) =>
    typeof error === "object" && error !== null && "code" in error
      ? String((error as { code: unknown }).code)
      : null,
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  importDeck: vi.fn(),
  listArchivedDecks: vi.fn(),
  listDeckCards: vi.fn(),
  unarchiveDeck: vi.fn(),
  updateDeck: vi.fn(),
}));

vi.mock("./CardEditorScreen", () => ({
  CardEditorScreen: () => <div>Card editor</div>,
}));

const deck: ipc.Deck = {
  id: "a".repeat(32),
  name: "Dog facts",
  description: "Dogs are cool animals",
  sourceNoteCount: 2,
  generatedCardCount: 2,
  studyableDueCount: 0,
  createdAt: 1,
  updatedAt: 1,
  archivedAt: null,
  revision: 1,
};

const card: ipc.Flashcard = {
  id: "b".repeat(32),
  noteId: "c".repeat(32),
  deckId: deck.id,
  direction: "forward",
  frontDoc: { type: "doc", content: [] },
  backDoc: { type: "doc", content: [] },
  frontText: "Do dogs have fur?",
  backText: "Yes",
  assets: [],
  createdAt: 1,
  updatedAt: 1,
  archivedAt: null,
  revision: 1,
};

const settings: ipc.AppSettings = {
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
    vi.mocked(ipc.getDeck).mockResolvedValue(deck);
    vi.mocked(ipc.listArchivedDecks).mockResolvedValue([]);
    vi.mocked(ipc.listDeckCards).mockResolvedValue({ cards: [{ noteId: card.noteId, primaryCardId: card.id, questionExcerpt: card.frontText, answerExcerpt: card.backText, generatedCardCount: 1, imageCount: 0, updatedAt: card.updatedAt }], totalCount: 1 });
    vi.mocked(ipc.getOrganizedCard).mockResolvedValue({ ...card, tags: [], dueAt: Date.now() });
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
    vi.mocked(openDialog).mockResolvedValue("/tmp/nclex.ostudy.json");
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
    expect(ipc.importDeck).not.toHaveBeenCalled();
    expect(openDialog).toHaveBeenCalledWith(
      expect.objectContaining({
        filters: [
          expect.objectContaining({ extensions: ["studydeck", "json"] }),
        ],
      }),
    );
    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Dog facts" })).toBeTruthy();
    });
    expect(showToast).toHaveBeenCalledWith(
      "success",
      "Imported 15 source notes as 15 scheduled cards.",
    );
  });

  it("continues to import studydeck packages through the same button", async () => {
    vi.mocked(openDialog).mockResolvedValue("/tmp/dog-facts.studydeck");
    vi.mocked(ipc.importDeck).mockResolvedValue({
      deck,
      cardCount: 2,
      assetCount: 0,
    });
    renderDecks();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(ipc.importDeck).toHaveBeenCalledWith(
        "/tmp/dog-facts.studydeck",
        importOperationId,
      );
    });
    expect(ipc.createOperationId).toHaveBeenCalledOnce();
    expect(generatedImport.importGeneratedFlashcards).not.toHaveBeenCalled();
  });
});