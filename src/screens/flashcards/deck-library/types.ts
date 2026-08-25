import type { DeckFolder } from "../../../ipc/deckFolders";

export interface FolderDraft {
  mode: "create" | "rename";
  folder: DeckFolder | null;
  name: string;
}

export type LibraryMoveSource = "keyboard" | "html-drag" | "pointer" | "fallback";

export type LibraryMoveIntent =
  | {
      kind: "deck";
      deckId: string;
      targetFolderId: string | null;
      targetIndex: number;
      source: LibraryMoveSource;
    }
  | {
      kind: "folder";
      folderId: string;
      targetIndex: number;
      source: LibraryMoveSource;
    };

export type LibraryDragPayload =
  | { kind: "deck"; id: string }
  | { kind: "folder"; id: string };

export const DECK_DRAG_MIME = "application/x-ostudy-deck-id";
export const FOLDER_DRAG_MIME = "application/x-ostudy-folder-id";
export const UNSORTED_DATA_VALUE = "__unsorted__";
