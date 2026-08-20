import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ShowToast } from "../../components/Toast";
import type { DeckSummary } from "../../ipc/decks";
import * as folderIpc from "../../ipc/deckFolders";
import type { AppSettings } from "../../ipc/settings";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { DeckLibraryWorkspace } from "./DeckLibraryWorkspace";

const deckControllerMocks = vi.hoisted(() => ({
  create: vi.fn(),
  deleteDeck: vi.fn(),
  undo: vi.fn(),
  invalidate: vi.fn(),
}));

vi.mock("../../controllers/decksController", () => ({
  DecksController: class {
    invalidate = deckControllerMocks.invalidate;
    create = deckControllerMocks.create;
    deleteDeck = deckControllerMocks.deleteDeck;
    undo = deckControllerMocks.undo;
  },
}));
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
}

function renderLibrary({
  decks = [pharmacology, anatomy],
  refreshDecks = vi.fn(async () => decks),
  showToast = vi.fn(),
  openDeckRequest = null,
  onOpenDeck = vi.fn(),
  onReturnToLibrary = vi.fn(),
}: RenderOptions = {}) {
  return render(
    <OverlayProvider>
      <DeckLibraryWorkspace
        decks={decks}
        settings={settings}
        refreshDecks={refreshDecks}
        onPracticeDeck={vi.fn()}
        openDeckRequest={openDeckRequest}
        onOpenDeck={onOpenDeck}
        onReturnToLibrary={onReturnToLibrary}
        onManageTemplates={vi.fn()}
        onCardDirtyChange={vi.fn()}
        showToast={showToast}
      />
    </OverlayProvider>,
  );
}

describe("DeckLibraryWorkspace", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    deckControllerMocks.create.mockResolvedValue(deck("c", "Created deck", 0, 0));
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

  it("opens a deck through the explicit parent route callback", async () => {
    const onOpenDeck = vi.fn();
    renderLibrary({ onOpenDeck });

    const deckButton = (await screen.findByText("Pharmacology")).closest("button");
    expect(deckButton).not.toBeNull();
    fireEvent.click(deckButton as HTMLButtonElement);

    expect(onOpenDeck).toHaveBeenCalledWith(pharmacology.id);
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

    fireEvent.click(await screen.findByRole("button", { name: "Delete Pharmacology" }));
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

  it("moves a deck between groups with the drag-handle keyboard alternative", async () => {
    vi.mocked(folderIpc.moveDeckInLibrary).mockResolvedValue({
      revision: 5,
      folders: [folder],
      placements: [
        { deckId: pharmacology.id, folderId: folder.id, sortOrder: 1000 },
        { deckId: anatomy.id, folderId: folder.id, sortOrder: 2000 },
      ],
    });
    renderLibrary();

    fireEvent.keyDown(await screen.findByRole("button", { name: /Reorder Anatomy\./u }), {
      key: "ArrowLeft",
    });

    await waitFor(() =>
      expect(folderIpc.moveDeckInLibrary).toHaveBeenCalledWith({
        deckId: anatomy.id,
        targetFolderId: folder.id,
        targetIndex: 1,
        expectedOrganizationRevision: 4,
      }),
    );
  });

  it("reorders folders with the drag-handle keyboard alternative", async () => {
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

    fireEvent.keyDown(await screen.findByRole("button", { name: /Reorder Anatomy\./u }), {
      key: "ArrowLeft",
    });
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
