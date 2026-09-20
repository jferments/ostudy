import { invoke } from "@tauri-apps/api/core";

import { createOperationId } from "./common";
import type {
  PreparedStudyGuidePdf,
  SourceLocator,
  StudyGuideOverview,
  StudyGuideSectionContent,
} from "./studyGuides";

export type StudyTargetKind = "document" | "collection";
export type StudyTargetArtifactKind = "flashcards" | "study_guide";

export interface StudyTargetRef {
  kind: StudyTargetKind;
  id: string;
}

export interface StudyTargetOutputPolicy {
  flashcards: boolean;
  studyGuide: boolean;
}

export interface StudyTargetArtifactStatus {
  kind: StudyTargetArtifactKind;
  state: string;
  artifactId: string | null;
  totalUnits: number;
  completedUnits: number;
  safeErrorCode: string;
  safeErrorMessage: string;
}

export interface StudyTargetBundleStatus {
  targetId: string;
  target: StudyTargetRef;
  title: string;
  revision: number;
  sourceSnapshotId: string;
  sourceChanged: boolean;
  deckId: string | null;
  guideId: string | null;
  guideRevision: number | null;
  latestOperationId: string | null;
  latestBatchState: string | null;
  flashcards: StudyTargetArtifactStatus;
  studyGuide: StudyTargetArtifactStatus;
}

export interface CollectionGuideSection {
  blockKey: string;
  title: string;
  content: StudyGuideSectionContent;
  contentHash: string;
  sourceDocumentId: string | null;
  sourceDocumentTitle: string | null;
  sourceGuideId: string | null;
  sourceTopicId: string | null;
  sourceLocator: SourceLocator | null;
}

export interface CollectionGuideContent {
  schemaVersion: number;
  collectionId: string;
  title: string;
  overview: StudyGuideOverview;
  sections: CollectionGuideSection[];
  coverageNotes: string[];
}

export interface CollectionStudyGuideOverlay {
  id: string;
  guideId: string;
  targetId: string;
  blockKey: string | null;
  overlayKind: string;
  baseContentHash: string | null;
  richDoc: unknown;
  plainText: string;
  state: string;
  revision: number;
  createdAt: number;
  updatedAt: number;
}

export interface CollectionStudyGuide {
  id: string;
  targetId: string;
  collectionId: string;
  title: string;
  revision: number;
  sourceSnapshotId: string;
  content: CollectionGuideContent;
  overlays: CollectionStudyGuideOverlay[];
  createdAt: number;
  updatedAt: number;
  publishedAt: number;
}

export interface GetStudyTargetBundleInput {
  target: StudyTargetRef;
}

export interface BeginStudyTargetBundleInput {
  operationId: string;
  target: StudyTargetRef;
  outputPolicy?: StudyTargetOutputPolicy;
  recreateArtifacts?: StudyTargetArtifactKind[];
}

export async function prepareCollectionStudyGuidePdf(
  collectionId: string,
  includeAnnotations: boolean,
  operationId = createOperationId(),
): Promise<PreparedStudyGuidePdf> {
  return invoke<PreparedStudyGuidePdf>("prepare_collection_study_guide_pdf", {
    input: { collectionId, operationId, includeAnnotations },
  });
}

export async function getCollectionStudyGuide(
  collectionId: string,
): Promise<CollectionStudyGuide | null> {
  return invoke<CollectionStudyGuide | null>("get_collection_study_guide", {
    input: { collectionId },
  });
}

export async function getStudyTargetBundle(
  input: GetStudyTargetBundleInput,
): Promise<StudyTargetBundleStatus | null> {
  return invoke<StudyTargetBundleStatus | null>("get_study_target_bundle", { input });
}

export async function beginStudyTargetBundle(
  input: BeginStudyTargetBundleInput,
): Promise<StudyTargetBundleStatus> {
  return invoke<StudyTargetBundleStatus>("begin_study_target_bundle", { input });
}

export interface ComposeStudyTargetBundleInput {
  operationId: string;
}

export async function composeStudyTargetBundle(
  input: ComposeStudyTargetBundleInput,
): Promise<StudyTargetBundleStatus> {
  return invoke<StudyTargetBundleStatus>("compose_study_target_bundle", { input });
}
