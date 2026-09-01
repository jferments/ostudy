import { act, renderHook } from "@testing-library/react";
import type { KeyboardEvent } from "react";
import { describe, expect, it, vi } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import type { DeckLibraryOrganization } from "../../../ipc/deckFolders";
import { createDeckLibrarySnapshot } from "./organizationModel";
import {
  resolveDeckKeyboardMove,
  resolveFolderKeyboardMove,
  useDeckLibraryReorder,
} from "./useDeckLibraryReorder";

const folderA = folder("f", "Nursing", 1_000);
const folderB = folder("g", "Clinical", 2_000);
const deckA = deck("a", "Pharmacology");
const deckB = deck("b", "Anatomy");
const organization: DeckLibraryOrganization = {
  revision: 4,
  folders: [folderA, folderB],
  placements: [
    { deckId: deckA.id, folderId: folderA.id, sortOrder: 1_000 },
    { deckId: deckB.id, folderId: null, sortOrder: 1_000 },
  ],
};
const snapshot = createDeckLibrarySnapshot([deckA, deckB], organization);

describe("deck-library reorder policy", () => {
  it.each([
    ["ArrowUp", 1, 2, 0],
    ["ArrowDown", 0, 2, 1],
    ["Home", 1, 2, 0],
    ["End", 0, 2, 1],
  ])("maps folder key %s to one folder intent", (key, index, count, targetIndex) => {
    expect(resolveFolderKeyboardMove(key, folderA.id, index, count)).toEqual({
      kind: "folder",
      folderId: folderA.id,
      targetIndex,
      source: "keyboard",
    });
  });

  it("maps horizontal deck keys across ordered folder groups and Unsorted", () => {
    expect(resolveDeckKeyboardMove("ArrowRight", snapshot, deckA.id, folderA.id, 0, 1)).toEqual({
      kind: "deck",
      deckId: deckA.id,
      targetFolderId: folderB.id,
      targetIndex: 0,
      source: "keyboard",
    });
    expect(resolveDeckKeyboardMove("ArrowLeft", snapshot, deckB.id, null, 0, 1)).toEqual({
      kind: "deck",
      deckId: deckB.id,
      targetFolderId: folderB.id,
      targetIndex: 0,
      source: "keyboard",
    });
  });

  it("rejects out-of-range keyboard movement instead of wrapping", () => {
    expect(resolveFolderKeyboardMove("ArrowUp", folderA.id, 0, 2)).toBeNull();
    expect(resolveDeckKeyboardMove("ArrowUp", snapshot, deckA.id, folderA.id, 0, 1)).toBeNull();
    expect(resolveDeckKeyboardMove("ArrowLeft", snapshot, deckA.id, folderA.id, 0, 1)).toBeNull();
  });

});

describe("useDeckLibraryReorder", () => {
  it("translates keyboard and fallback controls to validated deck move semantics", () => {
    const onMove = vi.fn();
    const { result } = renderHook(() => useDeckLibraryReorder(snapshot, false, onMove));

    act(() => {
      result.current.moveDeckWithKeyboard(
        keyboardEvent("ArrowLeft"),
        deckB.id,
        null,
        0,
        1,
      );
      result.current.moveDeckWithFallback(deckB.id, folderA.id, 1);
    });

    expect(onMove).toHaveBeenNthCalledWith(1, {
      kind: "deck",
      deckId: deckB.id,
      targetFolderId: folderB.id,
      targetIndex: 0,
      source: "keyboard",
    });
    expect(onMove).toHaveBeenNthCalledWith(2, {
      kind: "deck",
      deckId: deckB.id,
      targetFolderId: folderA.id,
      targetIndex: 1,
      source: "fallback",
    });
  });

  it("delegates fine and coarse pointer starts to the shared pointer session", () => {
    const onMove = vi.fn();
    const beginPointerDrag = vi.fn();
    const { result } = renderHook(() =>
      useDeckLibraryReorder(snapshot, false, onMove, beginPointerDrag),
    );
    const event = {} as never;

    act(() => {
      result.current.beginPointerDrag(event, { kind: "deck", id: deckB.id }, deckB.name);
    });
    expect(beginPointerDrag).toHaveBeenCalledWith(
      event,
      { kind: "deck", id: deckB.id },
      deckB.name,
    );
    expect(onMove).not.toHaveBeenCalled();
  });

  it("suppresses every input mode while a mutation is active", () => {
    const onMove = vi.fn();
    const { result } = renderHook(() => useDeckLibraryReorder(snapshot, true, onMove));
    act(() => {
      result.current.moveDeckWithKeyboard(
        keyboardEvent("ArrowLeft"),
        deckB.id,
        null,
        0,
        1,
      );
      result.current.moveDeckWithFallback(deckB.id, folderA.id, 1);
    });
    expect(onMove).not.toHaveBeenCalled();
  });
});

function keyboardEvent(key: string): KeyboardEvent<HTMLElement> {
  return { key, preventDefault: vi.fn() } as unknown as KeyboardEvent<HTMLElement>;
}

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

function folder(seed: string, name: string, sortOrder: number) {
  return {
    id: seed.repeat(32),
    name,
    sortOrder,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}
