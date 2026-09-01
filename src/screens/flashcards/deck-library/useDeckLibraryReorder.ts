import {
  useCallback,
  useMemo,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";

import type { DeckLibrarySnapshot } from "./organizationModel";
import { orderedDecksForFolder, validateMoveIntent } from "./organizationModel";
import type { LibraryDragPayload, LibraryMoveIntent } from "./types";

export { UNSORTED_DATA_VALUE } from "./pointerDragSession";

export interface DeckLibraryReorderAdapter {
  moveDeckWithKeyboard(
    event: KeyboardEvent<HTMLElement>,
    deckId: string,
    folderId: string | null,
    index: number,
    count: number,
  ): void;
  moveFolderWithKeyboard(
    event: KeyboardEvent<HTMLElement>,
    folderId: string,
    index: number,
  ): void;
  moveDeckWithFallback(
    deckId: string,
    targetFolderId: string | null,
    targetIndex: number,
  ): void;
  beginPointerDrag(
    event: ReactPointerEvent<HTMLElement>,
    payload: LibraryDragPayload,
    sourceName: string,
  ): void;
}

export function resolveFolderKeyboardMove(
  key: string,
  folderId: string,
  index: number,
  folderCount: number,
): LibraryMoveIntent | null {
  let targetIndex: number | null = null;
  if (key === "ArrowUp") targetIndex = index - 1;
  if (key === "ArrowDown") targetIndex = index + 1;
  if (key === "Home") targetIndex = 0;
  if (key === "End") targetIndex = folderCount - 1;
  if (targetIndex === null || targetIndex < 0 || targetIndex >= folderCount) return null;
  return { kind: "folder", folderId, targetIndex, source: "keyboard" };
}

export function resolveDeckKeyboardMove(
  key: string,
  snapshot: DeckLibrarySnapshot,
  deckId: string,
  folderId: string | null,
  index: number,
  count: number,
): LibraryMoveIntent | null {
  let targetIndex: number | null = null;
  if (key === "ArrowUp") targetIndex = index - 1;
  if (key === "ArrowDown") targetIndex = index + 1;
  if (key === "Home") targetIndex = 0;
  if (key === "End") targetIndex = count - 1;
  if (targetIndex !== null) {
    if (targetIndex < 0 || targetIndex >= count) return null;
    return {
      kind: "deck",
      deckId,
      targetFolderId: folderId,
      targetIndex,
      source: "keyboard",
    };
  }

  if (key !== "ArrowLeft" && key !== "ArrowRight") return null;
  const groups: Array<string | null> = [...snapshot.folders.map((folder) => folder.id), null];
  const currentGroupIndex = groups.findIndex((candidate) => candidate === folderId);
  const nextGroupIndex = currentGroupIndex + (key === "ArrowLeft" ? -1 : 1);
  if (nextGroupIndex < 0 || nextGroupIndex >= groups.length) return null;
  const targetFolderId = groups[nextGroupIndex];
  if (targetFolderId === undefined) return null;
  const targetCount = orderedDecksForFolder(
    snapshot.activeDecks.filter((candidate) => candidate.id !== deckId),
    snapshot.placementByDeck,
    targetFolderId,
  ).length;
  return {
    kind: "deck",
    deckId,
    targetFolderId,
    targetIndex: targetCount,
    source: "keyboard",
  };
}

export function useDeckLibraryReorder(
  snapshot: DeckLibrarySnapshot,
  disabled: boolean,
  onMove: (intent: LibraryMoveIntent) => void | Promise<void>,
  beginPointerDrag: DeckLibraryReorderAdapter["beginPointerDrag"] = () => undefined,
): DeckLibraryReorderAdapter {
  const emit = useCallback(
    (intent: LibraryMoveIntent): void => {
      if (disabled) return;
      const validation = validateMoveIntent(snapshot, intent);
      if (validation.status === "valid") void onMove(validation.intent);
    },
    [disabled, onMove, snapshot],
  );

  return useMemo(() => ({
    moveDeckWithKeyboard: (event, deckId, folderId, index, count) => {
      const intent = resolveDeckKeyboardMove(
        event.key,
        snapshot,
        deckId,
        folderId,
        index,
        count,
      );
      if (!intent) return;
      event.preventDefault();
      emit(intent);
    },
    moveFolderWithKeyboard: (event, folderId, index) => {
      const intent = resolveFolderKeyboardMove(
        event.key,
        folderId,
        index,
        snapshot.folders.length,
      );
      if (!intent) return;
      event.preventDefault();
      emit(intent);
    },
    moveDeckWithFallback: (deckId, targetFolderId, targetIndex) =>
      emit({ kind: "deck", deckId, targetFolderId, targetIndex, source: "fallback" }),
    beginPointerDrag,
  }), [beginPointerDrag, emit, snapshot]);
}
