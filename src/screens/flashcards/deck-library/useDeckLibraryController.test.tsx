import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import type { DeckLibraryOrganization } from "../../../ipc/deckFolders";
import * as folderIpc from "../../../ipc/deckFolders";
import { useDeckLibraryController } from "./useDeckLibraryController";

const createDeck = vi.fn();
const deleteDeck = vi.fn();
const undo = vi.fn();
const invalidate = vi.fn();

vi.mock("../../../controllers/decksController", () => ({
  DecksController: class {
    create = createDeck;
    deleteDeck = deleteDeck;
    undo = undo;
    invalidate = invalidate;
  },
}));

vi.mock("../../../ipc/deckFolders", async (importOriginal) => {
  const original = await importOriginal<typeof import("../../../ipc/deckFolders")>();
  return {
    ...original,
    getDeckLibraryOrganization: vi.fn(),
    createDeckFolder: vi.fn(),
    updateDeckFolder: vi.fn(),
    deleteDeckFolder: vi.fn(),
    moveDeckFolder: vi.fn(),
    moveDeckInLibrary: vi.fn(),
  };
});

const deckA = deck("a", "Alpha");
const deckB = deck("b", "Beta");
const folder = {
  id: "f".repeat(32),
  name: "Nursing",
  sortOrder: 1_000,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

function organization(revision = 1): DeckLibraryOrganization {
  return {
    revision,
    folders: [folder],
    placements: [
      { deckId: deckA.id, folderId: folder.id, sortOrder: 1_000 },
      { deckId: deckB.id, folderId: folder.id, sortOrder: 2_000 },
    ],
  };
}

describe("useDeckLibraryController", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(folderIpc.getDeckLibraryOrganization).mockResolvedValue(organization());
  });

  it("loads organization state independently from the presentation tree", async () => {
    const refreshDecks = vi.fn().mockResolvedValue([deckA, deckB]);
    const { result, unmount } = renderHook(() =>
      useDeckLibraryController({
        decks: [deckA, deckB],
        refreshDecks,
        onOpenDeck: vi.fn(),
        showToast: vi.fn(),
      }),
    );

    await waitFor(() => expect(result.current.isLoading).toBe(false));
    expect(result.current.organization).toEqual(organization());
    expect(result.current.activeDecks).toEqual([deckA, deckB]);
    unmount();
    expect(invalidate).toHaveBeenCalledTimes(1);
  });

  it("applies a move optimistically and replaces it with the authoritative response", async () => {
    let resolveMove: ((value: DeckLibraryOrganization) => void) | undefined;
    vi.mocked(folderIpc.moveDeckInLibrary).mockReturnValue(
      new Promise((resolve) => {
        resolveMove = resolve;
      }),
    );
    const { result } = renderHook(() =>
      useDeckLibraryController({
        decks: [deckA, deckB],
        refreshDecks: vi.fn().mockResolvedValue([deckA, deckB]),
        onOpenDeck: vi.fn(),
        showToast: vi.fn(),
      }),
    );
    await waitFor(() => expect(result.current.organization?.revision).toBe(1));

    let pending: Promise<void> | undefined;
    act(() => {
      pending = result.current.move({
        kind: "deck",
        deckId: deckA.id,
        targetFolderId: folder.id,
        targetIndex: 1,
        source: "keyboard",
      });
    });
    expect(result.current.organization?.revision).toBe(2);
    expect(result.current.organization?.placements[0]?.sortOrder).toBe(2_000);

    const authoritative = organization(2);
    authoritative.placements = [
      { deckId: deckB.id, folderId: folder.id, sortOrder: 1_000 },
      { deckId: deckA.id, folderId: folder.id, sortOrder: 2_000 },
    ];
    resolveMove?.(authoritative);
    await act(async () => pending);

    expect(folderIpc.moveDeckInLibrary).toHaveBeenCalledWith({
      deckId: deckA.id,
      targetFolderId: folder.id,
      targetIndex: 1,
      expectedOrganizationRevision: 1,
    });
    expect(result.current.organization).toEqual(authoritative);
    expect(result.current.announcement).toContain("Moved Alpha");
  });

  it("reloads on a revision conflict and reports recovery instead of leaving optimistic state", async () => {
    const latest = organization(4);
    vi.mocked(folderIpc.moveDeckFolder).mockRejectedValue({
      code: "library_organization_changed",
      message: "stale revision",
    });
    vi.mocked(folderIpc.getDeckLibraryOrganization)
      .mockResolvedValueOnce(organization())
      .mockResolvedValueOnce(latest);
    const showToast = vi.fn();
    const { result } = renderHook(() =>
      useDeckLibraryController({
        decks: [deckA, deckB],
        refreshDecks: vi.fn().mockResolvedValue([deckA, deckB]),
        onOpenDeck: vi.fn(),
        showToast,
      }),
    );
    await waitFor(() => expect(result.current.organization?.revision).toBe(1));

    await act(async () => {
      await result.current.move({
        kind: "folder",
        folderId: folder.id,
        targetIndex: 0,
        source: "fallback",
      });
    });

    expect(result.current.organization).toEqual(organization());
    expect(folderIpc.moveDeckFolder).not.toHaveBeenCalled();

    const secondFolder = { ...folder, id: "g".repeat(32), name: "Second", sortOrder: 2_000 };
    const twoFolders = { ...organization(), folders: [folder, secondFolder] };
    vi.mocked(folderIpc.getDeckLibraryOrganization).mockResolvedValueOnce(twoFolders);
    // lifecycle changes exercise the same reload lane; the conflict path is covered with a non-noop below.
    const { result: secondResult } = renderHook(() =>
      useDeckLibraryController({
        decks: [deckA, deckB],
        refreshDecks: vi.fn().mockResolvedValue([deckA, deckB]),
        onOpenDeck: vi.fn(),
        showToast,
      }),
    );
    await waitFor(() => expect(secondResult.current.organization?.folders).toHaveLength(2));
    vi.mocked(folderIpc.getDeckLibraryOrganization).mockResolvedValueOnce({
      ...twoFolders,
      revision: 4,
    });
    await act(async () => {
      await secondResult.current.move({
        kind: "folder",
        folderId: folder.id,
        targetIndex: 1,
        source: "keyboard",
      });
    });
    expect(secondResult.current.organization?.revision).toBe(4);
    expect(showToast).toHaveBeenCalledWith(
      "info",
      expect.stringContaining("latest organization is loaded"),
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
