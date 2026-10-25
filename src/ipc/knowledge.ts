import { invoke } from "./transport";

import type { SourceLocator } from "./studyGuides";

export type MaterialScope = { kind: "deck" | "document" | "guide" | "collection"; id: string };
export type StudyScope =
  | { kind: "general" | "library"; id: null }
  | MaterialScope
  | { kind: "selection"; id: null; members: MaterialScope[] };

/** Canonical order matches native scope serialization and permission identity. */
export function selectedScope(members: MaterialScope[]): StudyScope {
  const unique = [...new Map(members.map((member) => [studyScopeKey(member), member])).values()];
  unique.sort((a, b) => {
    const left = JSON.stringify({ kind: a.kind, id: a.id });
    const right = JSON.stringify({ kind: b.kind, id: b.id });
    return left < right ? -1 : left > right ? 1 : 0;
  });
  if (unique.length > 16) throw new Error("Select at most 16 study sources.");
  if (!unique.length) return { kind: "general", id: null };
  if (unique.length === 1) {
    const only = unique[0];
    if (only) return only;
  }
  return { kind: "selection", id: null, members: unique };
}

export function scopeMembers(scope: StudyScope): MaterialScope[] {
  if (scope.kind === "selection") return scope.members;
  if (scope.kind === "deck" || scope.kind === "document" || scope.kind === "guide" || scope.kind === "collection") return [scope];
  return [];
}


export type CollectionRelationshipKind = "complementary" | "contrast" | "contradictory" | "prerequisite";

export interface CollectionRelationship {
  id: string;
  kind: CollectionRelationshipKind;
  topicIds: string[];
  supportUnitIds: string[];
  note: string;
}

export interface CollectionTopic {
  id: string;
  title: string;
  summary: string;
  summarySupportUnitIds: string[];
  unitIds: string[];
  supportSourceIds: string[];
  supportDocumentIds: string[];
  emphasisSourceIds: string[];
}

export interface CollectionGap {
  kind: string;
  count: number;
  documentId: string | null;
  sourceId: string | null;
  unitIds: string[];
  detail: string;
}

export interface CollectionSynthesis {
  version: number;
  collectionId: string;
  snapshotId: string;
  summary: string;
  learningObjectives: string[];
  topics: CollectionTopic[];
  relationships: CollectionRelationship[];
  gaps: CollectionGap[];
  reductionStatus: string;
}

export interface CollectionViewState {
  synthesis: CollectionSynthesis;
  stale: boolean;
  reduced: boolean;
  analysisHash: string;
}

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
  state: "running" | "complete" | "partial" | "failed" | "interrupted" | "canceled";
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
  /**
   * A Tutor explanation as it is being written: unchecked provider text that
   * the finished, checked turn replaces. Null for every other task.
   */
  draft: string | null;
}

export interface SemanticSearchResult {
  sources: SourceCitation[];
  eligibleEntries: number;
  embeddingSpace: string;
}


/** Reads the retained compact view only; this never calls a provider. */
export async function getCollectionSynthesis(collectionId: string): Promise<CollectionViewState | null> {
  return invoke<CollectionViewState | null>("get_collection_synthesis", { collectionId });
}

/** Rebuilds the compact view locally from retained knowledge and coverage receipts. */
export async function refreshCollectionSynthesis(collectionId: string): Promise<CollectionViewState> {
  return invoke<CollectionViewState>("refresh_collection_synthesis", { collectionId });
}

/**
 * Explicit generation/preparation path. It may make at most one bounded provider
 * request after the complete local collection coverage plan is frozen.
 */
export async function prepareCollectionSynthesis(input: {
  operationId: string;
  collectionId: string;
}): Promise<CollectionViewState> {
  return invoke<CollectionViewState>("prepare_collection_synthesis", { input });
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
  return scope.kind === "selection"
    ? `selection:${scope.members.map(studyScopeKey).sort().join("|")}`
    : `${scope.kind}:${scope.id ?? ""}`;
}

export function sameStudyScope(left: StudyScope, right: StudyScope): boolean {
  return studyScopeKey(left) === studyScopeKey(right);
}
