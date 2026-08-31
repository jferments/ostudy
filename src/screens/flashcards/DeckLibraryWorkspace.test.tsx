import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ShowToast } from "../../components/Toast";
import type { DeckSummary } from "../../ipc/decks";
import * as folderIpc from "../../ipc/deckFolders";
import type { AppSettings } from "../../ipc/settings";
import type { SelectedPracticeRequest } from "../../ipc/practice";
import type { TransientDeckSelectionEntry } from "../../navigation/flashcardsRoutes";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { LayoutModeProvider } from "../../ui/layout/LayoutModeProvider";
import { DeckLibraryWorkspace } from "./DeckLibraryWorkspace";
import { DeckLibraryRoot } from "./deck-library/DeckLibraryRoot";
import type { DeckSelectionAnalysis } from "./deck-library/selectionModel";
import type { DeckLibraryController } from "./deck-library/useDeckLibraryController";

const deckControllerMocks = vi.hoisted(() => ({
  create: vi.fn(),
  setArchived: vi.fn(),
  deleteDeck: vi.fn(),
  undo: vi.fn(),
  invalidate: vi.fn(),
}));

vi.mock("../../controllers/decksController", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../controllers/decksController")>();
  return {
    ...actual,
    DecksController: class {
      invalidate = deckControllerMocks.invalidate;
      create = deckControllerMocks.create;
      setArchived = deckControllerMocks.setArchived;
      deleteDeck = deckControllerMocks.deleteDeck;
      undo = deckControllerMocks.undo;
    },
  };
});
vi.mock("../../ipc/deckFolders", () => ({
  getDeckLibraryOrganization: vi.fn(),
  createDeckFolder: vi.fn(),
  updateDeckFolder: vi.fn(),
  deleteDeckFolder: vi.fn(),
  moveDeckInLibrary: vi.fn(),
  moveDeckFolder: vi.fn(),
}));
vi.mock("../DecksScreen", () => ({
  DecksScreen: ({
    onViewChange,
  }: {
    onViewChange?: (view: "library" | "detail" | "editor") => void;
  }) => (
    <button
      type="button"
      onClick={() => {
        onViewChange?.("detail");
        onViewChange?.("library");
      }}
    >
      Return to deck library
    </button>
  ),
}));

const folder = {
  id: "f".repeat(32),
  name: "Nursing",
  sortOrder: 1000,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};
const pharmacology = deck("a", "Pharmacology", 24, 6);
const anatomy = deck("b", "Anatomy", 40, 12);
const organization = {
  revision: 4,
  folders: [folder],
  placements: [
    { deckId: pharmacology.id, folderId: folder.id, sortOrder: 1000 },
    { deckId: anatomy.id, folderId: null, sortOrder: 1000 },
  ],
};
const settings: AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  motionPreference: "system",
  materialPreference: "automatic",
  decorativeArtwork: true,
  celebrationsEnabled: true,
  tutorialsEnabled: true,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

interface RenderOptions {
  decks?: DeckSummary[];
  refreshDecks?: () => Promise<DeckSummary[]>;
  showToast?: ShowToast;
  openDeckRequest?: { requestId: number; deckId: string } | null;
  onOpenDeck?: (deckId: string) => void;
  onReturnToLibrary?: () => void;
  onPracticeDeck?: (deckId: string) => void;
  onPracticeSelected?: (
    request: SelectedPracticeRequest,
    selection: readonly TransientDeckSelectionEntry[],
  ) => void;
  layout?: "wide" | "compact";
}

function renderLibrary({
  decks = [pharmacology, anatomy],
  refreshDecks = vi.fn(async () => decks),
  showToast = vi.fn(),
  openDeckRequest = null,
  onOpenDeck = vi.fn(),
  onReturnToLibrary = vi.fn(),
  onPracticeDeck = vi.fn(),
  onPracticeSelected = vi.fn(),
  layout = "wide",
}: RenderOptions = {}) {
  setLayout(layout);
  return render(
    <LayoutModeProvider>
      <OverlayProvider>
        <DeckLibraryWorkspace
        decks={decks}
        settings={settings}
        refreshDecks={refreshDecks}
        onPracticeDeck={onPracticeDeck}
        onPracticeSelected={onPracticeSelected}
        openDeckRequest={openDeckRequest}
        onOpenDeck={onOpenDeck}
        onReturnToLibrary={onReturnToLibrary}
        onManageTemplates={vi.fn()}
        onCardDirtyChange={vi.fn()}
        showToast={showToast}
        />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
}

function setLayout(mode: "wide" | "compact"): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: query.includes("760") ? mode === "compact" : false,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

describe("DeckLibraryWorkspace", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    deckControllerMocks.create.mockResolvedValue(deck("c", "Created deck", 0, 0));
    deckControllerMocks.setArchived.mockImplementation(
      async ({ deckId, archived }: { deckId: string; archived: boolean }) => ({
        ...(deckId === pharmacology.id ? pharmacology : anatomy),
        archivedAt: archived ? Date.now() : null,
        revision: 2,
      }),
    );
    deckControllerMocks.deleteDeck.mockResolvedValue({
      affectedCount: 1,
      undoToken: "u".repeat(32),
      undoExpiresAt: Date.now() + 10_000,
      subjectLabel: "Deck",
    });
    deckControllerMocks.undo.mockResolvedValue({ restoredCount: 1 });
    vi.mocked(folderIpc.getDeckLibraryOrganization).mockResolvedValue(organization);
  });

  it("uses one compact organized-library presentation without a density selector", async () => {
    renderLibrary();

    const folderHeading = await screen.findByRole("heading", { name: "Folders" });
    const unsortedHeading = screen.getByRole("heading", { name: "Unsorted decks" });
    expect(
      Boolean(
        folderHeading.compareDocumentPosition(unsortedHeading) &
          Node.DOCUMENT_POSITION_FOLLOWING,
      ),
    ).toBe(true);
    const folderSection = folderHeading.closest("section");
    expect(folderSection).not.toBeNull();
    expect(
      within(folderSection as HTMLElement).getByRole("button", { name: "New folder" }),
    ).toBeTruthy();
    expect(screen.getByText("Pharmacology")).toBeTruthy();
    expect(screen.getByText("Anatomy")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Practice" })).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Delete Pharmacology" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Delete Anatomy" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Comfortable" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Compact" })).toBeNull();
    expect(screen.queryByText("Organize what you study")).toBeNull();
  });

  it("keeps the compact toolbar in the same visual, DOM, and keyboard order", async () => {
    renderLibrary({ layout: "compact" });

    const selectVisible = await screen.findByRole("button", { name: "Select visible" });
    const selectScope = screen.getByRole("button", { name: /Select all in current scope/u });
    const newDeck = screen.getByRole("button", { name: "New deck" });
    expect(selectVisible.compareDocumentPosition(selectScope) & Node.DOCUMENT_POSITION_FOLLOWING)
      .toBeTruthy();
    expect(selectScope.compareDocumentPosition(newDeck) & Node.DOCUMENT_POSITION_FOLLOWING)
      .toBeTruthy();
  });

  it("presents archived Delete directly outside compact layout", async () => {
    const archived = { ...deck("z", "Archived", 3, 0), archivedAt: 10 };
    renderLibrary({ decks: [pharmacology, anatomy, archived] });

    fireEvent.click(await screen.findByRole("button", { name: /Archived decks/u }));
    expect(screen.getByRole("button", { name: "Delete Archived" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Archived actions" })).toBeNull();
  });

  it("opens a deck through the explicit parent route callback", async () => {
    const onOpenDeck = vi.fn();
    renderLibrary({ onOpenDeck });

    const deckButton = (await screen.findByText("Pharmacology")).closest("button");
    expect(deckButton).not.toBeNull();
    fireEvent.click(deckButton as HTMLButtonElement);

    expect(onOpenDeck).toHaveBeenCalledWith(pharmacology.id);
  });

  it("keeps checkbox and action interactions separate from ordinary row opening", async () => {
    const onOpenDeck = vi.fn();
    renderLibrary({ onOpenDeck });

    const checkbox = await screen.findByRole("checkbox", { name: "Select Pharmacology" });
    const article = checkbox.closest("article");
    fireEvent.click(checkbox);
    expect(onOpenDeck).not.toHaveBeenCalled();
    expect((checkbox as HTMLInputElement).checked).toBe(true);
    expect(article?.getAttribute("data-selected")).toBe("true");

    const rowButton = screen.getByText("Pharmacology").closest("button");
    fireEvent.click(rowButton as HTMLButtonElement);
    expect(onOpenDeck).toHaveBeenCalledWith(pharmacology.id);
    expect((checkbox as HTMLInputElement).checked).toBe(true);
  });

  it("selects visible rows without collapsed content, selects the full lifecycle scope, and clears", async () => {
    renderLibrary();
    await screen.findByRole("checkbox", { name: "Select Pharmacology" });

    fireEvent.click(screen.getByRole("button", { name: "Collapse all" }));
    fireEvent.click(screen.getByRole("button", { name: "Select visible" }));
    expect(screen.getByText("1 item selected")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Expand Nursing folder" }));
    expect(
      (screen.getByRole("checkbox", { name: "Select Pharmacology" }) as HTMLInputElement)
        .checked,
    ).toBe(false);
    expect(
      (screen.getByRole("checkbox", { name: "Select Anatomy" }) as HTMLInputElement)
        .checked,
    ).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Select all in current scope (2)" }));
    expect(screen.getByText("2 items selected")).toBeTruthy();
    expect(
      (screen.getByRole("checkbox", { name: "Select Pharmacology" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Clear selection" }));
    expect(screen.queryByRole("region", { name: "Selected deck actions" })).toBeNull();
  });

  it("treats archived expansion as lifecycle scope without coupling folder collapse to selection", async () => {
    const archived = { ...deck("z", "Archived", 3, 0), archivedAt: 10 };
    renderLibrary({ decks: [pharmacology, anatomy, archived] });
    await screen.findByRole("button", { name: /Archived decks/u });

    expect(screen.getByRole("button", { name: "Select all in current scope (2)" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Archived decks/u }));
    expect(screen.getByRole("button", { name: "Select all in current scope (3)" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Select all in current scope (3)" }));
    expect(screen.getByText("3 items selected")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: /Archived decks/u }));
    await waitFor(() => expect(screen.getByText("2 items selected")).toBeTruthy());
    expect(
      (screen.getByRole("checkbox", { name: "Select Pharmacology" }) as HTMLInputElement)
        .checked,
    ).toBe(true);
  });

  it("keeps direct row Practice one-step and enables setup for valid multi-deck selection", async () => {
    const onPracticeDeck = vi.fn();
    const empty = deck("e", "Empty", 0, 0);
    renderLibrary({ decks: [pharmacology, empty], onPracticeDeck });

    const directPractice = await screen.findAllByRole("button", { name: "Practice" });
    expect(directPractice.some((button) => !button.hasAttribute("disabled"))).toBe(true);
    fireEvent.click(screen.getByRole("checkbox", { name: "Select Empty" }));
    expect(
      within(screen.getByRole("region", { name: "Selected deck actions" }))
        .getByRole("button", { name: "Practice" })
        .hasAttribute("disabled"),
    ).toBe(true);

    fireEvent.click(screen.getByRole("checkbox", { name: "Select Empty" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Select Pharmacology" }));
    const selectionPractice = within(
      screen.getByRole("region", { name: "Selected deck actions" }),
    ).getByRole("button", { name: "Practice" });
    expect(selectionPractice?.hasAttribute("disabled")).toBe(false);
    fireEvent.click(selectionPractice as HTMLButtonElement);
    expect(await screen.findByRole("heading", { name: "Practice selected decks" })).toBeTruthy();
    expect(onPracticeDeck).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Close dialog" }));

    fireEvent.click(screen.getByRole("checkbox", { name: "Select Empty" }));
    expect(
      within(screen.getByRole("region", { name: "Selected deck actions" }))
        .getByRole("button", { name: "Practice" })
        .hasAttribute("disabled"),
    ).toBe(true);
  });

  it("uses compact overflow for secondary row and folder actions while Practice stays direct", async () => {
    const archived = { ...deck("z", "Archived", 3, 0), archivedAt: 10 };
    renderLibrary({ layout: "compact", decks: [pharmacology, anatomy, archived] });

    expect((await screen.findAllByRole("button", { name: "Practice" })).length).toBe(2);
    expect(screen.queryByRole("button", { name: "Delete Pharmacology" })).toBeNull();
    const pharmacologyArticle = screen
      .getByRole("checkbox", { name: "Select Pharmacology" })
      .closest("article");
    fireEvent.click(
      within(pharmacologyArticle as HTMLElement).getByRole("button", {
        name: "Pharmacology actions",
      }),
    );
    expect(
      await screen.findByRole("menuitem", { name: "Delete Pharmacology" }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Close dialog" }));

    fireEvent.click(screen.getByRole("button", { name: /Archived decks/u }));
    expect(screen.queryByRole("button", { name: "Delete Archived" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Archived actions" }));
    expect(await screen.findByRole("menuitem", { name: "Delete Archived" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Close dialog" }));

    fireEvent.click(screen.getByRole("button", { name: "Nursing folder actions" }));
    expect(
      await screen.findByRole("menuitem", { name: "Rename Nursing folder" }),
    ).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Delete Nursing folder" })).toBeTruthy();
  });

  it("keeps selected Practice direct and moves secondary bulk actions into compact overflow", async () => {
    renderLibrary({ layout: "compact" });
    fireEvent.click(await screen.findByRole("checkbox", { name: "Select Pharmacology" }));

    const toolbar = screen.getByRole("region", { name: "Selected deck actions" });
    expect(within(toolbar).getByRole("button", { name: "Practice" })).toBeTruthy();
    expect(within(toolbar).queryByRole("button", { name: "Move selected" })).toBeNull();
    fireEvent.click(within(toolbar).getByRole("button", { name: "More selection actions" }));
    expect(await screen.findByRole("menuitem", { name: "Move selected" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Archive selected" })).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: "Delete selected" })).toBeTruthy();
  });

  it("disables bulk mutation controls while selected entries are stale", async () => {
    const { controller } = renderSelectionRoot({
      staleIds: [pharmacology.id],
    });
    const toolbar = screen.getByRole("region", { name: "Selected deck actions" });

    expect(
      within(toolbar).getByRole("button", { name: "Move selected" }).hasAttribute("disabled"),
    ).toBe(true);
    expect(
      within(toolbar).getByRole("button", { name: "Archive selected" }).hasAttribute("disabled"),
    ).toBe(true);
    fireEvent.click(within(toolbar).getByRole("button", { name: "More selection actions" }));
    const deleteItem = screen.getByRole("menuitem", { name: "Delete selected" });
    expect(deleteItem.hasAttribute("disabled")).toBe(true);
    fireEvent.click(deleteItem);
    expect(controller.deleteSelection).not.toHaveBeenCalled();
    expect(screen.getByText(/changed after selection/u)).toBeTruthy();
  });

  it("disables bulk mutation controls while selected entries are missing", async () => {
    const archived = { ...deck("z", "Archived", 3, 0), archivedAt: 10 };
    const { controller } = renderSelectionRoot({
      selectedIds: [archived.id, "missing-deck"],
      decks: [archived],
      missingIds: ["missing-deck"],
      activeIds: [],
      archivedIds: [archived.id],
      generatedIds: [archived.id],
    });
    const toolbar = screen.getByRole("region", { name: "Selected deck actions" });

    expect(
      within(toolbar).getByRole("button", { name: "Move selected" }).hasAttribute("disabled"),
    ).toBe(true);
    expect(
      within(toolbar).getByRole("button", { name: "Restore selected" }).hasAttribute("disabled"),
    ).toBe(true);
    fireEvent.click(within(toolbar).getByRole("button", { name: "More selection actions" }));
    const deleteItem = screen.getByRole("menuitem", { name: "Delete selected" });
    expect(deleteItem.hasAttribute("disabled")).toBe(true);
    fireEvent.click(deleteItem);
    expect(controller.deleteSelection).not.toHaveBeenCalled();
  });

  it("confirms the valid lifecycle subset of a mixed active and archived selection", async () => {
    const archived = { ...deck("z", "Archived", 3, 0), archivedAt: 10 };
    renderLibrary({ decks: [pharmacology, archived] });
    fireEvent.click(await screen.findByRole("button", { name: /Archived decks/u }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Select Pharmacology" }));
    fireEvent.click(screen.getByRole("checkbox", { name: "Select Archived" }));

    expect(screen.getByRole("button", { name: "Move selected" }).hasAttribute("disabled")).toBe(
      true,
    );
    fireEvent.click(screen.getByRole("button", { name: "Archive active selected" }));
    const dialog = await screen.findByRole("alertdialog", {
      name: "Archive 1 selected deck?",
    });
    expect(dialog.textContent).toContain("1 already archived selected deck remains unchanged");
    fireEvent.click(within(dialog).getByRole("button", { name: "Archive selected" }));

    await waitFor(() =>
      expect(deckControllerMocks.setArchived).toHaveBeenCalledWith({
        deckId: pharmacology.id,
        revision: pharmacology.revision,
        archived: true,
      }),
    );
    expect(deckControllerMocks.setArchived).toHaveBeenCalledTimes(1);
  });

  it("returns nested deck work through the explicit library-root callback", async () => {
    const onReturnToLibrary = vi.fn();
    renderLibrary({
      openDeckRequest: { requestId: 7, deckId: pharmacology.id },
      onReturnToLibrary,
    });

    fireEvent.click(await screen.findByRole("button", { name: "Return to deck library" }));
    expect(onReturnToLibrary).toHaveBeenCalledTimes(1);
  });

  it("requires confirmation before deleting a deck", async () => {
    vi.mocked(folderIpc.getDeckLibraryOrganization)
      .mockResolvedValueOnce(organization)
      .mockResolvedValue({
        revision: 5,
        folders: [folder],
        placements: [{ deckId: anatomy.id, folderId: null, sortOrder: 1000 }],
      });
    const refreshDecks = vi.fn(async () => [anatomy]);
    renderLibrary({ refreshDecks });

    const deleteButton = await screen.findByRole("button", { name: "Delete Pharmacology" });
    expect(deleteButton.getAttribute("data-tone")).toBe("danger");
    fireEvent.focus(deleteButton);
    expect(screen.getByRole("tooltip").textContent).toBe("Delete Pharmacology");
    fireEvent.click(deleteButton);
    const dialog = await screen.findByRole("alertdialog", { name: "Delete “Pharmacology”?" });
    expect(deckControllerMocks.deleteDeck).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete deck" }));

    await waitFor(() =>
      expect(deckControllerMocks.deleteDeck).toHaveBeenCalledWith(pharmacology.id),
    );
    await waitFor(() => expect(refreshDecks).toHaveBeenCalled());
  });

  it("collapses individual folders and exposes a collapse-all control", async () => {
    renderLibrary();

    const collapse = await screen.findByRole("button", { name: "Collapse Nursing folder" });
    expect(collapse.getAttribute("title")).toBe("Collapse folder");
    fireEvent.click(screen.getByRole("button", { name: "Collapse all" }));

    const expand = screen.getByRole("button", { name: "Expand Nursing folder" });
    expect(expand.getAttribute("aria-expanded")).toBe("false");
    expect(expand.getAttribute("title")).toBe("Expand folder");
    expect(screen.queryByText("Pharmacology")).toBeNull();

    fireEvent.click(expand);
    expect(screen.getByText("Pharmacology")).toBeTruthy();
  });

  it("moves a selected deck through the shared organization operation", async () => {
    vi.mocked(folderIpc.moveDeckInLibrary).mockResolvedValue({
      revision: 5,
      folders: [folder],
      placements: [
        { deckId: pharmacology.id, folderId: folder.id, sortOrder: 1000 },
        { deckId: anatomy.id, folderId: folder.id, sortOrder: 2000 },
      ],
    });
    renderLibrary();

    fireEvent.click(await screen.findByRole("checkbox", { name: "Select Anatomy" }));
    fireEvent.click(screen.getByRole("button", { name: "Move selected" }));
    const dialog = await screen.findByRole("dialog", { name: "Move selected decks" });
    expect(dialog.textContent).toContain(
      "If one deck can’t be moved, decks already moved stay in their new location",
    );
    fireEvent.change(within(dialog).getByRole("combobox", { name: "Destination" }), {
      target: { value: folder.id },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Move selected" }));

    await waitFor(() =>
      expect(folderIpc.moveDeckInLibrary).toHaveBeenCalledWith({
        deckId: anatomy.id,
        targetFolderId: folder.id,
        targetIndex: 1,
        expectedOrganizationRevision: 4,
      }),
    );
  });

  it("reorders folders with the retained keyboard alternative", async () => {
    const secondFolder = {
      ...folder,
      id: "g".repeat(32),
      name: "Clinical",
      sortOrder: 2000,
    };
    const twoFolderOrganization = {
      revision: 4,
      folders: [folder, secondFolder],
      placements: [
        { deckId: pharmacology.id, folderId: folder.id, sortOrder: 1000 },
        { deckId: anatomy.id, folderId: secondFolder.id, sortOrder: 1000 },
      ],
    };
    vi.mocked(folderIpc.getDeckLibraryOrganization).mockResolvedValue(twoFolderOrganization);
    vi.mocked(folderIpc.moveDeckFolder).mockResolvedValue({
      ...twoFolderOrganization,
      revision: 5,
      folders: [secondFolder, folder],
    });
    renderLibrary();

    fireEvent.keyDown(
      await screen.findByRole("button", { name: /Reorder Nursing folder\./u }),
      { key: "ArrowDown" },
    );

    await waitFor(() =>
      expect(folderIpc.moveDeckFolder).toHaveBeenCalledWith({
        folderId: folder.id,
        targetIndex: 1,
        expectedOrganizationRevision: 4,
      }),
    );
  });

  it("reloads the authoritative revision after a stale organization conflict", async () => {
    const refreshed = { ...organization, revision: 5 };
    vi.mocked(folderIpc.getDeckLibraryOrganization)
      .mockResolvedValueOnce(organization)
      .mockResolvedValueOnce(refreshed);
    vi.mocked(folderIpc.moveDeckInLibrary).mockRejectedValue({
      code: "library_organization_changed",
      message: "The deck library organization changed. Reload it and try again.",
    });
    vi.mocked(folderIpc.createDeckFolder).mockResolvedValue({
      revision: 6,
      folders: [folder, { ...folder, id: "g".repeat(32), name: "Second", sortOrder: 2000 }],
      placements: organization.placements,
    });
    renderLibrary();

    fireEvent.click(await screen.findByRole("checkbox", { name: "Select Anatomy" }));
    fireEvent.click(screen.getByRole("button", { name: "Move selected" }));
    const moveDialog = await screen.findByRole("dialog", { name: "Move selected decks" });
    fireEvent.change(within(moveDialog).getByRole("combobox", { name: "Destination" }), {
      target: { value: folder.id },
    });
    fireEvent.click(within(moveDialog).getByRole("button", { name: "Move selected" }));
    await waitFor(() => expect(folderIpc.getDeckLibraryOrganization).toHaveBeenCalledTimes(2));

    fireEvent.click(screen.getByRole("button", { name: "New folder" }));
    const dialog = await screen.findByRole("dialog", { name: "Create a folder" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Folder name" }), {
      target: { value: "Second" },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Create folder" }));

    await waitFor(() =>
      expect(folderIpc.createDeckFolder).toHaveBeenCalledWith({
        name: "Second",
        expectedOrganizationRevision: 5,
      }),
    );
  });
});

function renderSelectionRoot(overrides: Partial<DeckSelectionAnalysis>) {
  const analysis: DeckSelectionAnalysis = {
    selectedIds: [pharmacology.id],
    decks: [pharmacology],
    missingIds: [],
    staleIds: [],
    activeIds: [pharmacology.id],
    archivedIds: [],
    emptyIds: [],
    generatedIds: [pharmacology.id],
    isMixedLifecycle: false,
    ...overrides,
  };
  const controller = {
    organization: null,
    snapshot: null,
    isLoading: false,
    isMutating: false,
    errorMessage: null,
    announcement: "",
    sheet: null,
    showArchived: false,
    collapsedFolderIds: new Set<string>(),
    allFoldersCollapsed: false,
    selection: { entries: new Map() },
    selectedIds: new Set(analysis.selectedIds),
    selectionAnalysis: analysis,
    visibleDeckCount: 0,
    currentScopeDeckCount: 0,
    reloadOrganization: vi.fn(),
    refreshLibraryAfterDetail: vi.fn(),
    openDeck: vi.fn(),
    openDeckCreator: vi.fn(),
    openFolderCreator: vi.fn(),
    openFolderRenamer: vi.fn(),
    closeSheet: vi.fn(),
    createDeck: vi.fn(),
    saveFolder: vi.fn(),
    toggleDeckSelected: vi.fn(),
    selectVisible: vi.fn(),
    selectCurrentScope: vi.fn(),
    clearSelection: vi.fn(),
    openSelectionMover: vi.fn(),
    openPracticeSetup: vi.fn(),
    moveSelection: vi.fn(),
    setSelectionArchived: vi.fn(),
    deleteSelection: vi.fn(),
    removeDeck: vi.fn(),
    removeFolder: vi.fn(),
    move: vi.fn(),
    toggleFolder: vi.fn(),
    collapseAllFolders: vi.fn(),
    toggleArchived: vi.fn(),
  } as unknown as DeckLibraryController;
  setLayout("wide");
  render(
    <LayoutModeProvider>
      <OverlayProvider>
        <DeckLibraryRoot
          controller={controller}
          onPracticeDeck={vi.fn()}
          decks={[pharmacology]}
          practiceShuffle
          onPracticeSelected={vi.fn()}
        />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
  return { controller };
}

function deck(
  seed: string,
  name: string,
  generatedCardCount: number,
  due: number,
): DeckSummary {
  return {
    id: seed.repeat(32),
    name,
    description: `${name} study deck`,
    sourceNoteCount: generatedCardCount,
    generatedCardCount,
    studyableDueCount: due,
    archivedAt: null,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}
