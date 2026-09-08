import { invoke } from "@tauri-apps/api/core";

import type { SourceLocator } from "./studyGuides";

export type StudyScope =
  | { kind: "general" | "library"; id: null }
  | { kind: "deck" | "document" | "guide"; id: string };

export interface ScopeChoice {
  scope: StudyScope;
  title: string;
}

export type SourceKind = "document" | "card" | "guide" | "conversation";
export type ContentOrigin = "source_document" | "user_authored" | "generated" | "conversation";

export interface SourceCitation {
  id: string;
  kind: SourceKind;
  ownerId: string;
  title: string;
  excerpt: string;
  locator: SourceLocator | null;
  origin: ContentOrigin;
  contentHash: string;
}

export interface PreparationScope {
  scope: StudyScope;
  includeConversations: boolean;
}

export interface PreparationPreview {
  selection: PreparationScope;
  consentKey: string;
  sourceCount: number;
  sourceCharacters: number;
  imageCount: number;
  extractionProvider: string;
  extractionModel: string;
  embeddingProvider: string;
  embeddingModel: string;
  embeddingDimensions: number | null;
}

export interface PreparationStatus {
  scopeKey: string;
  operationId: string;
  consentKey: string;
  embeddingSpace: string;
  state: "running" | "complete" | "failed" | "interrupted" | "canceled";
  totalSources: number;
  completedSources: number;
  errorCode: string;
  errorMessage: string;
  updatedAt: number;
}

export interface LearningTaskProgress {
  operationId: string;
  phase: string;
  completed: number;
  total: number;
  canceled: boolean;
}

export interface SemanticSearchResult {
  sources: SourceCitation[];
  eligibleEntries: number;
  embeddingSpace: string;
}

export async function listSemanticScopeChoices(): Promise<ScopeChoice[]> {
  return invoke<ScopeChoice[]>("list_semantic_scope_choices");
}

/** Reading this preview never calls a provider. */
export async function previewSemanticPreparation(
  input: PreparationScope,
): Promise<PreparationPreview> {
  return invoke<PreparationPreview>("preview_semantic_preparation", { input });
}

/** The caller retains this operation identity until the durable result is reconciled. */
export async function prepareSemanticMaterial(input: {
  operationId: string;
  selection: PreparationScope;
  consentKey: string;
}): Promise<PreparationStatus> {
  return invoke<PreparationStatus>("prepare_semantic_material", { input });
}

export async function getSemanticPreparation(
  input: PreparationScope,
): Promise<PreparationStatus | null> {
  return invoke<PreparationStatus | null>("get_semantic_preparation", { input });
}

export async function getLearningTaskProgress(
  operationId: string,
): Promise<LearningTaskProgress | null> {
  return invoke<LearningTaskProgress | null>("get_learning_task_progress", { operationId });
}

export async function cancelLearningTask(operationId: string): Promise<void> {
  return invoke<void>("cancel_learning_task", { operationId });
}

export async function searchSemanticMaterial(input: {
  operationId: string;
  selection: PreparationScope;
  query: string;
}): Promise<SemanticSearchResult> {
  return invoke<SemanticSearchResult>("search_semantic_material", { input });
}

export async function getSemanticSourceCitation(input: {
  sourceId: string;
  contentHash: string;
}): Promise<SourceCitation> {
  return invoke<SourceCitation>("get_semantic_source_citation", { input });
}

export async function clearSemanticIndex(): Promise<void> {
  return invoke<void>("clear_semantic_index");
}

export function studyScopeKey(scope: StudyScope): string {
  return `${scope.kind}:${scope.id ?? ""}`;
}

export function sameStudyScope(left: StudyScope, right: StudyScope): boolean {
  return left.kind === right.kind && left.id === right.id;
}
