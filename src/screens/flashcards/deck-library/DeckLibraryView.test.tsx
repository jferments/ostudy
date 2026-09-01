import { useRef, useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import { createDeckLibrarySnapshot } from "./organizationModel";
import type { DeckLibraryController } from "./useDeckLibraryController";
import type { DeckLibraryReorderAdapter } from "./useDeckLibraryReorder";
import type { PointerDragFeedbackState } from "./useDeckLibraryPointerSession";

const rowRender = vi.hoisted(() => vi.fn());

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
const snapshot = createDeckLibrarySnapshot(decks, {
  revision: 11,
  folders: [],
  placements: decks.map((item, index) => ({
    deckId: item.id,
    folderId: null,
    sortOrder: (index + 1) * 1_000,
  })),
});
const reorder = new Proxy(
  {},
  { get: () => vi.fn() },
) as DeckLibraryReorderAdapter;
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
  beforeEach(() => rowRender.mockClear());

  it("does not recommit a deterministic large deck tree when drag target feedback changes", () => {
    render(<Harness />);
    expect(screen.getAllByTestId("large-tree-deck")).toHaveLength(240);
    expect(rowRender).toHaveBeenCalledTimes(240);

    fireEvent.click(screen.getByRole("button", { name: "Change drag target" }));

    expect(rowRender).toHaveBeenCalledTimes(240);
    expect(screen.getByText("Clinical")).not.toBeNull();
  });
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
