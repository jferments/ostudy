import type { DeckSummary } from "../../../ipc/decks";
import type { DeckFolder } from "../../../ipc/deckFolders";

export type LibraryMoveSource = "keyboard" | "pointer" | "fallback";

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

export type LibrarySheetState =
  | { kind: "create-deck" }
  | { kind: "create-folder"; name: string }
  | { kind: "rename-folder"; folder: DeckFolder; name: string }
  | { kind: "move-selection" }
  | { kind: "practice-setup"; scope: import("./practiceSetupModel").PracticeDeckScope };

export interface DeckLifecyclePartition {
  active: DeckSummary[];
  archived: DeckSummary[];
}
