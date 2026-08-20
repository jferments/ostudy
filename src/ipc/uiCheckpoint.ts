import { invoke } from "@tauri-apps/api/core";

import type { FlashcardsWorkspaceName } from "../navigation/flashcardsRoutes";
import type { AnalyticsRange } from "./analytics";

export type CheckpointSection =
  | "decks"
  | "documents"
  | "cards"
  | "templates"
  | "practice"
  | "analytics"
  | "settings";

export type FlashcardsCheckpointView = "root" | "deck";

export interface PracticeCheckpoint {
  sessionId: string;
  position: number;
  revealed: boolean;
}

export interface AnalyticsCheckpoint {
  range: AnalyticsRange;
  deckIds: string[];
  tagIds: string[];
}

export interface UiCheckpoint {
  schemaVersion: 1;
  updatedAt: number;
  section: CheckpointSection;
  flashcardsWorkspace: FlashcardsWorkspaceName | null;
  flashcardsView: FlashcardsCheckpointView | null;
  deckId: string | null;
  editorDraftId: string | null;
  practice: PracticeCheckpoint | null;
  analytics: AnalyticsCheckpoint | null;
}

export interface SaveUiCheckpointInput {
  section: CheckpointSection;
  flashcardsWorkspace?: FlashcardsWorkspaceName;
  flashcardsView?: FlashcardsCheckpointView;
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
