import { invoke } from "@tauri-apps/api/core";

import type { AnalyticsRange } from "./analytics";
import type { FlashcardsWorkspaceName } from "../navigation/flashcardsRoutes";

export type CheckpointSection =
  | "decks"
  | "study-guides"
  | "documents"
  | "analytics"
  | "settings";

export interface CheckpointReturnTarget {
  section: CheckpointSection;
  flashcardsWorkspace: FlashcardsWorkspaceName | null;
  deckId: string | null;
}

export interface PracticeCursorCheckpoint {
  sessionId: string;
  position: number;
  revealed: boolean;
}

export interface PracticeCheckpoint extends PracticeCursorCheckpoint {
  returnTarget: CheckpointReturnTarget;
}

export interface AnalyticsCheckpoint {
  range: AnalyticsRange;
  deckIds: string[];
  tagIds: string[];
}

export interface UiCheckpoint {
  schemaVersion: 2;
  updatedAt: number;
  section: CheckpointSection;
  flashcardsWorkspace: FlashcardsWorkspaceName | null;
  deckId: string | null;
  editorDraftId: string | null;
  practice: PracticeCheckpoint | null;
  analytics: AnalyticsCheckpoint | null;
}

export interface SaveUiCheckpointInput {
  section: CheckpointSection;
  flashcardsWorkspace?: FlashcardsWorkspaceName;
  deckId?: string;
  editorDraftId?: string;
  practice?: PracticeCheckpoint;
  analytics?: AnalyticsCheckpoint;
}

export async function getUiCheckpoint(): Promise<UiCheckpoint | null> {
  return invoke<UiCheckpoint | null>("get_ui_checkpoint");
}

export async function saveUiCheckpoint(input: SaveUiCheckpointInput): Promise<UiCheckpoint> {
  return invoke<UiCheckpoint>("save_ui_checkpoint", { input });
}

export async function clearUiCheckpoint(): Promise<void> {
  return invoke<void>("clear_ui_checkpoint");
}
