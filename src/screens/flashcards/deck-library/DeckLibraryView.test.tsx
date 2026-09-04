import { useRef, useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import type { DeckFolder } from "../../../ipc/deckFolders";
import { LayoutModeProvider } from "../../../ui/layout/LayoutModeProvider";
import { createDeckLibrarySnapshot } from "./organizationModel";
import type { DeckLibraryController } from "./useDeckLibraryController";
import type { DeckLibraryReorderAdapter } from "./useDeckLibraryReorder";
import type { PointerDragFeedbackState } from "./useDeckLibraryPointerSession";

const rowRender = vi.hoisted(() => vi.fn());
const originalMatchMedia = window.matchMedia;

vi.mock("./DeckItem", () => ({
  DeckItem: ({ deck }: { deck: DeckSummary }) => {
    rowRender(deck.id);
    return <article data-testid="large-tree-deck">{deck.name}</article>;
  },
}));
vi.mock("./ArchivedDecksSection", () => ({ ArchivedDecksSection: () => null }));

import { DeckLibraryView } from "./DeckLibraryView";
import { LibraryDragFeedback } from "./LibraryDragFeedback";

const decks = Array.from({ length: 240 }, (_, index) => deck(index));
const folder: DeckFolder = {
  id: "f".repeat(32),
  name: "Nursing",
  sortOrder: 1_000,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};
const snapshot = createDeckLibrarySnapshot(decks, {
  revision: 11,
  folders: [],
  placements: decks.map((item, index) => ({
    deckId: item.id,
    folderId: null,
    sortOrder: (index + 1) * 1_000,
  })),
});
const reorder: DeckLibraryReorderAdapter = {
  moveDeckWithKeyboard: vi.fn(),
  moveFolderWithKeyboard: vi.fn(),
  moveDeckWithFallback: vi.fn(),
  beginPointerDrag: vi.fn(),
};
const controller = {
  snapshot,
  selectedIds: new Set<string>(),
  isMutating: false,
  showArchived: false,
  collapsedFolderIds: new Set<string>(),
  allFoldersCollapsed: false,
  currentScopeDeckCount: decks.length,
  openDeckCreator: vi.fn(),
  openFolderCreator: vi.fn(),
  collapseAllFolders: vi.fn(),
  expandFolder: vi.fn(),
  clearSelection: vi.fn(),
  selectCurrentScope: vi.fn(),
  toggleFolder: vi.fn(),
  openDeck: vi.fn(),
  removeDeck: vi.fn(),
  removeFolder: vi.fn(),
  openFolderRenamer: vi.fn(),
  toggleDeckSelected: vi.fn(),
  toggleArchived: vi.fn(),
} as unknown as DeckLibraryController;
const onPracticeDeck = vi.fn();
const onExportDeck = vi.fn();

describe("DeckLibraryView", () => {
  beforeEach(() => {
    rowRender.mockClear();
    vi.mocked(reorder.beginPointerDrag).mockClear();
    onPracticeDeck.mockClear();
    onExportDeck.mockClear();
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
  });

  it("does not recommit a deterministic large deck tree when semantic drag feedback changes", () => {
    render(<Harness />);
    expect(screen.getAllByTestId("large-tree-deck")).toHaveLength(240);
    expect(rowRender).toHaveBeenCalledTimes(240);

    fireEvent.click(screen.getByRole("button", { name: "Change drag target" }));

    expect(rowRender).toHaveBeenCalledTimes(240);
    expect(screen.getByText("Clinical, position 1.")).toBeTruthy();
  });

  it("renders one actionable empty state without empty folder, unsorted, or selection chrome", () => {
    const openDeckCreator = vi.fn();
    const emptyController = {
      ...controller,
      snapshot: createDeckLibrarySnapshot([], {
        revision: 1,
        folders: [],
        placements: [],
      }),
      currentScopeDeckCount: 0,
      openDeckCreator,
    } as DeckLibraryController;

    renderView(emptyController);

    expect(screen.getByRole("heading", { name: "No decks have been created yet." })).toBeTruthy();
    expect(screen.getByText(/Import deck from file above/u)).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Folders" })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Unsorted decks" })).toBeNull();
    expect(screen.queryByRole("button", { name: /Collapse all|Expand all/u })).toBeNull();
    expect(screen.queryByRole("button", { name: /Select all/u })).toBeNull();
    expect(screen.queryByText("No folders yet.")).toBeNull();
    expect(screen.queryByText("All active decks are in folders.")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Create a new deck" }));
    expect(openDeckCreator).toHaveBeenCalledOnce();
  });

  it("places New folder, Collapse all, and Select all together only when relevant", () => {
    const activeDeck = deck(0);
    const folderSnapshot = createDeckLibrarySnapshot([activeDeck], {
      revision: 12,
      folders: [folder],
      placements: [{ deckId: activeDeck.id, folderId: folder.id, sortOrder: 1_000 }],
    });
    const openFolderCreator = vi.fn();
    const collapseAllFolders = vi.fn();
    const selectCurrentScope = vi.fn();
    const folderController = {
      ...controller,
      snapshot: folderSnapshot,
      currentScopeDeckCount: 1,
      openFolderCreator,
      collapseAllFolders,
      selectCurrentScope,
    } as DeckLibraryController;

    renderView(folderController);

    expect(screen.getByRole("heading", { name: "Folders" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Unsorted decks" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Help with folders" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Create a folder" }));
    fireEvent.click(screen.getByRole("button", { name: "Collapse all" }));
    fireEvent.click(screen.getByRole("button", { name: "Select all (1)" }));
    expect(openFolderCreator).toHaveBeenCalledOnce();
    expect(collapseAllFolders).toHaveBeenCalledOnce();
    expect(selectCurrentScope).toHaveBeenCalledOnce();
  });

  it("turns Collapse all into Expand all after every folder is collapsed", () => {
    const activeDeck = deck(0);
    const folderSnapshot = createDeckLibrarySnapshot([activeDeck], {
      revision: 13,
      folders: [folder],
      placements: [{ deckId: activeDeck.id, folderId: folder.id, sortOrder: 1_000 }],
    });
    const expandFolder = vi.fn();
    const folderController = {
      ...controller,
      snapshot: folderSnapshot,
      currentScopeDeckCount: 1,
      allFoldersCollapsed: true,
      collapsedFolderIds: new Set([folder.id]),
      expandFolder,
    } as DeckLibraryController;

    renderView(folderController);
    fireEvent.click(screen.getByRole("button", { name: "Expand all" }));
    expect(expandFolder).toHaveBeenCalledWith(folder.id);
  });

  it.each([
    { collapsed: false, layoutMode: "wide", toggleName: "Collapse Nursing folder" },
    { collapsed: true, layoutMode: "wide", toggleName: "Expand Nursing folder" },
    { collapsed: false, layoutMode: "medium", toggleName: "Collapse Nursing folder" },
    { collapsed: true, layoutMode: "medium", toggleName: "Expand Nursing folder" },
    { collapsed: false, layoutMode: "compact", toggleName: "Collapse Nursing folder" },
    { collapsed: true, layoutMode: "compact", toggleName: "Expand Nursing folder" },
  ] as const)(
    "starts folder drag from the summary but not explicit controls when collapsed is $collapsed in $layoutMode layout",
    ({ collapsed, layoutMode, toggleName }) => {
      setLayoutMode(layoutMode);
      const folderSnapshot = createDeckLibrarySnapshot([deck(0)], {
        revision: 12,
        folders: [folder],
        placements: [{ deckId: deck(0).id, folderId: folder.id, sortOrder: 1_000 }],
      });
      const folderController = {
        ...controller,
        snapshot: folderSnapshot,
        currentScopeDeckCount: 1,
        collapsedFolderIds: collapsed ? new Set([folder.id]) : new Set<string>(),
      } as DeckLibraryController;

      renderView(folderController);

      fireEvent.pointerDown(screen.getByRole("heading", { name: folder.name }));
      fireEvent.pointerDown(screen.getByText("1 deck · 1 ready"));

      expect(reorder.beginPointerDrag).toHaveBeenCalledTimes(2);
      expect(reorder.beginPointerDrag).toHaveBeenLastCalledWith(
        expect.anything(),
        { kind: "folder", id: folder.id },
        `${folder.name} folder`,
      );

      fireEvent.pointerDown(screen.getByRole("button", { name: toggleName }));
      fireEvent.pointerDown(
        screen.getByRole("button", { name: /Reorder Nursing folder\./u }),
      );
      if (layoutMode === "compact") {
        fireEvent.pointerDown(screen.getByRole("button", { name: "Nursing folder actions" }));
      } else {
        fireEvent.pointerDown(screen.getByRole("button", { name: "Rename" }));
        fireEvent.pointerDown(screen.getByRole("button", { name: "Delete" }));
      }

      expect(reorder.beginPointerDrag).toHaveBeenCalledTimes(2);
    },
  );

  it.each([
    { collapsed: false, layoutMode: "wide", toggleName: "Collapse Nursing folder" },
    { collapsed: true, layoutMode: "wide", toggleName: "Expand Nursing folder" },
    { collapsed: false, layoutMode: "medium", toggleName: "Collapse Nursing folder" },
    { collapsed: true, layoutMode: "medium", toggleName: "Expand Nursing folder" },
    { collapsed: false, layoutMode: "compact", toggleName: "Collapse Nursing folder" },
    { collapsed: true, layoutMode: "compact", toggleName: "Expand Nursing folder" },
  ] as const)(
    "uses only the explicit caret to toggle when collapsed is $collapsed in $layoutMode layout",
    ({ collapsed, layoutMode, toggleName }) => {
      setLayoutMode(layoutMode);
      const toggleFolder = vi.fn();
      const folderSnapshot = createDeckLibrarySnapshot([deck(0)], {
        revision: 12,
        folders: [folder],
        placements: [{ deckId: deck(0).id, folderId: folder.id, sortOrder: 1_000 }],
      });
      const folderController = {
        ...controller,
        snapshot: folderSnapshot,
        currentScopeDeckCount: 1,
        collapsedFolderIds: collapsed ? new Set([folder.id]) : new Set<string>(),
        toggleFolder,
      } as DeckLibraryController;

      renderView(folderController);

      fireEvent.click(screen.getByRole("button", { name: toggleName }));
      expect(toggleFolder).toHaveBeenCalledTimes(1);
      expect(toggleFolder).toHaveBeenCalledWith(folder.id);
    },
  );
});

function renderView(selectedController: DeckLibraryController) {
  return render(
    <LayoutModeProvider>
      <DeckLibraryView
        controller={selectedController}
        reorder={reorder}
        libraryRef={{ current: null }}
        onPracticeDeck={onPracticeDeck}
        onExportDeck={onExportDeck}
      />
    </LayoutModeProvider>,
  );
}

function Harness() {
  const libraryRef = useRef<HTMLDivElement | null>(null);
  const [feedback, setFeedback] = useState<PointerDragFeedbackState>(() =>
    dragFeedback("Nursing", "valid"),
  );
  return (
    <>
      <button type="button" onClick={() => setFeedback(dragFeedback("Clinical", "invalid"))}>
        Change drag target
      </button>
      <DeckLibraryView
        controller={controller}
        reorder={reorder}
        libraryRef={libraryRef}
        onPracticeDeck={onPracticeDeck}
        onExportDeck={onExportDeck}
      />
      <LibraryDragFeedback feedback={feedback} />
    </>
  );
}

function dragFeedback(
  destinationName: string,
  status: "valid" | "invalid",
): PointerDragFeedbackState {
  return {
    active: true,
    sourceName: "Large library drag source",
    candidate: {
      key: `${status}:${destinationName}`,
      status,
      intent: null,
      targetKey: null,
      destinationFolderId: null,
      destinationName,
      insertionLabel: "position 1",
      insertionTargetKey: null,
      insertionEdge: null,
      announcement: `${destinationName}, position 1.`,
    },
  };
}

function setLayoutMode(mode: "wide" | "medium" | "compact"): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches:
      (query === "(max-width: 760px)" && mode === "compact") ||
      (query === "(max-width: 1180px)" && mode !== "wide"),
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

function deck(index: number): DeckSummary {
  return {
    id: index.toString(16).padStart(32, "0"),
    name: `Deck ${index.toString().padStart(3, "0")}`,
    description: "",
    sourceNoteCount: 1,
    generatedCardCount: 1,
    studyableDueCount: 1,
    archivedAt: null,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}
