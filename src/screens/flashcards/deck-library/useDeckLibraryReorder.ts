import {
  useCallback,
  useEffect,
  useRef,
  type DragEvent,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";

import type { DeckLibrarySnapshot } from "./organizationModel";
import { orderedDecksForFolder } from "./organizationModel";
import type { LibraryDragPayload, LibraryMoveIntent } from "./types";

export const DECK_DRAG_MIME = "application/x-ostudy-deck-id";
export const FOLDER_DRAG_MIME = "application/x-ostudy-folder-id";
export const UNSORTED_DATA_VALUE = "__unsorted__";

interface PointerDragState {
  payload: LibraryDragPayload;
  startX: number;
  startY: number;
  active: boolean;
}

export interface DeckLibraryReorderAdapter {
  startDeckDrag(event: DragEvent<HTMLElement>, deckId: string): void;
  startFolderDrag(event: DragEvent<HTMLElement>, folderId: string): void;
  dropDeck(
    event: DragEvent<HTMLElement>,
    targetFolderId: string | null,
    targetIndex: number,
  ): void;
  dropFolder(
    event: DragEvent<HTMLElement>,
    targetFolderId: string,
    targetFolderIndex: number,
    targetDeckCount: number,
  ): void;
  dropFolderTail(event: DragEvent<HTMLElement>): void;
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
  ): void;
  updatePointerDrag(event: ReactPointerEvent<HTMLElement>): void;
  finishPointerDrag(event: ReactPointerEvent<HTMLElement>): void;
  cancelPointerDrag(event: ReactPointerEvent<HTMLElement>): void;
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

export function isCompatiblePointerTarget(
  payload: LibraryDragPayload,
  target: Pick<HTMLElement, "dataset">,
): boolean {
  const kind = target.dataset.libraryDrop;
  if (payload.kind === "folder") return kind === "folder" || kind === "folder-tail";
  return kind === "deck" || kind === "folder" || kind === "unsorted";
}

export function useDeckLibraryReorder(
  snapshot: DeckLibrarySnapshot,
  disabled: boolean,
  onMove: (intent: LibraryMoveIntent) => void | Promise<void>,
): DeckLibraryReorderAdapter {
  const pointerDragRef = useRef<PointerDragState | null>(null);
  const pointerDropTargetRef = useRef<HTMLElement | null>(null);

  const clearPointerDropTarget = useCallback((): void => {
    if (pointerDropTargetRef.current) {
      delete pointerDropTargetRef.current.dataset.dragOver;
    }
    pointerDropTargetRef.current = null;
  }, []);

  const setPointerDropTarget = useCallback(
    (target: HTMLElement | null, kind: LibraryDragPayload["kind"]): void => {
      if (pointerDropTargetRef.current === target) return;
      clearPointerDropTarget();
      pointerDropTargetRef.current = target;
      if (target) target.dataset.dragOver = kind;
    },
    [clearPointerDropTarget],
  );

  useEffect(() => {
    return () => clearPointerDropTarget();
  }, [clearPointerDropTarget]);

  const emit = useCallback(
    (intent: LibraryMoveIntent): void => {
      if (!disabled) void onMove(intent);
    },
    [disabled, onMove],
  );

  const dropDeck = useCallback(
    (
      event: DragEvent<HTMLElement>,
      targetFolderId: string | null,
      targetIndex: number,
    ): void => {
      event.preventDefault();
      event.stopPropagation();
      if (disabled) return;
      const deckId = readTransferValue(event, DECK_DRAG_MIME);
      if (snapshot.activeDecks.some((deck) => deck.id === deckId)) {
        emit({
          kind: "deck",
          deckId,
          targetFolderId,
          targetIndex,
          source: "html-drag",
        });
      }
    },
    [disabled, emit, snapshot.activeDecks],
  );

  const dropFolder = useCallback(
    (
      event: DragEvent<HTMLElement>,
      targetFolderId: string,
      targetFolderIndex: number,
      targetDeckCount: number,
    ): void => {
      event.preventDefault();
      event.stopPropagation();
      if (disabled) return;
      const folderId = readTransferValue(event, FOLDER_DRAG_MIME);
      if (snapshot.folders.some((folder) => folder.id === folderId)) {
        emit({
          kind: "folder",
          folderId,
          targetIndex: targetFolderIndex,
          source: "html-drag",
        });
        return;
      }
      const deckId = readTransferValue(event, DECK_DRAG_MIME);
      if (snapshot.activeDecks.some((deck) => deck.id === deckId)) {
        emit({
          kind: "deck",
          deckId,
          targetFolderId,
          targetIndex: targetDeckCount,
          source: "html-drag",
        });
      }
    },
    [disabled, emit, snapshot.activeDecks, snapshot.folders],
  );

  const finishPointerDrag = useCallback(
    (event: ReactPointerEvent<HTMLElement>, commit: boolean): void => {
      const state = pointerDragRef.current;
      const target = pointerDropTargetRef.current;
      pointerDragRef.current = null;
      if (
        typeof event.currentTarget.hasPointerCapture === "function" &&
        event.currentTarget.hasPointerCapture(event.pointerId)
      ) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }
      clearPointerDropTarget();
      if (!commit || !state?.active || !target || disabled) return;

      if (state.payload.kind === "folder") {
        const targetIndex = Number(target.dataset.folderIndex);
        if (Number.isInteger(targetIndex)) {
          emit({
            kind: "folder",
            folderId: state.payload.id,
            targetIndex,
            source: "pointer",
          });
        }
        return;
      }

      const targetIndex = Number(target.dataset.targetIndex);
      if (!Number.isInteger(targetIndex)) return;
      const encodedFolderId = target.dataset.folderId;
      const targetFolderId =
        !encodedFolderId || encodedFolderId === UNSORTED_DATA_VALUE ? null : encodedFolderId;
      emit({
        kind: "deck",
        deckId: state.payload.id,
        targetFolderId,
        targetIndex,
        source: "pointer",
      });
    },
    [clearPointerDropTarget, disabled, emit],
  );

  return {
    startDeckDrag: (event, deckId) => {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData(DECK_DRAG_MIME, deckId);
      event.dataTransfer.setData("text/plain", deckId);
    },
    startFolderDrag: (event, folderId) => {
      event.dataTransfer.effectAllowed = "move";
      event.dataTransfer.setData(FOLDER_DRAG_MIME, folderId);
      event.dataTransfer.setData("text/plain", folderId);
    },
    dropDeck,
    dropFolder,
    dropFolderTail: (event) => {
      event.preventDefault();
      event.stopPropagation();
      if (disabled) return;
      const folderId = readTransferValue(event, FOLDER_DRAG_MIME);
      if (snapshot.folders.some((folder) => folder.id === folderId)) {
        emit({
          kind: "folder",
          folderId,
          targetIndex: Math.max(0, snapshot.folders.length - 1),
          source: "html-drag",
        });
      }
    },
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
    beginPointerDrag: (event, payload) => {
      if (event.pointerType === "mouse" || event.button !== 0 || disabled) return;
      pointerDragRef.current = {
        payload,
        startX: event.clientX,
        startY: event.clientY,
        active: false,
      };
      if (typeof event.currentTarget.setPointerCapture === "function") {
        event.currentTarget.setPointerCapture(event.pointerId);
      }
    },
    updatePointerDrag: (event) => {
      const state = pointerDragRef.current;
      if (!state) return;
      if (!state.active) {
        const distance = Math.hypot(event.clientX - state.startX, event.clientY - state.startY);
        if (distance < 8) return;
        state.active = true;
      }
      event.preventDefault();
      const candidate = document
        .elementFromPoint(event.clientX, event.clientY)
        ?.closest<HTMLElement>("[data-library-drop]");
      const compatible =
        candidate && isCompatiblePointerTarget(state.payload, candidate) ? candidate : null;
      setPointerDropTarget(compatible, state.payload.kind);
    },
    finishPointerDrag: (event) => finishPointerDrag(event, true),
    cancelPointerDrag: (event) => finishPointerDrag(event, false),
  };
}

function readTransferValue(event: DragEvent<HTMLElement>, type: string): string {
  try {
    return event.dataTransfer.getData(type);
  } catch {
    return "";
  }
}
