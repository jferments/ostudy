import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as cardsIpc from "../ipc/cards";
import * as generatedImport from "../ipc/generated-import";
import * as commonIpc from "../ipc/common";
import * as decksIpc from "../ipc/decks";
import * as settingsIpc from "../ipc/settings";
import * as platformFiles from "../platform/files";
import { LayoutModeProvider } from "../ui/layout/LayoutModeProvider";
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

vi.mock("../platform/files", () => ({
  selectExternalImport: vi.fn(),
  publishExport: vi.fn(),
}));
vi.mock("../ipc/generated-import", () => ({
  importGeneratedFlashcards: vi.fn(),
}));

const importOperationId = "9".repeat(32);
const originalMatchMedia = window.matchMedia;

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
}));
vi.mock("../ipc/common", () => ({
  createMutationId: vi.fn(() => "f".repeat(32)),
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
  setDeckArchived: vi.fn(),
  updateDeck: vi.fn(),
}));
vi.mock("../ipc/scheduling", () => ({
  applyScheduleAction: vi.fn(),
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

const card: cardsIpc.CardBrowserRow = {
  cardId: "b".repeat(32),
  noteId: "c".repeat(32),
  deckId: deck.id,
  deckName: deck.name,
  cardType: "basic",
  cardLabel: "Reverse",
  frontText: "Do dogs have fur?",
  backText: "Yes",
  tags: [],
  dueAt: Date.now() + 60_000,
  lastReviewedAt: null,
  reviewCount: 0,
  suspendedAt: null,
  buriedUntil: null,
  needsMorePractice: false,
  createdAt: 1,
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

function setLayoutMode(mode: "wide" | "medium" | "compact"): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: query.includes("760")
      ? mode === "compact"
      : query.includes("1180")
        ? mode !== "wide"
        : false,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

function renderDecks(
  showToast = vi.fn(),
  onPracticeDeck = vi.fn(),
  layoutMode: "wide" | "medium" | "compact" = "wide",
  availableDecks: decksIpc.DeckSummary[] = [deck],
) {
  setLayoutMode(layoutMode);
  render(
    <LayoutModeProvider>
      <CardPracticeProvider onPracticeCards={vi.fn()}>
        <DecksScreen
          decks={availableDecks}
          settings={settings}
          refreshDecks={async () => availableDecks}
          onPracticeDeck={onPracticeDeck}
          onDirtyChange={vi.fn()}
          showToast={showToast}
        />
      </CardPracticeProvider>
    </LayoutModeProvider>,
  );
  return { showToast, onPracticeDeck };
}

async function openDeckDetail(): Promise<HTMLElement> {
  fireEvent.click(screen.getByText("Dog facts").closest("button") as HTMLButtonElement);
  return screen.findByRole("heading", { name: "Dog facts" });
}

describe("DecksScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue(null);
    vi.mocked(platformFiles.publishExport).mockResolvedValue({
      preparedExportId: "2".repeat(32),
      displayName: "export",
      destinationKind: "desktop_file",
      completedAt: Date.now(),
    });
    vi.mocked(decksIpc.getDeck).mockResolvedValue(deck);
    vi.mocked(cardsIpc.listTags).mockResolvedValue([]);
    vi.mocked(cardsIpc.searchCards).mockResolvedValue({ cards: [card], totalCount: 1 });
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
  });

  it("uses the shared Q/A browser with deck-only authoring and lightweight search", async () => {
    const { onPracticeDeck } = renderDecks();

    const libraryPractice = screen.getByRole("button", { name: "Practice" });
    expect(libraryPractice.hasAttribute("disabled")).toBe(false);

    const deckButton = screen.getByText("Dog facts").closest("button");
    expect(deckButton).not.toBeNull();
    fireEvent.click(deckButton as HTMLButtonElement);

    expect(await screen.findByRole("heading", { name: "Dog facts" })).toBeTruthy();
    const table = await screen.findByRole("table", { name: "Matching cards" });
    expect(within(table).getByText("Q:")).toBeTruthy();
    expect(within(table).getByText("A:")).toBeTruthy();
    expect(within(table).getByText("Reverse")).toBeTruthy();

    expect(screen.queryByRole("region", { name: "Card filters" })).toBeNull();
    expect(screen.getByRole("region", { name: "Search this deck" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Bulk add" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Add flashcard" })).toBeTruthy();

    expect(screen.queryByRole("button", { name: "Practice selected" })).toBeNull();
    fireEvent.click(screen.getByRole("checkbox", { name: "Select Do dogs have fur?" }));
    const practiceSelected = screen.getByRole("button", { name: "Practice selected" });
    expect((practiceSelected as HTMLButtonElement).disabled).toBe(false);

    fireEvent.change(screen.getByRole("searchbox", { name: "Search cards" }), {
      target: { value: "fur" },
    });
    await waitFor(() =>
      expect(cardsIpc.searchCards).toHaveBeenLastCalledWith(
        expect.objectContaining({
          deckIds: [deck.id],
          query: "fur",
          statusFilter: "all",
          sort: "relevance",
        }),
      ),
    );

    const detailPractice = screen.getByRole("button", { name: "Practice" });
    expect(detailPractice.hasAttribute("disabled")).toBe(false);
    fireEvent.click(detailPractice);
    expect(onPracticeDeck).toHaveBeenCalledWith(deck.id);
  });

  it("opens the existing edit workflow from the shared icon action", async () => {
    renderDecks();
    await openDeckDetail();

    const editButton = screen.getByRole("button", {
      name: "Edit deck name and description",
    });
    fireEvent.focus(editButton);
    expect(screen.getByRole("tooltip").textContent).toBe("Edit deck");
    fireEvent.click(editButton);

    const dialog = await screen.findByRole("dialog", { name: "Edit deck" });
    expect((within(dialog).getByRole("textbox", { name: /Deck name/u }) as HTMLInputElement).value)
      .toBe("Dog facts");
  });

  it("keeps zero-card Practice direct and explains why the primary action is disabled", async () => {
    const emptyDeck = { ...deck, generatedCardCount: 0 };
    vi.mocked(decksIpc.getDeck).mockResolvedValue(emptyDeck);
    const { onPracticeDeck } = renderDecks(vi.fn(), vi.fn(), "compact", [emptyDeck]);
    await openDeckDetail();

    const group = screen.getByRole("group", { name: "Dog facts actions" });
    const practice = within(group).getByRole("button", { name: "Practice" });
    expect(practice.textContent).toBe("Practice");
    expect(practice.className).toMatch(/primary/u);
    expect((practice as HTMLButtonElement).disabled).toBe(true);
    expect(practice.getAttribute("title")).toBe(
      "Add a flashcard before practicing this deck",
    );
    const descriptionId = practice.getAttribute("aria-describedby");
    expect(descriptionId).toBeTruthy();
    expect(document.getElementById(descriptionId as string)?.textContent).toBe(
      "Add a flashcard before practicing this deck",
    );
    fireEvent.click(practice);
    expect(onPracticeDeck).not.toHaveBeenCalled();

    fireEvent.click(within(group).getByRole("button", { name: "More actions" }));
    expect(screen.getByRole("menuitem", { name: "Export" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Archive Dog facts" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Delete Dog facts" })).toBeTruthy();
  });

  it("routes the shared archive icon through the existing revision-checked handler", async () => {
    vi.mocked(decksIpc.setDeckArchived).mockResolvedValue({
      ...deck,
      archivedAt: Date.now(),
      revision: 2,
    });
    renderDecks();
    await openDeckDetail();

    fireEvent.click(screen.getByRole("button", { name: "Archive Dog facts" }));
    await waitFor(() =>
      expect(decksIpc.setDeckArchived).toHaveBeenCalledWith({
        deckId: deck.id,
        revision: deck.revision,
        archived: true,
      }),
    );
  });

  it("preserves the detail Export handler and visible busy state", async () => {
    let resolveExport!: (value: decksIpc.PreparedDeckExport) => void;
    vi.mocked(decksIpc.prepareDeckExport).mockReturnValue(
      new Promise((resolve) => {
        resolveExport = resolve;
      }),
    );
    renderDecks();
    await openDeckDetail();

    const exportButton = screen.getByRole("button", { name: "Export" });
    fireEvent.click(exportButton);
    await waitFor(() => expect(exportButton.getAttribute("aria-busy")).toBe("true"));
    expect((exportButton as HTMLButtonElement).disabled).toBe(true);
    expect(decksIpc.prepareDeckExport).toHaveBeenCalledWith(deck.id, importOperationId);

    resolveExport({
      preparedExport: {
        id: "2".repeat(32),
        purpose: "studydeck",
        suggestedFilename: "dog-facts.studydeck",
        mediaType: "application/vnd.ostudy.studydeck",
        byteSize: 128,
        sha256: "0".repeat(64),
        expiresAt: Date.now() + 60_000,
      },
      cardCount: deck.generatedCardCount,
      assetCount: 0,
    });
    await waitFor(() => expect(platformFiles.publishExport).toHaveBeenCalledWith("2".repeat(32)));
    await waitFor(() => expect(exportButton.hasAttribute("aria-busy")).toBe(false));
  });

  it("orders the deck header as Back, title, description, then actions", async () => {
    renderDecks();
    const heading = await openDeckDetail();
    const header = heading.closest("header");
    expect(header).not.toBeNull();
    const back = within(header as HTMLElement).getByRole("button", { name: "← All decks" });
    const description = within(header as HTMLElement).getByText(deck.description);
    const actions = within(header as HTMLElement).getByRole("group", {
      name: "Dog facts actions",
    });

    expect(back.compareDocumentPosition(heading)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(heading.compareDocumentPosition(description)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(description.compareDocumentPosition(actions)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      within(header as HTMLElement).getByRole("button", {
        name: "Edit deck name and description",
      }),
    ).toBeTruthy();
  });

  it.each([
    {
      mode: "wide",
      direct: ["Practice", "Export", "Archive Dog facts"],
      overflow: ["Delete Dog facts"],
    },
    {
      mode: "medium",
      direct: ["Practice", "Export"],
      overflow: ["Archive Dog facts", "Delete Dog facts"],
    },
    {
      mode: "compact",
      direct: ["Practice"],
      overflow: ["Export", "Archive Dog facts", "Delete Dog facts"],
    },
  ] as const)("uses shared $mode deck-action placement", async ({ mode, direct, overflow }) => {
    renderDecks(vi.fn(), vi.fn(), mode);
    await openDeckDetail();
    const group = screen.getByRole("group", { name: "Dog facts actions" });
    expect(group.getAttribute("data-layout-mode")).toBe(mode);
    for (const label of direct) {
      expect(within(group).getByRole("button", { name: label })).toBeTruthy();
    }
    for (const label of overflow) {
      expect(within(group).queryByRole("button", { name: label })).toBeNull();
    }

    fireEvent.click(within(group).getByRole("button", { name: "More actions" }));
    for (const label of overflow) {
      expect(screen.getByRole("menuitem", { name: label })).toBeTruthy();
    }
    expect(screen.getByRole("menuitem", { name: "Delete Dog facts" }).className).toContain(
      "destructive",
    );
  });

  it("imports generated JSON through the ordinary Import deck button", async () => {
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({
      id: "1".repeat(32),
      purpose: "study_content",
      detectedKind: "generated_json",
      displayName: "nclex.ostudy.json",
      mediaType: "application/octet-stream",
      byteSize: 128,
      sha256: "0".repeat(64),
      expiresAt: Date.now() + 60_000,
    });
    vi.mocked(generatedImport.importGeneratedFlashcards).mockResolvedValue({
      deckId: deck.id,
      createdNoteCount: 15,
      createdCardCount: 15,
      skippedDuplicateCount: 0,
    });
    const { showToast } = renderDecks();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(generatedImport.importGeneratedFlashcards).toHaveBeenCalledWith("1".repeat(32));
    });
    expect(decksIpc.importDeck).not.toHaveBeenCalled();
    expect(platformFiles.selectExternalImport).toHaveBeenCalledWith("study_content");
    expect(await screen.findByRole("heading", { name: "Dog facts" })).toBeTruthy();
    expect(showToast).toHaveBeenCalledWith(
      "success",
      "Imported 15 card entries as 15 scheduled cards.",
    );
  });

  it("continues to import studydeck packages through the same button", async () => {
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({
      id: "1".repeat(32),
      purpose: "study_content",
      detectedKind: "studydeck",
      displayName: "dog-facts.studydeck",
      mediaType: "application/octet-stream",
      byteSize: 128,
      sha256: "0".repeat(64),
      expiresAt: Date.now() + 60_000,
    });
    vi.mocked(decksIpc.importDeck).mockResolvedValue({ deck, cardCount: 2, assetCount: 0 });
    renderDecks();

    fireEvent.click(screen.getByRole("button", { name: "Import deck" }));

    await waitFor(() => {
      expect(decksIpc.importDeck).toHaveBeenCalledWith("1".repeat(32), importOperationId);
    });
    expect(commonIpc.createOperationId).toHaveBeenCalled();
    expect(generatedImport.importGeneratedFlashcards).not.toHaveBeenCalled();
  });
});
