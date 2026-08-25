import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ShowToast } from "../../../components/Toast";
import type { DeckSummary } from "../../../ipc/decks";
import type { DeckLibraryOrganization } from "../../../ipc/deckFolders";
import {
  useDeckLibraryController,
  type DeckLibraryControllerDependencies,
} from "./useDeckLibraryController";

const folder = {
  id: "f".repeat(32),
  name: "Nursing",
  sortOrder: 1_000,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};
const deckA = deck("a", "Pharmacology");
const deckB = deck("b", "Anatomy");
const organization: DeckLibraryOrganization = {
  revision: 4,
  folders: [folder],
  placements: [
    { deckId: deckA.id, folderId: folder.id, sortOrder: 1_000 },
    { deckId: deckB.id, folderId: null, sortOrder: 1_000 },
  ],
};

interface Harness {
  dependencies: DeckLibraryControllerDependencies;
  getOrganization: ReturnType<typeof vi.fn>;
  createFolder: ReturnType<typeof vi.fn>;
  updateFolder: ReturnType<typeof vi.fn>;
  deleteFolder: ReturnType<typeof vi.fn>;
  moveDeck: ReturnType<typeof vi.fn>;
  moveFolder: ReturnType<typeof vi.fn>;
  createDeck: ReturnType<typeof vi.fn>;
  deleteDeck: ReturnType<typeof vi.fn>;
  undo: ReturnType<typeof vi.fn>;
  invalidate: ReturnType<typeof vi.fn>;
  refreshDecks: ReturnType<typeof vi.fn>;
  onOpenDeck: ReturnType<typeof vi.fn>;
  showToast: ShowToast;
  confirm: ReturnType<typeof vi.fn>;
}

function createHarness(): Harness {
  const getOrganization = vi.fn().mockResolvedValue(organization);
  const createFolder = vi.fn();
  const updateFolder = vi.fn();
  const deleteFolder = vi.fn();
  const moveDeck = vi.fn();
  const moveFolder = vi.fn();
  const createDeck = vi.fn().mockResolvedValue(deck("c", "Created"));
  const deleteDeck = vi.fn().mockResolvedValue({
    undoToken: "u".repeat(32),
    undoExpiresAt: 999,
  });
  const undo = vi.fn().mockResolvedValue({ restoredCount: 1 });
  const invalidate = vi.fn();
  const dependencies: DeckLibraryControllerDependencies = {
    getOrganization,
    createFolder,
    updateFolder,
    deleteFolder,
    moveDeck,
    moveFolder,
    createDeckController: () => ({ create: createDeck, deleteDeck, undo, invalidate }),
  };
  return {
    dependencies,
    getOrganization,
    createFolder,
    updateFolder,
    deleteFolder,
    moveDeck,
    moveFolder,
    createDeck,
    deleteDeck,
    undo,
    invalidate,
    refreshDecks: vi.fn().mockResolvedValue([deckA, deckB]),
    onOpenDeck: vi.fn(),
    showToast: vi.fn(),
    confirm: vi.fn().mockResolvedValue(true),
  };
}

function renderController(harness: Harness, decks: DeckSummary[] = [deckA, deckB]) {
  return renderHook(
    ({ currentDecks }: { currentDecks: DeckSummary[] }) =>
      useDeckLibraryController({
        decks: currentDecks,
        refreshDecks: harness.refreshDecks,
        onOpenDeck: harness.onOpenDeck,
        showToast: harness.showToast,
        confirm: harness.confirm,
        dependencies: harness.dependencies,
      }),
    { initialProps: { currentDecks: decks } },
  );
}

describe("useDeckLibraryController", () => {
  let harness: Harness;

  beforeEach(() => {
    harness = createHarness();
  });

  it("owns initial organization loading and derives the library snapshot", async () => {
    const { result, unmount } = renderController(harness);

    expect(result.current.isLoading).toBe(true);
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.snapshot?.decksByFolder.get(folder.id)).toEqual([deckA]);
    expect(result.current.snapshot?.unsortedDecks).toEqual([deckB]);

    unmount();
    expect(harness.invalidate).toHaveBeenCalledTimes(1);
  });

  it("applies optimistic movement, commits the authoritative response, and announces it", async () => {
    let resolveMove: ((value: DeckLibraryOrganization) => void) | null = null;
    harness.moveDeck.mockImplementation(
      () =>
        new Promise<DeckLibraryOrganization>((resolve) => {
          resolveMove = resolve;
        }),
    );
    const committed: DeckLibraryOrganization = {
      ...organization,
      revision: 5,
      placements: [
        { deckId: deckA.id, folderId: folder.id, sortOrder: 1_000 },
        { deckId: deckB.id, folderId: folder.id, sortOrder: 2_000 },
      ],
    };
    const { result } = renderController(harness);
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());

    let movement: Promise<void> | undefined;
    act(() => {
      movement = result.current.move({
        kind: "deck",
        deckId: deckB.id,
        targetFolderId: folder.id,
        targetIndex: 1,
        source: "keyboard",
      });
    });
    expect(result.current.isMutating).toBe(true);
    expect(result.current.snapshot?.decksByFolder.get(folder.id)?.map((deck) => deck.id)).toEqual([
      deckA.id,
      deckB.id,
    ]);

    await act(async () => {
      resolveMove?.(committed);
      await movement;
    });
    expect(harness.moveDeck).toHaveBeenCalledWith({
      deckId: deckB.id,
      targetFolderId: folder.id,
      targetIndex: 1,
      expectedOrganizationRevision: 4,
    });
    expect(result.current.organization?.revision).toBe(5);
    expect(result.current.announcement).toContain("Moved Anatomy to Nursing, position 2.");
    expect(result.current.isMutating).toBe(false);
  });

  it("reloads after a stale revision and uses the refreshed revision for the next mutation", async () => {
    const refreshed = { ...organization, revision: 5 };
    harness.getOrganization
      .mockResolvedValueOnce(organization)
      .mockResolvedValueOnce(refreshed);
    harness.moveDeck.mockRejectedValue({
      code: "library_organization_changed",
      message: "stale",
    });
    harness.createFolder.mockResolvedValue({
      ...refreshed,
      revision: 6,
      folders: [folder, { ...folder, id: "g".repeat(32), name: "Second" }],
    });
    const { result } = renderController(harness);
    await waitFor(() => expect(result.current.organization?.revision).toBe(4));

    await act(async () => {
      await result.current.move({
        kind: "deck",
        deckId: deckB.id,
        targetFolderId: folder.id,
        targetIndex: 1,
        source: "pointer",
      });
    });
    expect(result.current.organization?.revision).toBe(5);
    expect(harness.showToast).toHaveBeenCalledWith(
      "info",
      expect.stringContaining("latest organization is loaded"),
    );

    act(() => result.current.openFolderCreator());
    await act(async () => result.current.saveFolder("Second"));
    expect(harness.createFolder).toHaveBeenCalledWith({
      name: "Second",
      expectedOrganizationRevision: 5,
    });
  });

  it("owns folder sheet, collapse, archive, and rename presentation state", async () => {
    const archived = { ...deck("z", "Archived"), archivedAt: 5 };
    const { result } = renderController(harness, [deckA, deckB, archived]);
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());

    act(() => result.current.openFolderCreator());
    expect(result.current.sheet).toEqual({ kind: "create-folder", name: "" });
    act(() => result.current.openFolderRenamer(folder.id));
    expect(result.current.sheet).toMatchObject({ kind: "rename-folder", name: "Nursing" });
    act(() => result.current.toggleFolder(folder.id));
    expect(result.current.collapsedFolderIds.has(folder.id)).toBe(true);
    act(() => result.current.toggleArchived());
    expect(result.current.showArchived).toBe(true);
  });

  it("confirms deck deletion, exposes undo, and refreshes both parent and organization", async () => {
    harness.getOrganization
      .mockResolvedValueOnce(organization)
      .mockResolvedValue({
        revision: 5,
        folders: [folder],
        placements: [{ deckId: deckB.id, folderId: null, sortOrder: 1_000 }],
      });
    const showToast = vi.mocked(harness.showToast);
    const { result } = renderController(harness);
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());

    await act(async () => result.current.removeDeck(deckA.id));
    expect(harness.confirm).toHaveBeenCalledWith(
      expect.objectContaining({ title: "Delete “Pharmacology”?", destructive: true }),
    );
    expect(harness.deleteDeck).toHaveBeenCalledWith(deckA.id);
    const action = showToast.mock.calls.find((call) => call[2]?.label === "Undo")?.[2];
    expect(action).toBeDefined();

    await act(async () => action?.run());
    expect(harness.undo).toHaveBeenCalledWith("u".repeat(32));
    expect(harness.refreshDecks).toHaveBeenCalled();
    expect(harness.getOrganization.mock.calls.length).toBeGreaterThanOrEqual(3);
  });

  it("reloads organization when active/archive lifecycle changes", async () => {
    const { result, rerender } = renderController(harness);
    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(harness.getOrganization).toHaveBeenCalledTimes(1);

    rerender({ currentDecks: [{ ...deckA, archivedAt: 9 }, deckB] });
    await waitFor(() => expect(harness.getOrganization).toHaveBeenCalledTimes(2));
  });
});

function deck(seed: string, name: string): DeckSummary {
  return {
    id: seed.repeat(32),
    name,
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
