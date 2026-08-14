import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ShowToast } from "../../components/Toast";
import type { DeckSummary } from "../../ipc/decks";
import * as folderIpc from "../../ipc/deckFolders";
import * as settingsIpc from "../../ipc/settings";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { DeckLibraryWorkspace } from "./DeckLibraryWorkspace";

vi.mock("../../controllers/decksController", () => ({
  DecksController: class {
    invalidate = vi.fn();
    create = vi.fn();
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
vi.mock("../../ipc/settings", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../ipc/settings")>();
  return { ...actual, saveSettings: vi.fn() };
});
vi.mock("../DecksScreen", () => ({
  DecksScreen: ({ onViewChange }: { onViewChange?: (view: "library" | "detail" | "editor") => void }) => (
    <div>
      <button
        type="button"
        onClick={() => {
          onViewChange?.("detail");
          onViewChange?.("library");
        }}
      >
        Return to deck library
      </button>
    </div>
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
const settings = {
  theme: "dark" as const,
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  motionPreference: "system" as const,
  materialPreference: "automatic" as const,
  decorativeArtwork: true,
  celebrationsEnabled: true,
  tutorialsEnabled: true,
  deckViewDensity: "comfortable" as const,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

function renderLibrary({
  refreshDecks = vi.fn(async () => [pharmacology, anatomy]),
  showToast = vi.fn(),
}: {
  refreshDecks?: () => Promise<DeckSummary[]>;
  showToast?: ShowToast;
} = {}) {
  return render(
    <OverlayProvider>
      <DeckLibraryWorkspace
        decks={[pharmacology, anatomy]}
        settings={settings}
        refreshDecks={refreshDecks}
        onPracticeDeck={vi.fn()}
        openDeckRequest={null}
        onOpenDeckRequestConsumed={vi.fn()}
        onManageTemplates={vi.fn()}
        onCreateCards={vi.fn()}
        onCardDirtyChange={vi.fn()}
        showToast={showToast}
      />
    </OverlayProvider>,
  );
}

describe("DeckLibraryWorkspace", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(folderIpc.getDeckLibraryOrganization).mockResolvedValue(organization);
    vi.mocked(settingsIpc.saveSettings).mockImplementation(async (input) => input);
  });

  it("renders folders before loose decks and keeps both deck groups studyable", async () => {
    renderLibrary();

    const folderHeading = await screen.findByRole("heading", { name: "Folders" });
    const unfiledHeading = screen.getByRole("heading", { name: "Unfiled decks" });
    expect(
      Boolean(
        folderHeading.compareDocumentPosition(unfiledHeading) &
          Node.DOCUMENT_POSITION_FOLLOWING,
      ),
    ).toBe(true);
    expect(screen.getByText("Pharmacology")).toBeTruthy();
    expect(screen.getByText("Anatomy")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: "Practice" })).toHaveLength(2);
  });

  it("moves a deck through the same revisioned command used by drag and keyboard actions", async () => {
    const moved = {
      revision: 5,
      folders: [folder],
      placements: [
        { deckId: pharmacology.id, folderId: folder.id, sortOrder: 1000 },
        { deckId: anatomy.id, folderId: folder.id, sortOrder: 2000 },
      ],
    };
    vi.mocked(folderIpc.moveDeckInLibrary).mockResolvedValue(moved);
    renderLibrary();

    const anatomyRow = (await screen.findByText("Anatomy")).closest("article");
    expect(anatomyRow).not.toBeNull();
    fireEvent.click(within(anatomyRow as HTMLElement).getByRole("button", { name: "Move" }));
    const dialog = await screen.findByRole("dialog", { name: "Move “Anatomy”" });
    fireEvent.change(within(dialog).getByRole("combobox", { name: "Destination" }), {
      target: { value: folder.id },
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Move deck" }));

    await waitFor(() =>
      expect(folderIpc.moveDeckInLibrary).toHaveBeenCalledWith({
        deckId: anatomy.id,
        targetFolderId: folder.id,
        targetIndex: 1,
        expectedOrganizationRevision: 4,
      }),
    );
  });

  it("stops a deck-row drop from bubbling into the parent folder drop target", async () => {
    const moved = {
      revision: 5,
      folders: [folder],
      placements: [
        { deckId: anatomy.id, folderId: folder.id, sortOrder: 1000 },
        { deckId: pharmacology.id, folderId: folder.id, sortOrder: 2000 },
      ],
    };
    vi.mocked(folderIpc.moveDeckInLibrary).mockResolvedValue(moved);
    renderLibrary();

    const pharmacologyRow = (await screen.findByText("Pharmacology")).closest("article");
    expect(pharmacologyRow).not.toBeNull();
    fireEvent.drop(pharmacologyRow as HTMLElement, {
      dataTransfer: {
        getData: () => anatomy.id,
      },
    });

    await waitFor(() => expect(folderIpc.moveDeckInLibrary).toHaveBeenCalledTimes(1));
    expect(folderIpc.moveDeckInLibrary).toHaveBeenCalledWith({
      deckId: anatomy.id,
      targetFolderId: folder.id,
      targetIndex: 0,
      expectedOrganizationRevision: 4,
    });
  });

  it("reloads the authoritative revision after a stale organization conflict", async () => {
    const refreshed = { ...organization, revision: 5 };
    const created = {
      revision: 6,
      folders: [folder, { ...folder, id: "g".repeat(32), name: "Second", sortOrder: 2000 }],
      placements: organization.placements,
    };
    vi.mocked(folderIpc.getDeckLibraryOrganization)
      .mockResolvedValueOnce(organization)
      .mockResolvedValueOnce(refreshed);
    vi.mocked(folderIpc.moveDeckInLibrary).mockRejectedValue({
      code: "library_organization_changed",
      message: "The deck library organization changed. Reload it and try again.",
    });
    vi.mocked(folderIpc.createDeckFolder).mockResolvedValue(created);
    renderLibrary();

    const anatomyRow = (await screen.findByText("Anatomy")).closest("article");
    expect(anatomyRow).not.toBeNull();
    fireEvent.click(within(anatomyRow as HTMLElement).getByRole("button", { name: "Move" }));
    const moveDialog = await screen.findByRole("dialog", { name: "Move “Anatomy”" });
    fireEvent.change(within(moveDialog).getByRole("combobox", { name: "Destination" }), {
      target: { value: folder.id },
    });
    fireEvent.click(within(moveDialog).getByRole("button", { name: "Move deck" }));

    await waitFor(() => expect(folderIpc.getDeckLibraryOrganization).toHaveBeenCalledTimes(2));
    fireEvent.click(within(moveDialog).getByRole("button", { name: "Cancel" }));
    fireEvent.click(screen.getByRole("button", { name: "New folder" }));
    const folderDialog = await screen.findByRole("dialog", { name: "Create a folder" });
    fireEvent.change(within(folderDialog).getByRole("textbox", { name: "Folder name" }), {
      target: { value: "Second" },
    });
    fireEvent.click(within(folderDialog).getByRole("button", { name: "Create folder" }));

    await waitFor(() =>
      expect(folderIpc.createDeckFolder).toHaveBeenCalledWith({
        name: "Second",
        expectedOrganizationRevision: 5,
      }),
    );
  });

  it("refreshes decks and organization after returning from nested deck work", async () => {
    const refreshDecks = vi.fn(async () => [pharmacology, anatomy]);
    vi.mocked(folderIpc.getDeckLibraryOrganization)
      .mockResolvedValueOnce(organization)
      .mockResolvedValueOnce({ ...organization, revision: 5 });
    renderLibrary({ refreshDecks });

    const deckButton = (await screen.findByText("Pharmacology")).closest("button");
    expect(deckButton).not.toBeNull();
    fireEvent.click(deckButton as HTMLButtonElement);
    fireEvent.click(await screen.findByRole("button", { name: "Return to deck library" }));

    await waitFor(() => expect(refreshDecks).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(folderIpc.getDeckLibraryOrganization).toHaveBeenCalledTimes(2));
  });

  it("persists compact density as a presentation preference", async () => {
    renderLibrary();
    fireEvent.click(await screen.findByRole("button", { name: "Compact" }));
    await waitFor(() =>
      expect(settingsIpc.saveSettings).toHaveBeenCalledWith({
        ...settings,
        deckViewDensity: "compact",
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
