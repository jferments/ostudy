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
vi.mock("../../../ui/help/HelpProvider", () => ({ HelpTrigger: () => null }));

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
  openFolderCreator: vi.fn(),
  collapseAllFolders: vi.fn(),
  toggleFolder: vi.fn(),
  openDeck: vi.fn(),
  removeDeck: vi.fn(),
  removeFolder: vi.fn(),
  openFolderRenamer: vi.fn(),
  toggleDeckSelected: vi.fn(),
  toggleArchived: vi.fn(),
} as unknown as DeckLibraryController;
const onPracticeDeck = vi.fn();

describe("DeckLibraryView render isolation", () => {
  beforeEach(() => {
    rowRender.mockClear();
    vi.mocked(reorder.beginPointerDrag).mockClear();
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
  });

  it("does not recommit a deterministic large deck tree when drag target feedback changes", () => {
    render(<Harness />);
    expect(screen.getAllByTestId("large-tree-deck")).toHaveLength(240);
    expect(rowRender).toHaveBeenCalledTimes(240);

    fireEvent.click(screen.getByRole("button", { name: "Change drag target" }));

    expect(rowRender).toHaveBeenCalledTimes(240);
    expect(screen.getByText("Clinical · position 1")).not.toBeNull();
  });

  it.each([
    { collapsed: false, layoutMode: "wide", toggleName: "Collapse Nursing folder" },
    { collapsed: true, layoutMode: "wide", toggleName: "Expand Nursing folder" },
    { collapsed: false, layoutMode: "medium", toggleName: "Collapse Nursing folder" },
    { collapsed: true, layoutMode: "medium", toggleName: "Expand Nursing folder" },
    { collapsed: false, layoutMode: "compact", toggleName: "Collapse Nursing folder" },
    { collapsed: true, layoutMode: "compact", toggleName: "Expand Nursing folder" },
  ])(
    "starts folder drag from title and copy, not explicit controls, when collapsed is $collapsed in $layoutMode layout",
    ({ collapsed, layoutMode, toggleName }) => {
      window.matchMedia = vi.fn().mockImplementation((query: string) => ({
        matches:
          (query === "(max-width: 760px)" && layoutMode === "compact") ||
          (query === "(max-width: 1180px)" && layoutMode !== "wide"),
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        addListener: vi.fn(),
        removeListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }));
      const folderSnapshot = createDeckLibrarySnapshot([deck(0)], {
        revision: 12,
        folders: [folder],
        placements: [{ deckId: deck(0).id, folderId: folder.id, sortOrder: 1_000 }],
      });
      const folderController = {
        ...controller,
        snapshot: folderSnapshot,
        collapsedFolderIds: collapsed ? new Set([folder.id]) : new Set<string>(),
      } as DeckLibraryController;

      render(
        <LayoutModeProvider>
          <DeckLibraryView
            controller={folderController}
            reorder={reorder}
            libraryRef={{ current: null }}
            onPracticeDeck={onPracticeDeck}
          />
        </LayoutModeProvider>,
      );

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
        fireEvent.pointerDown(
          screen.getByRole("button", { name: "Nursing folder actions" }),
        );
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
  ])(
    "toggles a folder from short summary clicks and its explicit control when collapsed is $collapsed in $layoutMode layout",
    ({ collapsed, layoutMode, toggleName }) => {
      window.matchMedia = vi.fn().mockImplementation((query: string) => ({
        matches:
          (query === "(max-width: 760px)" && layoutMode === "compact") ||
          (query === "(max-width: 1180px)" && layoutMode !== "wide"),
        media: query,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
        addListener: vi.fn(),
        removeListener: vi.fn(),
        dispatchEvent: vi.fn(),
      }));
      const toggleFolder = vi.fn();
      const folderSnapshot = createDeckLibrarySnapshot([deck(0)], {
        revision: 12,
        folders: [folder],
        placements: [{ deckId: deck(0).id, folderId: folder.id, sortOrder: 1_000 }],
      });
      const folderController = {
        ...controller,
        snapshot: folderSnapshot,
        collapsedFolderIds: collapsed ? new Set([folder.id]) : new Set<string>(),
        toggleFolder,
      } as DeckLibraryController;

      render(
        <LayoutModeProvider>
          <DeckLibraryView
            controller={folderController}
            reorder={reorder}
            libraryRef={{ current: null }}
            onPracticeDeck={onPracticeDeck}
          />
        </LayoutModeProvider>,
      );

      const title = screen.getByRole("heading", { name: folder.name });
      const copy = screen.getByText("1 deck · 1 ready");
      const dragSurface = title.closest("[data-library-pointer-drag-source='true']");

      expect(dragSurface?.tagName).toBe("DIV");
      expect(dragSurface?.hasAttribute("role")).toBe(false);
      expect(dragSurface?.hasAttribute("tabindex")).toBe(false);

      fireEvent.click(title);
      fireEvent.click(copy);
      fireEvent.click(screen.getByRole("button", { name: toggleName }));

      expect(toggleFolder).toHaveBeenCalledTimes(3);
      expect(toggleFolder).toHaveBeenNthCalledWith(1, folder.id);
      expect(toggleFolder).toHaveBeenNthCalledWith(2, folder.id);
      expect(toggleFolder).toHaveBeenNthCalledWith(3, folder.id);
    },
  );
});

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
      />
      <LibraryDragFeedback feedback={feedback} bindPreview={() => undefined} />
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
