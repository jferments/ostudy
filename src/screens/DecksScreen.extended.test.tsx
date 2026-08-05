import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as generatedImport from "../generatedImport";
import * as ipc from "../ipc";
import { makeDeck, makeFlashcard, makeSettings, resetTestEntityIds } from "../test/factories";
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
  getErrorMessage: (error: unknown) => {
    if (error instanceof Error) return error.message;
    if (typeof error === "object" && error !== null && "message" in error) {
      return String((error as { message: unknown }).message);
    }
    return "Unexpected error";
  },
  importDeck: vi.fn(),
  listArchivedDecks: vi.fn(),
  listDeckCards: vi.fn(),
  undoDeletion: vi.fn(),
  unarchiveDeck: vi.fn(),
  updateDeck: vi.fn(),
}));
vi.mock("./CardEditorScreen", () => ({
  CardEditorScreen: () => <div>Card editor</div>,
}));

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
  render(<DecksScreen {...props} />);
  return { deck, props, refreshDecks, showToast };
}

function deletionResult(deckName: string): ipc.DeletionResult {
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
    vi.mocked(ipc.listArchivedDecks).mockResolvedValue([]);
    resetTestEntityIds();
    vi.mocked(openDialog).mockResolvedValue(null);
    vi.mocked(saveDialog).mockResolvedValue(null);
    vi.mocked(ipc.listDeckCards).mockResolvedValue({ cards: [], totalCount: 0 });
  });

  it("creates a named deck and opens its detail view", async () => {
    const existing = makeDeck({ name: "Existing" });
    const created = makeDeck({ name: "Neurology", description: "CNS review", sourceNoteCount: 0, generatedCardCount: 0 });
    vi.mocked(ipc.createDeck).mockResolvedValue(created);
    const { refreshDecks, showToast } = renderDeckLibrary(existing);

    fireEvent.click(screen.getByRole("button", { name: "New deck" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Deck name" }), {
      target: { value: "  Neurology  " },
    });
    fireEvent.change(screen.getByRole("textbox", { name: /Description/u }), {
      target: { value: "CNS review" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create deck" }));

    await waitFor(() => {
      expect(ipc.createDeck).toHaveBeenCalledWith({
        name: "  Neurology  ",
        description: "CNS review",
      });
      expect(showToast).toHaveBeenCalledWith("success", "Created “Neurology”.");
      expect(refreshDecks).toHaveBeenCalledOnce();
    });
    expect(screen.getByRole("heading", { name: "Neurology" })).toBeTruthy();
    expect(screen.getByText("This deck is empty")).toBeTruthy();
  });

  it("does not invoke an importer when the native picker is canceled", async () => {
    renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => expect(openDialog).toHaveBeenCalledOnce());
    expect(ipc.importDeck).not.toHaveBeenCalled();
    expect(generatedImport.importGeneratedFlashcards).not.toHaveBeenCalled();
  });

  it("rejects an unsupported selected extension without mutating the library", async () => {
    vi.mocked(openDialog).mockResolvedValue("/tmp/cards.txt");
    const { showToast } = renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith(
        "error",
        "Select an ostudy .studydeck or generated .ostudy.json file.",
      );
    });
    expect(ipc.importDeck).not.toHaveBeenCalled();
    expect(generatedImport.importGeneratedFlashcards).not.toHaveBeenCalled();
    expect(screen.getByRole("heading", { name: "Decks" })).toBeTruthy();
  });

  it("reports skipped generated-JSON duplicates from the authoritative result", async () => {
    const imported = makeDeck({ name: "Imported pharmacology", sourceNoteCount: 2, generatedCardCount: 3 });
    vi.mocked(openDialog).mockResolvedValue("/tmp/cards.ostudy.json");
    vi.mocked(generatedImport.importGeneratedFlashcards).mockResolvedValue({
      deckId: imported.id,
      createdNoteCount: 2,
      createdCardCount: 3,
      skippedDuplicateCount: 4,
    });
    vi.mocked(ipc.getDeck).mockResolvedValue(imported);
    const importedCard = makeFlashcard({ deckId: imported.id });
    vi.mocked(ipc.listDeckCards).mockResolvedValue({
      cards: [{ noteId: importedCard.noteId, primaryCardId: importedCard.id,
        questionExcerpt: importedCard.frontText, answerExcerpt: importedCard.backText,
        generatedCardCount: 1, imageCount: 0, updatedAt: importedCard.updatedAt }],
      totalCount: 1,
    });
    const { showToast } = renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Imported 2 source notes as 3 scheduled cards. 4 exact duplicates were skipped.",
      );
    });
    expect(screen.getByRole("heading", { name: imported.name })).toBeTruthy();
  });

  it("does not reclassify a committed studydeck import when detail refresh fails", async () => {
    const imported = makeDeck({ name: "Imported deck", sourceNoteCount: 2, generatedCardCount: 2 });
    vi.mocked(openDialog).mockResolvedValue("/tmp/imported.studydeck");
    vi.mocked(ipc.importDeck).mockResolvedValue({
      deck: imported,
      generatedCardCount: 2,
      assetCount: 0,
    });
    vi.mocked(ipc.listDeckCards).mockRejectedValue(new Error("Card list unavailable."));
    const refreshDecks = vi.fn().mockResolvedValue([imported]);
    const { showToast } = renderDeckLibrary(undefined, { refreshDecks });

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(ipc.importDeck).toHaveBeenCalledWith(
        "/tmp/imported.studydeck",
        importOperationId,
      );
      expect(showToast).toHaveBeenNthCalledWith(
        1,
        "success",
        "Imported “Imported deck” with 2 cards.",
      );
      expect(showToast).toHaveBeenNthCalledWith(
        2,
        "info",
        "The deck was imported, but it could not be displayed yet: Card list unavailable.",
      );
      expect(refreshDecks).toHaveBeenCalledOnce();
    });
    expect(showToast).not.toHaveBeenCalledWith("error", expect.anything());
    expect(screen.getByRole("heading", { name: imported.name })).toBeTruthy();
  });

  it("keeps the library usable when a studydeck import fails", async () => {
    vi.mocked(openDialog).mockResolvedValue("/tmp/broken.studydeck");
    vi.mocked(ipc.importDeck).mockRejectedValue({
      code: "invalid_deck_package",
      message: "Invalid deck package.",
    });
    const { showToast } = renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(ipc.importDeck).toHaveBeenCalledWith(
        "/tmp/broken.studydeck",
        importOperationId,
      );
      expect(showToast).toHaveBeenCalledWith("error", "Invalid deck package.");
    });
    expect(screen.getByRole("heading", { name: "Decks" })).toBeTruthy();
  });

  it("reuses one import operation identity after an ambiguous transport result", async () => {
    const imported = makeDeck({ name: "Recovered import", sourceNoteCount: 1, generatedCardCount: 1 });
    vi.mocked(openDialog).mockResolvedValue("/tmp/retry.studydeck");
    vi.mocked(ipc.importDeck)
      .mockRejectedValueOnce(new Error("Native response was interrupted."))
      .mockResolvedValueOnce({ deck: imported, generatedCardCount: 1, assetCount: 0 });
    renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));
    await waitFor(() => expect(ipc.importDeck).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(ipc.importDeck).toHaveBeenCalledTimes(2);
      expect(ipc.importDeck).toHaveBeenNthCalledWith(
        1,
        "/tmp/retry.studydeck",
        importOperationId,
      );
      expect(ipc.importDeck).toHaveBeenNthCalledWith(
        2,
        "/tmp/retry.studydeck",
        importOperationId,
      );
      expect(ipc.createOperationId).toHaveBeenCalledOnce();
    });
  });

  it("does not export when the native save picker is canceled", async () => {
    renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Export" }));

    await waitFor(() => expect(saveDialog).toHaveBeenCalledOnce());
    expect(ipc.exportDeck).not.toHaveBeenCalled();
  });

  it("exports to the selected path and reports the exact generated-card count", async () => {
    vi.mocked(saveDialog).mockResolvedValue("/tmp/cardiology.studydeck");
    vi.mocked(ipc.exportDeck).mockResolvedValue({
      destinationPath: "/tmp/cardiology.studydeck",
      generatedCardCount: 3,
      assetCount: 1,
    });
    const { deck, showToast } = renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Export" }));

    await waitFor(() => {
      expect(ipc.exportDeck).toHaveBeenCalledWith(
        deck.id,
        "/tmp/cardiology.studydeck",
      );
      expect(showToast).toHaveBeenCalledWith("success", "Exported 3 cards.");
    });
  });

  it("honors delete confirmation before issuing a destructive command", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const { deck } = renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    expect(confirm).toHaveBeenCalledWith(
      `Delete “${deck.name}” and its 1 card?`,
    );
    expect(ipc.deleteDeck).not.toHaveBeenCalled();

    confirm.mockReturnValue(true);
    vi.mocked(ipc.deleteDeck).mockResolvedValue(deletionResult(deck.name));
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(ipc.deleteDeck).toHaveBeenCalledWith(deck.id));
  });
});