import { act, renderHook } from "@testing-library/react";
import type {
  DragEvent,
  KeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from "react";
import { describe, expect, it, vi } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import type { DeckLibraryOrganization } from "../../../ipc/deckFolders";
import { createDeckLibrarySnapshot } from "./organizationModel";
import {
  DECK_DRAG_MIME,
  FOLDER_DRAG_MIME,
  isCompatiblePointerTarget,
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

  it("accepts only semantically compatible pointer drop targets", () => {
    expect(isCompatiblePointerTarget({ kind: "folder", id: folderA.id }, target("folder"))).toBe(
      true,
    );
    expect(isCompatiblePointerTarget({ kind: "folder", id: folderA.id }, target("deck"))).toBe(
      false,
    );
    expect(isCompatiblePointerTarget({ kind: "deck", id: deckA.id }, target("unsorted"))).toBe(
      true,
    );
    expect(isCompatiblePointerTarget({ kind: "deck", id: deckA.id }, target("folder-tail"))).toBe(
      false,
    );
  });
});

describe("useDeckLibraryReorder", () => {
  it("translates HTML drag, keyboard, and fallback controls to the same deck move semantics", () => {
    const onMove = vi.fn();
    const { result } = renderHook(() => useDeckLibraryReorder(snapshot, false, onMove));

    act(() => {
      result.current.dropDeck(
        dragEvent({ [DECK_DRAG_MIME]: deckB.id }),
        folderA.id,
        1,
      );
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
      targetFolderId: folderA.id,
      targetIndex: 1,
      source: "html-drag",
    });
    expect(onMove).toHaveBeenNthCalledWith(2, {
      kind: "deck",
      deckId: deckB.id,
      targetFolderId: folderB.id,
      targetIndex: 0,
      source: "keyboard",
    });
    expect(onMove).toHaveBeenNthCalledWith(3, {
      kind: "deck",
      deckId: deckB.id,
      targetFolderId: folderA.id,
      targetIndex: 1,
      source: "fallback",
    });
  });

  it("distinguishes a folder drag from a deck dropped on a folder", () => {
    const onMove = vi.fn();
    const { result } = renderHook(() => useDeckLibraryReorder(snapshot, false, onMove));

    act(() => {
      result.current.dropFolder(
        dragEvent({ [FOLDER_DRAG_MIME]: folderA.id }),
        folderB.id,
        1,
        0,
      );
      result.current.dropFolder(
        dragEvent({ [DECK_DRAG_MIME]: deckB.id }),
        folderA.id,
        0,
        1,
      );
    });

    expect(onMove).toHaveBeenNthCalledWith(1, {
      kind: "folder",
      folderId: folderA.id,
      targetIndex: 1,
      source: "html-drag",
    });
    expect(onMove).toHaveBeenNthCalledWith(2, {
      kind: "deck",
      deckId: deckB.id,
      targetFolderId: folderA.id,
      targetIndex: 1,
      source: "html-drag",
    });
  });

  it("commits a touch pointer gesture through the same intent boundary and clears DOM state", () => {
    const onMove = vi.fn();
    const { result } = renderHook(() => useDeckLibraryReorder(snapshot, false, onMove));
    const dropTarget = document.createElement("div");
    dropTarget.dataset.libraryDrop = "folder";
    dropTarget.dataset.folderId = folderA.id;
    dropTarget.dataset.targetIndex = "1";
    const originalElementFromPoint = document.elementFromPoint;
    const elementFromPoint = vi.fn(() => dropTarget);
    Object.defineProperty(document, "elementFromPoint", {
      configurable: true,
      value: elementFromPoint,
    });
    const captureTarget = pointerCaptureTarget();

    act(() => {
      result.current.beginPointerDrag(
        pointerEvent(captureTarget, { pointerType: "touch", clientX: 0, clientY: 0 }),
        { kind: "deck", id: deckB.id },
      );
      result.current.updatePointerDrag(
        pointerEvent(captureTarget, { pointerType: "touch", clientX: 20, clientY: 0 }),
      );
    });
    expect(dropTarget.dataset.dragOver).toBe("deck");

    act(() => {
      result.current.finishPointerDrag(
        pointerEvent(captureTarget, { pointerType: "touch", clientX: 20, clientY: 0 }),
      );
    });
    expect(onMove).toHaveBeenCalledWith({
      kind: "deck",
      deckId: deckB.id,
      targetFolderId: folderA.id,
      targetIndex: 1,
      source: "pointer",
    });
    expect(dropTarget.dataset.dragOver).toBeUndefined();
    expect(captureTarget.releasePointerCapture).toHaveBeenCalled();
    Object.defineProperty(document, "elementFromPoint", {
      configurable: true,
      value: originalElementFromPoint,
    });
  });

  it("suppresses every input mode while a mutation is active", () => {
    const onMove = vi.fn();
    const { result } = renderHook(() => useDeckLibraryReorder(snapshot, true, onMove));
    act(() => {
      result.current.dropDeck(dragEvent({ [DECK_DRAG_MIME]: deckB.id }), folderA.id, 1);
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

function dragEvent(data: Record<string, string>): DragEvent<HTMLElement> {
  return {
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
    dataTransfer: {
      effectAllowed: "uninitialized",
      getData: (type: string) => data[type] ?? "",
      setData: vi.fn(),
    },
  } as unknown as DragEvent<HTMLElement>;
}

function keyboardEvent(key: string): KeyboardEvent<HTMLElement> {
  return { key, preventDefault: vi.fn() } as unknown as KeyboardEvent<HTMLElement>;
}

function pointerCaptureTarget() {
  return {
    setPointerCapture: vi.fn(),
    hasPointerCapture: vi.fn().mockReturnValue(true),
    releasePointerCapture: vi.fn(),
  };
}

function pointerEvent(
  currentTarget: ReturnType<typeof pointerCaptureTarget>,
  overrides: Partial<ReactPointerEvent<HTMLElement>>,
): ReactPointerEvent<HTMLElement> {
  return {
    pointerId: 9,
    button: 0,
    pointerType: "touch",
    clientX: 0,
    clientY: 0,
    preventDefault: vi.fn(),
    currentTarget,
    ...overrides,
  } as unknown as ReactPointerEvent<HTMLElement>;
}

function target(kind: string): Pick<HTMLElement, "dataset"> {
  return { dataset: { libraryDrop: kind } } as Pick<HTMLElement, "dataset">;
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
