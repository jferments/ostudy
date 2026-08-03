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
vi.mock("../ipc", () => ({
  createDeck: vi.fn(),
  deleteCard: vi.fn(),
  deleteDeck: vi.fn(),
  exportDeck: vi.fn(),
  getDeck: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  importDeck: vi.fn(),
  listCards: vi.fn(),
  updateDeck: vi.fn(),
}));
vi.mock("./CardEditorScreen", () => ({
  CardEditorScreen: () => <div>Card editor</div>,
}));

function renderDeckLibrary(
  deck = makeDeck({ name: "Cardiology", cardCount: 1 }),
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

describe("DecksScreen extended behavior", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetTestEntityIds();
    vi.mocked(openDialog).mockResolvedValue(null);
    vi.mocked(saveDialog).mockResolvedValue(null);
    vi.mocked(ipc.listCards).mockResolvedValue([]);
  });

  it("creates a named deck and opens its detail view", async () => {
    const existing = makeDeck({ name: "Existing" });
    const created = makeDeck({ name: "Neurology", description: "CNS review", cardCount: 0 });
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
    const imported = makeDeck({ name: "Imported pharmacology", cardCount: 3 });
    vi.mocked(openDialog).mockResolvedValue("/tmp/cards.ostudy.json");
    vi.mocked(generatedImport.importGeneratedFlashcards).mockResolvedValue({
      deckId: imported.id,
      createdNoteCount: 2,
      createdCardCount: 3,
      skippedDuplicateCount: 4,
    });
    vi.mocked(ipc.getDeck).mockResolvedValue(imported);
    vi.mocked(ipc.listCards).mockResolvedValue([
      makeFlashcard({ deckId: imported.id }),
    ]);
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

  it("keeps the library usable when a studydeck import fails", async () => {
    vi.mocked(openDialog).mockResolvedValue("/tmp/broken.studydeck");
    vi.mocked(ipc.importDeck).mockRejectedValue(new Error("Invalid deck package."));
    const { showToast } = renderDeckLibrary();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith("error", "Invalid deck package.");
    });
    expect(screen.getByRole("heading", { name: "Decks" })).toBeTruthy();
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
      cardCount: 3,
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
    vi.mocked(ipc.deleteDeck).mockResolvedValue(undefined);
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(ipc.deleteDeck).toHaveBeenCalledWith(deck.id));
  });
});
