import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ShowToast } from "../../../components/Toast";
import type { DeckSummary } from "../../../ipc/decks";
import type { DeckLibraryOrganization } from "../../../ipc/deckFolders";
import {
  useDeckLibraryController,
  type DeckLibraryControllerDependencies,
  type UseDeckLibraryControllerOptions,
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
  setArchived: ReturnType<typeof vi.fn>;
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
  const setArchived = vi.fn(async ({ deckId, archived }: { deckId: string; archived: boolean }) => ({
    ...(deckId === deckA.id ? deckA : deckB),
    archivedAt: archived ? 50 : null,
    revision: 2,
  }));
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
    createDeckController: () => ({ create: createDeck, setArchived, deleteDeck, undo, invalidate }),
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
    setArchived,
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
        refreshDecks:
          harness.refreshDecks as UseDeckLibraryControllerOptions["refreshDecks"],
        onOpenDeck: harness.onOpenDeck as UseDeckLibraryControllerOptions["onOpenDeck"],
        showToast: harness.showToast,
        confirm: harness.confirm as UseDeckLibraryControllerOptions["confirm"],
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

  it("keeps selection across organization and collapse changes but reconciles scope and lifecycle", async () => {
    const archived = { ...deck("z", "Archived"), archivedAt: 5 };
    const { result } = renderController(harness, [deckA, deckB, archived]);
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());

    act(() => result.current.toggleFolder(folder.id));
    act(() => result.current.selectVisible());
    expect([...result.current.selectedIds]).toEqual([deckB.id]);

    act(() => result.current.selectCurrentScope());
    expect(result.current.selectedIds).toEqual(new Set([deckB.id, deckA.id]));
    act(() => result.current.toggleFolder(folder.id));
    expect(result.current.selectedIds).toEqual(new Set([deckB.id, deckA.id]));

    act(() => result.current.toggleArchived());
    act(() => result.current.selectCurrentScope());
    expect(result.current.selectedIds.has(archived.id)).toBe(true);
    act(() => result.current.toggleArchived());
    await waitFor(() => expect(result.current.selectedIds.has(archived.id)).toBe(false));
  });

  it("blocks stale selected revisions until the learner reselects", async () => {
    const { result, rerender } = renderController(harness);
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());
    act(() => result.current.toggleDeckSelected(deckA));

    rerender({ currentDecks: [{ ...deckA, revision: 2 }, deckB] });
    expect(result.current.selectionAnalysis.staleIds).toEqual([deckA.id]);
    act(() => result.current.openSelectionMover());

    expect(result.current.sheet).toBeNull();
    expect(harness.showToast).toHaveBeenCalledWith(
      "info",
      expect.stringContaining("changed since selection"),
    );
  });

  it("moves selected decks in display order and advances the shared organization revision", async () => {
    const destination = { ...folder, id: "g".repeat(32), name: "Clinical", sortOrder: 2_000 };
    const initial = { ...organization, folders: [folder, destination] };
    const afterFirst: DeckLibraryOrganization = {
      ...initial,
      revision: 5,
      placements: [
        { deckId: deckA.id, folderId: destination.id, sortOrder: 1_000 },
        { deckId: deckB.id, folderId: null, sortOrder: 1_000 },
      ],
    };
    const afterSecond: DeckLibraryOrganization = {
      ...afterFirst,
      revision: 6,
      placements: [
        { deckId: deckA.id, folderId: destination.id, sortOrder: 1_000 },
        { deckId: deckB.id, folderId: destination.id, sortOrder: 2_000 },
      ],
    };
    harness.getOrganization.mockResolvedValue(initial);
    harness.moveDeck
      .mockResolvedValueOnce(afterFirst)
      .mockResolvedValueOnce(afterSecond);
    const { result } = renderController(harness);
    await waitFor(() => expect(result.current.organization).toEqual(initial));
    act(() => result.current.selectCurrentScope());

    await act(async () => result.current.moveSelection(destination.id));

    expect(harness.moveDeck).toHaveBeenNthCalledWith(1, {
      deckId: deckA.id,
      targetFolderId: destination.id,
      targetIndex: 0,
      expectedOrganizationRevision: 4,
    });
    expect(harness.moveDeck).toHaveBeenNthCalledWith(2, {
      deckId: deckB.id,
      targetFolderId: destination.id,
      targetIndex: 1,
      expectedOrganizationRevision: 5,
    });
    expect(result.current.organization).toEqual(afterSecond);
    expect(result.current.selectedIds).toEqual(new Set([deckA.id, deckB.id]));
  });

  it("stops bulk move after a shared-revision failure, reloads, and reports unattempted decks", async () => {
    const deckC = deck("c", "Cardiology");
    const destination = { ...folder, id: "g".repeat(32), name: "Clinical", sortOrder: 2_000 };
    const initial: DeckLibraryOrganization = {
      ...organization,
      folders: [folder, destination],
      placements: [
        ...organization.placements,
        { deckId: deckC.id, folderId: null, sortOrder: 2_000 },
      ],
    };
    const afterFirst: DeckLibraryOrganization = {
      ...initial,
      revision: 5,
      placements: [
        { deckId: deckA.id, folderId: destination.id, sortOrder: 1_000 },
        { deckId: deckB.id, folderId: null, sortOrder: 1_000 },
        { deckId: deckC.id, folderId: null, sortOrder: 2_000 },
      ],
    };
    const authoritative = { ...afterFirst, revision: 6 };
    harness.getOrganization
      .mockResolvedValueOnce(initial)
      .mockResolvedValueOnce(authoritative);
    harness.moveDeck
      .mockResolvedValueOnce(afterFirst)
      .mockRejectedValueOnce({
        code: "library_organization_changed",
        message: "organization changed",
      });
    const { result } = renderController(harness, [deckA, deckB, deckC]);
    await waitFor(() => expect(result.current.organization).toEqual(initial));
    act(() => result.current.selectCurrentScope());

    await act(async () => result.current.moveSelection(destination.id));

    expect(harness.moveDeck).toHaveBeenCalledTimes(2);
    expect(harness.moveDeck).toHaveBeenNthCalledWith(2, {
      deckId: deckB.id,
      targetFolderId: destination.id,
      targetIndex: 1,
      expectedOrganizationRevision: 5,
    });
    expect(harness.getOrganization).toHaveBeenCalledTimes(2);
    expect(result.current.organization).toEqual(authoritative);
    expect(result.current.selectedIds).toEqual(new Set([deckA.id, deckB.id, deckC.id]));
    expect(harness.showToast).toHaveBeenCalledWith(
      "info",
      expect.stringMatching(
        new RegExp(`Failed: Anatomy \\(${deckB.id}\\).*Not attempted: Cardiology \\(${deckC.id}\\)`),
      ),
    );
  });

  it("continues independent archive mutations, preserves revisions, and retains failed IDs", async () => {
    harness.setArchived.mockImplementation(
      async ({ deckId, revision, archived }: { deckId: string; revision: number; archived: boolean }) => {
        if (deckId === deckB.id) throw { code: "deck_changed", message: "stale" };
        return { ...deckA, archivedAt: archived ? 9 : null, revision: revision + 1 };
      },
    );
    const { result } = renderController(harness);
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());
    act(() => result.current.selectCurrentScope());

    await act(async () => result.current.setSelectionArchived(true));

    expect(harness.confirm).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Archive 2 selected decks?",
        description: expect.stringContaining("not atomic"),
      }),
    );
    expect(harness.setArchived).toHaveBeenNthCalledWith(1, {
      deckId: deckA.id,
      revision: deckA.revision,
      archived: true,
    });
    expect(harness.setArchived).toHaveBeenNthCalledWith(2, {
      deckId: deckB.id,
      revision: deckB.revision,
      archived: true,
    });
    expect([...result.current.selectedIds]).toEqual([deckB.id]);
    expect(harness.showToast).toHaveBeenCalledWith(
      "info",
      expect.stringContaining(`Failed: Anatomy (${deckB.id})`),
    );
  });

  it("does not report an empty archive subset when another mutation is already running", async () => {
    let finishArchive: (() => void) | null = null;
    harness.setArchived.mockImplementation(
      () => new Promise<DeckSummary>((resolve) => {
        finishArchive = () => resolve({ ...deckA, archivedAt: 9, revision: 2 });
      }),
    );
    const { result } = renderController(harness);
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());
    act(() => result.current.toggleDeckSelected(deckA));

    let firstArchive: Promise<void> | undefined;
    act(() => {
      firstArchive = result.current.setSelectionArchived(true);
    });
    await waitFor(() => expect(harness.setArchived).toHaveBeenCalledTimes(1));
    await act(async () => result.current.setSelectionArchived(true));

    expect(harness.showToast).not.toHaveBeenCalledWith(
      "info",
      "No active selected decks can be archived.",
    );
    await act(async () => {
      finishArchive?.();
      await firstArchive;
    });
  });

  it("retries only unresolved bulk restores using their remaining undo expiry", async () => {
    const archived = { ...deck("c", "Archived"), archivedAt: 5 };
    harness.deleteDeck.mockImplementation(async (deckId: string) => {
      if (deckId === deckB.id) throw new Error("delete failed");
      return {
        undoToken: `undo-${deckId}`,
        undoExpiresAt: deckId === deckA.id ? 900 : 1_000,
      };
    });
    let archivedUndoAttempts = 0;
    harness.undo.mockImplementation(async (undoToken: string) => {
      if (undoToken === `undo-${archived.id}` && archivedUndoAttempts++ === 0) {
        throw new Error("restore failed");
      }
      return { restoredCount: 1 };
    });
    const showToast = vi.mocked(harness.showToast);
    const { result } = renderController(harness, [deckA, deckB, archived]);
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());
    act(() => result.current.toggleArchived());
    act(() => result.current.selectCurrentScope());

    await act(async () => result.current.deleteSelection());

    expect(harness.confirm).toHaveBeenCalledWith(
      expect.objectContaining({
        title: "Delete 3 selected decks?",
        destructive: true,
        description: expect.stringContaining("Selection: 2 active, 1 archived"),
      }),
    );
    expect([...result.current.selectedIds]).toEqual([deckB.id]);
    const action = showToast.mock.calls.find((call) => call[2]?.label === "Undo all")?.[2];
    expect(action?.expiresAt).toBe(900);
    expect(showToast).toHaveBeenCalledWith(
      "info",
      expect.stringContaining(`Failed: Anatomy (${deckB.id})`),
      expect.objectContaining({ label: "Undo all" }),
    );

    await act(async () => {
      try {
        await action?.run();
      } catch {
        // The toast action rejects so its host keeps the failed action visible.
      }
    });
    expect(harness.undo.mock.calls.map(([token]) => token)).toEqual([
      `undo-${deckA.id}`,
      `undo-${archived.id}`,
    ]);
    const retry = [...showToast.mock.calls]
      .reverse()
      .find((call) => call[2]?.label === "Retry remaining undo")?.[2];
    expect(retry?.expiresAt).toBe(1_000);

    harness.undo.mockClear();
    await act(async () => retry?.run());
    expect(harness.undo).toHaveBeenCalledTimes(1);
    expect(harness.undo).toHaveBeenCalledWith(`undo-${archived.id}`);
  });

  it("reports a committed bulk mutation as successful when reconciliation fails", async () => {
    harness.refreshDecks.mockRejectedValue(new Error("refresh unavailable"));
    const { result } = renderController(harness);
    await waitFor(() => expect(result.current.snapshot).not.toBeNull());
    act(() => result.current.toggleDeckSelected(deckA));

    await act(async () => result.current.setSelectionArchived(true));

    expect(harness.setArchived).toHaveBeenCalledWith({
      deckId: deckA.id,
      revision: deckA.revision,
      archived: true,
    });
    expect(harness.showToast).toHaveBeenCalledWith(
      "info",
      expect.stringMatching(/Archived 1 of 1.*committed changes remain saved.*refresh unavailable/u),
    );
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
