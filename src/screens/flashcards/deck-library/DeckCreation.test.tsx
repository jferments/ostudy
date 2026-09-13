import { act, renderHook, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { DeckSummary } from "../../../ipc/decks";
import { useDeckLibraryController, type DeckLibraryControllerDependencies } from "./useDeckLibraryController";

it("creates a named deck and opens its committed identity after refreshing the live library", async () => {
  const created: DeckSummary = {
    id: "c".repeat(32), name: "Neurology", description: "CNS review",
    sourceNoteCount: 0, generatedCardCount: 0, studyableDueCount: 0,
    archivedAt: null, createdAt: 1, updatedAt: 1, revision: 1,
  };
  const create = vi.fn().mockResolvedValue(created);
  const refresh = vi.fn().mockResolvedValue([created]);
  const open = vi.fn();
  const dependencies: DeckLibraryControllerDependencies = {
    getOrganization: vi.fn().mockResolvedValue({ revision: 1, folders: [], placements: [] }),
    createFolder: vi.fn(), updateFolder: vi.fn(), deleteFolder: vi.fn(),
    moveDeck: vi.fn(), moveFolder: vi.fn(),
    createDeckController: () => ({
      create, setArchived: vi.fn(), deleteDeck: vi.fn(), undo: vi.fn(), invalidate: vi.fn(),
    }),
  };
  const { result } = renderHook(() => useDeckLibraryController({
    decks: [], refreshDecks: refresh, onOpenDeck: open,
    showToast: vi.fn(), confirm: vi.fn().mockResolvedValue(true), dependencies,
  }));
  await waitFor(() => expect(result.current.isLoading).toBe(false));
  await act(async () => { await result.current.createDeck("Neurology", "CNS review"); });
  expect(create).toHaveBeenCalledWith({ name: "Neurology", description: "CNS review" });
  expect(refresh).toHaveBeenCalledOnce();
  expect(open).toHaveBeenCalledWith(created.id);
});
