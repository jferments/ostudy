import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as generatedImport from "../ipc/generated-import";
import * as commonIpc from "../ipc/common";
import * as decksIpc from "../ipc/decks";
import * as platformFiles from "../platform/files";
import { makeDeck, makeSettings, resetTestEntityIds } from "../test/factories";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
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
  getErrorMessage: (error: unknown) => {
    if (error instanceof Error) return error.message;
    if (typeof error === "object" && error !== null && "message" in error) {
      return String((error as { message: unknown }).message);
    }
    return "Unexpected error";
  },
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
vi.mock("./CardEditorScreen", () => ({
  CardEditorScreen: () => <div>Card editor</div>,
}));

function summaryFor(deck: decksIpc.DeckSummary): decksIpc.DeckCardSummary {
  return {
    noteId: "c".repeat(32),
    primaryCardId: "b".repeat(32),
    questionExcerpt: `${deck.name} question`,
    answerExcerpt: "Answer",
    generatedCardCount: 1,
    imageCount: 0,
    reversible: false,
    updatedAt: 1,
  };
}

function pageFor(deck: decksIpc.DeckSummary): decksIpc.DeckCardSummaryPage {
  if (deck.sourceNoteCount === 0) {
    return { cards: [], totalCount: 0, offset: 0, hasMore: false };
  }
  return {
    cards: [summaryFor(deck)],
    totalCount: deck.sourceNoteCount,
    offset: 0,
    hasMore: deck.sourceNoteCount > 1,
  };
}

function renderDeckLibrary(
  deck = makeDeck({ name: "Cardiology", sourceNoteCount: 1, generatedCardCount: 1 }),
  overrides: Partial<React.ComponentProps<typeof DecksScreen>> = {},
) {
  const refreshDecks = vi.fn().mockResolvedValue([deck]);
  const showToast = vi.fn();
  const props: React.ComponentProps<typeof DecksScreen> = {
    decks: [deck],
    settings: makeSettings(),
    refreshDecks,
    onPracticeDeck: vi.fn(),
    onDirtyChange: vi.fn(),
    showToast,
    ...overrides,
  };
  render(
    <OverlayProvider>
      <DecksScreen {...props} />
    </OverlayProvider>,
  );
  return { deck, props, refreshDecks, showToast };
}

function deletionResult(deckName: string): commonIpc.DeletionResult {
  return {
    affectedCount: 1,
    undoToken: "f".repeat(32),
    undoExpiresAt: Date.now() + 10_000,
    subjectLabel: `Deck “${deckName}”`,
  };
}

describe("DecksScreen extended behavior", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTestEntityIds();
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue(null);
    vi.mocked(platformFiles.publishExport).mockResolvedValue({ preparedExportId: "2".repeat(32), displayName: "export", destinationKind: "desktop_file", completedAt: Date.now() });
    vi.mocked(decksIpc.listDeckCardSummaries).mockResolvedValue({
      cards: [],
      totalCount: 0,
      offset: 0,
      hasMore: false,
    });
  });

  it("creates a named deck and opens its detail view", async () => {
    const existing = makeDeck({ name: "Existing" });
    const created = makeDeck({
      name: "Neurology",
      description: "CNS review",
      sourceNoteCount: 0,
      generatedCardCount: 0,
      studyableDueCount: 0,
    });
    vi.mocked(decksIpc.createDeck).mockResolvedValue(created);
    const { refreshDecks, showToast } = renderDeckLibrary(existing);

    fireEvent.click(screen.getByRole("button", { name: "New deck" }));
    const nameInput = screen.getByRole("textbox", { name: "Deck name" });
    await waitFor(() => expect(document.activeElement).toBe(nameInput));
    fireEvent.change(nameInput, {
      target: { value: "  Neurology  " },
    });
    fireEvent.change(screen.getByRole("textbox", { name: /Description/u }), {
      target: { value: "CNS review" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create deck" }));

    await waitFor(() => {
      expect(decksIpc.createDeck).toHaveBeenCalledWith({
        name: "  Neurology  ",
        description: "CNS review",
      });
      expect(showToast).toHaveBeenCalledWith("success", "Created “Neurology”.");
      expect(refreshDecks).toHaveBeenCalledOnce();
    });
    expect(screen.getByRole("heading", { name: "Neurology" })).toBeTruthy();
    expect(screen.getByText("This deck is empty")).toBeTruthy();
  });

  it("does not invoke an importer when the platform picker is canceled", async () => {
    renderDeckLibrary();
    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));
    await waitFor(() => expect(platformFiles.selectExternalImport).toHaveBeenCalledOnce());
    expect(decksIpc.importDeck).not.toHaveBeenCalled();
    expect(generatedImport.importGeneratedFlashcards).not.toHaveBeenCalled();
  });

  it("rejects an unsupported selected extension without mutating the library", async () => {
    vi.mocked(platformFiles.selectExternalImport).mockRejectedValue(new Error("Unsupported external file."));
    const { showToast } = renderDeckLibrary();
    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));
    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith(
        "error",
        "Unsupported external file.",
      );
    });
    expect(decksIpc.importDeck).not.toHaveBeenCalled();
    expect(generatedImport.importGeneratedFlashcards).not.toHaveBeenCalled();
    expect(screen.getByRole("heading", { name: "Decks" })).toBeTruthy();
  });

  it("reports skipped generated-JSON duplicates from the authoritative result", async () => {
    const imported = makeDeck({
      name: "Imported pharmacology",
      sourceNoteCount: 2,
      generatedCardCount: 3,
    });
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({ id: "1".repeat(32), purpose: "study_content", detectedKind: "generated_json", displayName: "cards.ostudy.json", mediaType: "application/octet-stream", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 });
    vi.mocked(generatedImport.importGeneratedFlashcards).mockResolvedValue({
      deckId: imported.id,
      createdNoteCount: 2,
      createdCardCount: 3,
      skippedDuplicateCount: 4,
    });
    vi.mocked(decksIpc.getDeck).mockResolvedValue(imported);
    vi.mocked(decksIpc.listDeckCardSummaries).mockResolvedValue(pageFor(imported));
    const { showToast } = renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Imported 2 card entries as 3 scheduled cards. 4 exact duplicates were skipped.",
      );
    });
    expect(screen.getByRole("heading", { name: imported.name })).toBeTruthy();
  });

  it("keeps a committed studydeck import open when its card page cannot load", async () => {
    const imported = makeDeck({
      name: "Imported deck",
      sourceNoteCount: 2,
      generatedCardCount: 2,
    });
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({ id: "1".repeat(32), purpose: "study_content", detectedKind: "studydeck", displayName: "imported.studydeck", mediaType: "application/octet-stream", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 });
    vi.mocked(decksIpc.importDeck).mockResolvedValue({
      deck: imported,
      cardCount: 2,
      assetCount: 0,
    });
    vi.mocked(decksIpc.listDeckCardSummaries).mockRejectedValue(
      new Error("Card page unavailable."),
    );
    const refreshDecks = vi.fn().mockResolvedValue([imported]);
    const { showToast } = renderDeckLibrary(undefined, { refreshDecks });

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(decksIpc.importDeck).toHaveBeenCalledWith(
        "1".repeat(32),
        importOperationId,
      );
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Imported “Imported deck” with 2 cards.",
      );
      expect(refreshDecks).toHaveBeenCalledOnce();
    });
    expect(screen.getByRole("heading", { name: imported.name })).toBeTruthy();
    expect(await screen.findByText("Card page unavailable.")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "No matching flashcards" })).toBeNull();
  });

  it("keeps the library usable when a studydeck import fails", async () => {
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({ id: "1".repeat(32), purpose: "study_content", detectedKind: "studydeck", displayName: "broken.studydeck", mediaType: "application/octet-stream", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 });
    vi.mocked(decksIpc.importDeck).mockRejectedValue({
      code: "invalid_deck_package",
      message: "Invalid deck package.",
    });
    const { showToast } = renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(decksIpc.importDeck).toHaveBeenCalledWith(
        "1".repeat(32),
        importOperationId,
      );
      expect(showToast).toHaveBeenCalledWith("error", "Invalid deck package.");
    });
    expect(screen.getByRole("heading", { name: "Decks" })).toBeTruthy();
  });

  it("reuses one import operation identity after an ambiguous transport result", async () => {
    const imported = makeDeck({ name: "Recovered import" });
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({ id: "1".repeat(32), purpose: "study_content", detectedKind: "studydeck", displayName: "retry.studydeck", mediaType: "application/octet-stream", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 });
    vi.mocked(decksIpc.importDeck)
      .mockRejectedValueOnce(new Error("Native response was interrupted."))
      .mockResolvedValueOnce({ deck: imported, cardCount: 1, assetCount: 0 });
    renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));
    await waitFor(() => expect(decksIpc.importDeck).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(decksIpc.importDeck).toHaveBeenCalledTimes(2);
      expect(decksIpc.importDeck).toHaveBeenNthCalledWith(
        1,
        "1".repeat(32),
        importOperationId,
      );
      expect(decksIpc.importDeck).toHaveBeenNthCalledWith(
        2,
        "1".repeat(32),
        importOperationId,
      );
      expect(commonIpc.createOperationId).toHaveBeenCalled();
    });
  });

  it("does not report success when export publication is canceled", async () => {
    vi.mocked(decksIpc.prepareDeckExport).mockResolvedValue({
      preparedExport: {
        id: "2".repeat(32),
        purpose: "studydeck",
        suggestedFilename: "cardiology.studydeck",
        mediaType: "application/vnd.ostudy.studydeck",
        byteSize: 128,
        sha256: "0".repeat(64),
        expiresAt: Date.now() + 60_000,
      },
      cardCount: 1,
      assetCount: 0,
    });
    vi.mocked(platformFiles.publishExport).mockResolvedValue(null);
    const { showToast } = renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Export" }));

    await waitFor(() => {
      expect(decksIpc.prepareDeckExport).toHaveBeenCalledOnce();
      expect(platformFiles.publishExport).toHaveBeenCalledWith("2".repeat(32));
    });
    expect(showToast).not.toHaveBeenCalledWith("success", expect.any(String));
  });

  it("exports to the selected path and reports the exact generated-card count", async () => {
    vi.mocked(platformFiles.publishExport).mockResolvedValue({ preparedExportId: "2".repeat(32), displayName: "export", destinationKind: "desktop_file", completedAt: Date.now() });
    vi.mocked(decksIpc.prepareDeckExport).mockResolvedValue({ preparedExport: { id: "2".repeat(32), purpose: "studydeck", suggestedFilename: "deck.studydeck", mediaType: "application/vnd.ostudy.studydeck", byteSize: 128, sha256: "0".repeat(64), expiresAt: Date.now() + 60_000 }, cardCount: 3, assetCount: 1 });
    const { deck, showToast } = renderDeckLibrary();
    fireEvent.click(screen.getByRole("button", { name: "Export" }));
    await waitFor(() => {
      expect(decksIpc.prepareDeckExport).toHaveBeenCalledWith(deck.id, importOperationId);
      expect(showToast).toHaveBeenCalledWith("success", "Exported 3 cards.");
    });
  });

  it("honors shared delete confirmation before issuing a destructive command", async () => {
    const { deck } = renderDeckLibrary();
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(
      await screen.findByRole("alertdialog", { name: `Delete “${deck.name}”?` }),
    ).toBeTruthy();
    expect(decksIpc.deleteDeck).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(decksIpc.deleteDeck).not.toHaveBeenCalled();

    vi.mocked(decksIpc.deleteDeck).mockResolvedValue(deletionResult(deck.name));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    await screen.findByRole("alertdialog", { name: `Delete “${deck.name}”?` });
    fireEvent.click(screen.getByRole("button", { name: "Delete deck" }));
    await waitFor(() => expect(decksIpc.deleteDeck).toHaveBeenCalledWith(deck.id));
  });
});
