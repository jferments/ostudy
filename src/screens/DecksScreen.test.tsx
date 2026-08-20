import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as generatedImport from "../ipc/generated-import";
import * as commonIpc from "../ipc/common";
import * as decksIpc from "../ipc/decks";
import * as settingsIpc from "../ipc/settings";
import { dispatchPlatformBack, platformBackHandlerCountForTests } from "../platform/backNavigation";
import * as platformFiles from "../platform/files";
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

vi.mock("../platform/files", () => ({
  selectExternalImport: vi.fn(),
  publishExport: vi.fn(),
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
  prepareDeckExport: vi.fn(),
  getDeck: vi.fn(),
  importDeck: vi.fn(),
  listDeckCardSummaries: vi.fn(),
  setDeckArchived: vi.fn(),
  updateDeck: vi.fn(),
}));

vi.mock("./CardEditorScreen", async () => {
  const React = await import("react");
  return {
    CardEditorScreen: ({
      onCancel,
      onRegisterBackAction,
    }: {
      onCancel: () => void;
      onRegisterBackAction?: (action: (() => void) | null) => void;
    }) => {
      React.useLayoutEffect(() => {
        onRegisterBackAction?.(onCancel);
        return () => onRegisterBackAction?.(null);
      }, [onCancel, onRegisterBackAction]);
      return <div>Card editor</div>;
    },
  };
});

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
  reversible: true,
  updatedAt: 1,
};

const settings: settingsIpc.AppSettings = {
  theme: "synthwave",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  animationsEnabled: true,
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
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue(null);
    vi.mocked(platformFiles.publishExport).mockResolvedValue({ preparedExportId: "2".repeat(32), displayName: "export", destinationKind: "desktop_file", completedAt: Date.now() });
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
    expect(await screen.findByLabelText("Reversible card")).toBeTruthy();
    expect(screen.queryByText(/card variants/u)).toBeNull();

    const detailPractice = screen.getByRole("button", { name: "Practice" });
    expect(detailPractice.hasAttribute("disabled")).toBe(false);
    fireEvent.click(detailPractice);

    expect(onPracticeDeck).toHaveBeenCalledWith(deck.id, "detail");
  });

  it("keeps Android Back owned through editor, detail, and library", async () => {
    const view = render(
      <DecksScreen
        decks={[deck]}
        settings={settings}
        refreshDecks={async () => [deck]}
        onPracticeDeck={vi.fn()}
        onDirtyChange={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    const deckButton = screen.getByText("Dog facts").closest("button");
    expect(deckButton).not.toBeNull();
    fireEvent.click(deckButton as HTMLButtonElement);
    await waitFor(() => expect(screen.getByRole("heading", { name: "Dog facts" })).toBeTruthy());
    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(1));

    fireEvent.click(screen.getByRole("button", { name: "Add flashcard" }));
    await waitFor(() => expect(screen.getByText("Card editor")).toBeTruthy());
    expect(platformBackHandlerCountForTests()).toBe(1);

    await act(async () => expect(dispatchPlatformBack()).toBe(true));
    await waitFor(() => expect(screen.getByRole("heading", { name: "Dog facts" })).toBeTruthy());
    expect(screen.queryByText("Card editor")).toBeNull();
    expect(platformBackHandlerCountForTests()).toBe(1);

    await act(async () => expect(dispatchPlatformBack()).toBe(true));
    await waitFor(() => expect(screen.getByRole("heading", { name: "Decks" })).toBeTruthy());
    await waitFor(() => expect(platformBackHandlerCountForTests()).toBe(0));
    view.unmount();
  });

  it("imports generated JSON through the ordinary Import deck button", async () => {
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({ id: "1".repeat(32), purpose: "study_content", detectedKind: "generated_json", displayName: "nclex.ostudy.json", mediaType: "application/octet-stream", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 });
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
        "1".repeat(32),
      );
    });
    expect(decksIpc.importDeck).not.toHaveBeenCalled();
    expect(platformFiles.selectExternalImport).toHaveBeenCalledWith("study_content");
    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Dog facts" })).toBeTruthy();
    });
    expect(showToast).toHaveBeenCalledWith(
      "success",
      "Imported 15 card entries as 15 scheduled cards.",
    );
  });

  it("continues to import studydeck packages through the same button", async () => {
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({ id: "1".repeat(32), purpose: "study_content", detectedKind: "studydeck", displayName: "dog-facts.studydeck", mediaType: "application/octet-stream", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 });
    vi.mocked(decksIpc.importDeck).mockResolvedValue({
      deck,
      cardCount: 2,
      assetCount: 0,
    });
    renderDecks();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(decksIpc.importDeck).toHaveBeenCalledWith(
        "1".repeat(32),
        importOperationId,
      );
    });
    expect(commonIpc.createOperationId).toHaveBeenCalled();
    expect(generatedImport.importGeneratedFlashcards).not.toHaveBeenCalled();
  });
});
