import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ShowToast } from "../../components/Toast";
import type { DeckSummary } from "../../ipc/decks";
import * as folderIpc from "../../ipc/deckFolders";
import type { SelectedPracticeRequest } from "../../ipc/practice";
import type { AppSettings } from "../../ipc/settings";
import type {
  SubjectFocusLaunch,
  TransientDeckSelectionEntry,
} from "../../navigation/flashcardsRoutes";
import { LayoutModeProvider } from "../../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { DeckLibraryWorkspace } from "./DeckLibraryWorkspace";
import { matchesLayoutQuery, layoutWidths } from "../../test/viewport";

const deckControllerMocks = vi.hoisted(() => ({
  create: vi.fn(),
  setArchived: vi.fn(),
  merge: vi.fn(),
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
      merge = deckControllerMocks.merge;
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

vi.mock("./DeckDetailRoute", () => ({
  DeckDetailRoute: ({
    openRequest,
    onReturnToLibrary,
  }: {
    openRequest: { requestId: number; deckId: string };
    onReturnToLibrary: () => void;
  }) => (
    <div data-testid="deck-detail-workspace">
      <span>Deck card list for {openRequest.deckId}</span>
      <button type="button" onClick={onReturnToLibrary}>
        Return to deck library
      </button>
    </div>
  ),
}));

const folder = {
  id: "f".repeat(32),
  name: "Nursing",
  sortOrder: 1_000,
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
    { deckId: pharmacology.id, folderId: folder.id, sortOrder: 1_000 },
    { deckId: anatomy.id, folderId: null, sortOrder: 1_000 },
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
  onOpenDeck?: (
    deckId: string,
    selection?: readonly TransientDeckSelectionEntry[],
  ) => void;
  onReturnToLibrary?: () => void;
  onPracticeDeck?: (deckId: string) => void;
  onPracticeSelected?: (
    request: SelectedPracticeRequest,
    selection: readonly TransientDeckSelectionEntry[],
  ) => void;
  layout?: "wide" | "compact";
  subjectFocus?: SubjectFocusLaunch;
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
  subjectFocus,
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
          onCardDirtyChange={vi.fn()}
          showToast={showToast}
          subjectFocus={subjectFocus}
        />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
}

function setLayout(mode: "wide" | "compact"): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: matchesLayoutQuery(query, layoutWidths[mode]),
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
    deckControllerMocks.merge.mockImplementation(async (input: {
      operationId: string;
      sourceDeckId: string;
      destinationDeckId: string;
    }) => ({
      operationId: input.operationId,
      sourceDeckId: input.sourceDeckId,
      destinationDeckId: input.destinationDeckId,
      movedNoteCount: 24,
      movedCardCount: 24,
      retargetedDocumentCount: 1,
      retargetedStudyTargetCount: 0,
      sourceRevision: 2,
      destinationRevision: 2,
      sourceArchivedAt: 50,
      mergedAt: 50,
    }));
    deckControllerMocks.deleteDeck.mockResolvedValue({
      affectedCount: 1,
      undoToken: "u".repeat(32),
      undoExpiresAt: Date.now() + 10_000,
      subjectLabel: "Deck",
    });
    deckControllerMocks.undo.mockResolvedValue({ restoredCount: 1 });
    vi.mocked(folderIpc.getDeckLibraryOrganization).mockResolvedValue(organization);
    vi.mocked(folderIpc.createDeckFolder).mockResolvedValue(organization);
    vi.mocked(folderIpc.updateDeckFolder).mockResolvedValue(organization);
    vi.mocked(folderIpc.deleteDeckFolder).mockResolvedValue(organization);
    vi.mocked(folderIpc.moveDeckInLibrary).mockResolvedValue(organization);
    vi.mocked(folderIpc.moveDeckFolder).mockResolvedValue(organization);
  });

  it("renders the organized deck library with Practice direct and deck maintenance in menus", async () => {
    renderLibrary();

    const folders = await screen.findByRole("heading", { name: "Folders" });
    const unsorted = screen.getByRole("heading", { name: "Unsorted decks" });
    expect(folders.compareDocumentPosition(unsorted) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByText("Pharmacology")).toBeTruthy();
    expect(screen.getByText("Anatomy")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Import deck" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Import deck from file" })).toBeNull();
    expect(screen.getAllByRole("button", { name: "Practice" })).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Pharmacology actions" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Anatomy actions" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Delete Pharmacology" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Delete Anatomy" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Open" })).toBeNull();
    expect(screen.queryByText("Organize what you study")).toBeNull();
  });

  it("opens the clicked deck through the explicit detail route and preserves selection", async () => {
    const onOpenDeck = vi.fn();
    renderLibrary({ onOpenDeck });

    const openSurface = await screen.findByRole("button", { name: "Open Pharmacology" });
    fireEvent.click(openSurface);
    expect(onOpenDeck).toHaveBeenLastCalledWith(pharmacology.id, []);

    fireEvent.click(screen.getByRole("checkbox", { name: "Select Pharmacology" }));
    fireEvent.click(screen.getByText("24 cards · 6 ready"));
    expect(onOpenDeck).toHaveBeenLastCalledWith(
      pharmacology.id,
      [{ id: pharmacology.id, revision: pharmacology.revision }],
    );
    expect(
      screen.getByRole("checkbox", { name: "Select Pharmacology" }).getAttribute("aria-checked"),
    ).toBe("true");
  });

  it("mounts the route-owned card workspace immediately without a loading gate", async () => {
    renderLibrary({
      openDeckRequest: { requestId: 7, deckId: pharmacology.id },
    });

    expect(
      await screen.findByText(`Deck card list for ${pharmacology.id}`),
    ).toBeTruthy();
    expect(screen.getByTestId("deck-detail-workspace")).toBeTruthy();
    expect(screen.queryByText("Opening deck cards…")).toBeNull();
    expect(screen.queryByRole("heading", { name: "Decks" })).toBeNull();
    expect(screen.queryByText("Pharmacology study deck")).toBeNull();
  });

  it("uses exact direct bulk labels and icons without a one-item ellipsis menu", async () => {
    renderLibrary();
    fireEvent.click(await screen.findByRole("checkbox", { name: "Select Pharmacology" }));

    const toolbar = screen.getByRole("region", { name: "Selected deck actions" });
    expect(within(toolbar).getByRole("button", { name: "Practice" })).toBeTruthy();
    expect(within(toolbar).getByRole("button", { name: "Move" })).toBeTruthy();
    const archive = within(toolbar).getByRole("button", { name: "Archive selected" });
    const remove = within(toolbar).getByRole("button", { name: "Delete selected" });
    expect(archive.querySelector("svg")).not.toBeNull();
    expect(remove.querySelector("svg")).not.toBeNull();
    expect(remove.getAttribute("data-tone")).toBe("danger");
    expect(within(toolbar).queryByRole("button", { name: "Move selected" })).toBeNull();
    expect(within(toolbar).queryByRole("button", { name: "More selection actions" })).toBeNull();
  });

  it("keeps Practice direct and the deck menu compact while preserving the folder menu", async () => {
    renderLibrary({ layout: "compact" });

    expect((await screen.findAllByRole("button", { name: "Practice" })).length).toBe(2);
    expect(screen.getByRole("button", { name: "Pharmacology actions" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Anatomy actions" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Delete Pharmacology" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Nursing folder actions" }));
    const sheet = await screen.findByRole("dialog", { name: "Nursing folder actions" });
    expect(within(sheet).getByRole("button", { name: "Rename Nursing folder" })).toBeTruthy();
    expect(within(sheet).getByRole("button", { name: "Delete Nursing folder" })).toBeTruthy();
  });

  it("places an icon-only collapse caret directly beside the folder title", async () => {
    renderLibrary();

    const heading = await screen.findByRole("heading", { name: "Nursing" });
    const collapse = screen.getByRole("button", { name: "Collapse Nursing folder" });
    expect(collapse.textContent?.trim()).toBe("");
    expect(collapse.querySelector("svg")).not.toBeNull();
    expect(heading.parentElement?.contains(collapse)).toBe(true);
    fireEvent.click(collapse);

    const expand = screen.getByRole("button", { name: "Expand Nursing folder" });
    expect(expand.textContent?.trim()).toBe("");
    expect(expand.getAttribute("aria-expanded")).toBe("false");
    expect(screen.queryByText("Pharmacology")).toBeNull();
    fireEvent.click(expand);
    expect(screen.getByText("Pharmacology")).toBeTruthy();
  });

  it("keeps direct row Practice isolated from deck opening", async () => {
    const onPracticeDeck = vi.fn();
    const onOpenDeck = vi.fn();
    renderLibrary({ onPracticeDeck, onOpenDeck });

    const article = (await screen.findByRole("checkbox", { name: "Select Pharmacology" })).closest(
      "article",
    );
    fireEvent.click(within(article as HTMLElement).getByRole("button", { name: "Practice" }));
    expect(onPracticeDeck).toHaveBeenCalledWith(pharmacology.id);
    expect(onOpenDeck).not.toHaveBeenCalled();
  });

  it("opens Merge from the deck menu and submits exact source/destination revisions and counts", async () => {
    const refreshDecks = vi.fn(async () => [pharmacology, anatomy]);
    renderLibrary({ refreshDecks });

    fireEvent.click(await screen.findByRole("button", { name: "Pharmacology actions" }));
    const menu = await screen.findByRole("menu", { name: "Pharmacology actions" });
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Merge…" }));

    const sheet = await screen.findByRole("dialog", { name: "Merge “Pharmacology”" });
    const destinationSelect = within(sheet).getByRole(
      "combobox",
      { name: "Destination deck" },
    ) as HTMLSelectElement;
    expect(destinationSelect.value).toBe(anatomy.id);
    expect(sheet.textContent).toContain("24 cards will move");
    fireEvent.click(within(sheet).getByRole("button", { name: "Merge decks" }));

    await waitFor(() => expect(deckControllerMocks.merge).toHaveBeenCalledTimes(1));
    expect(deckControllerMocks.merge).toHaveBeenCalledWith({
      operationId: expect.any(String),
      sourceDeckId: pharmacology.id,
      destinationDeckId: anatomy.id,
      sourceRevision: pharmacology.revision,
      destinationRevision: anatomy.revision,
      expectedSourceNoteCount: pharmacology.sourceNoteCount,
      expectedSourceCardCount: pharmacology.generatedCardCount,
    });
    const operationId = deckControllerMocks.merge.mock.calls[0]?.[0].operationId;
    expect(operationId).toHaveLength(32);
    await waitFor(() => expect(refreshDecks).toHaveBeenCalledTimes(1));
  });

  it("moves selected decks through the shared revision-checked organization operation", async () => {
    vi.mocked(folderIpc.moveDeckInLibrary).mockResolvedValue({
      revision: 5,
      folders: [folder],
      placements: [
        { deckId: pharmacology.id, folderId: folder.id, sortOrder: 1_000 },
        { deckId: anatomy.id, folderId: folder.id, sortOrder: 2_000 },
      ],
    });
    renderLibrary();

    fireEvent.click(await screen.findByRole("checkbox", { name: "Select Anatomy" }));
    fireEvent.click(screen.getByRole("button", { name: "Move" }));
    const dialog = await screen.findByRole("dialog", { name: "Move selected decks" });
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

  it("requires confirmation before the direct trash action deletes a deck", async () => {
    const refreshDecks = vi.fn(async () => [anatomy]);
    renderLibrary({ refreshDecks });

    fireEvent.click(await screen.findByRole("button", { name: "Pharmacology actions" }));
    const menu = await screen.findByRole("menu", { name: "Pharmacology actions" });
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete Pharmacology" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Delete “Pharmacology”?" });
    expect(deckControllerMocks.deleteDeck).not.toHaveBeenCalled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete deck" }));

    await waitFor(() =>
      expect(deckControllerMocks.deleteDeck).toHaveBeenCalledWith(pharmacology.id),
    );
    await waitFor(() => expect(refreshDecks).toHaveBeenCalled());
  });

  it("returns nested deck work through the explicit library-root callback", async () => {
    const onReturnToLibrary = vi.fn();
    renderLibrary({
      openDeckRequest: { requestId: 7, deckId: pharmacology.id },
      onReturnToLibrary,
    });

    fireEvent.click(screen.getByRole("button", { name: "Return to deck library" }));
    expect(onReturnToLibrary).toHaveBeenCalledOnce();
  });

  it("brings the subject Home opened into view once, without animating", async () => {
    const scrolled: Element[] = [];
    const scrollIntoView = vi.fn(function (this: Element) {
      scrolled.push(this);
    });
    Element.prototype.scrollIntoView = scrollIntoView;
    const view = renderLibrary({ subjectFocus: { requestId: 3, subjectId: folder.id } });

    await waitFor(() => expect(scrollIntoView).toHaveBeenCalledOnce());
    expect(scrollIntoView).toHaveBeenCalledWith({ block: "start" });
    expect(scrolled[0]?.getAttribute("data-folder-id")).toBe(folder.id);

    view.rerender(
      <LayoutModeProvider>
        <OverlayProvider>
          <DeckLibraryWorkspace
            decks={[pharmacology, anatomy, deck("d", "Histology", 3, 1)]}
            settings={settings}
            refreshDecks={vi.fn(async () => [pharmacology, anatomy])}
            onPracticeDeck={vi.fn()}
            onPracticeSelected={vi.fn()}
            openDeckRequest={null}
            onOpenDeck={vi.fn()}
            onReturnToLibrary={vi.fn()}
            onCardDirtyChange={vi.fn()}
            showToast={vi.fn()}
            subjectFocus={{ requestId: 3, subjectId: folder.id }}
          />
        </OverlayProvider>
      </LayoutModeProvider>,
    );
    await screen.findByText("Histology");
    expect(scrollIntoView).toHaveBeenCalledOnce();
  });

  it("brings No subject's unsorted decks into view", async () => {
    const scrolled: Element[] = [];
    Element.prototype.scrollIntoView = vi.fn(function (this: Element) {
      scrolled.push(this);
    });
    renderLibrary({ subjectFocus: { requestId: 4, subjectId: null } });

    await waitFor(() => expect(scrolled).toHaveLength(1));
    expect(scrolled[0]?.getAttribute("data-geometry-key")).toBe("destination:unsorted");
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
