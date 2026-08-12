import { invoke } from "@tauri-apps/api/core";

export type EditorDraftKind = "card" | "template" | "bulk_cards" | "ai_profile";
export type EditorDraftState = "active" | "committed" | "discarded" | "conflicted";

export interface EditorDraft<T = Record<string, unknown>> {
  id: string;
  draftKind: EditorDraftKind;
  targetEntityId: string | null;
  contextEntityId: string | null;
  baseRevision: number | null;
  payloadSchemaVersion: number;
  payload: T;
  revision: number;
  state: EditorDraftState;
  assetDraftId: string | null;
  createdAt: number;
  updatedAt: number;
  lastOpenedAt: number;
}

export interface OpenEditorDraftInput<T> {
  operationId: string;
  draftKind: EditorDraftKind;
  targetEntityId?: string;
  contextEntityId?: string;
  baseRevision?: number;
  payloadSchemaVersion: number;
  initialPayload: T;
}

export interface SaveEditorDraftInput<T> {
  draftId: string;
  revision: number;
  payloadSchemaVersion: number;
  payload: T;
  assetIds: string[];
}

export async function openEditorDraft<T extends Record<string, unknown>>(
  input: OpenEditorDraftInput<T>,
): Promise<EditorDraft<T>> {
  return invoke<EditorDraft<T>>("open_editor_draft", { input });
}

export async function saveEditorDraft<T extends Record<string, unknown>>(
  input: SaveEditorDraftInput<T>,
): Promise<EditorDraft<T>> {
  return invoke<EditorDraft<T>>("save_editor_draft", { input });
}

export async function discardEditorDraft(draftId: string, revision: number): Promise<void> {
  return invoke<void>("discard_editor_draft", { input: { draftId, revision } });
}

export async function commitEditorDraft(
  draftId: string,
  revision: number,
  targetEntityId?: string,
): Promise<void> {
  return invoke<void>("commit_editor_draft", {
    input: { draftId, revision, targetEntityId },
  });
}

export async function listEditorDrafts(): Promise<EditorDraft[]> {
  return invoke<EditorDraft[]>("list_editor_drafts");
}
