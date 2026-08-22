import { invoke } from "@tauri-apps/api/core";

import { createOperationId, type RichDocument } from "./common";
import type { PreparedExport } from "./externalFiles";

export interface SourceLocator {
  unitKind: string;
  unitNumbers: number[];
  sectionLabel?: string | null;
  assetIds?: string[];
}

export interface LearningTopic {
  id: string;
  parentTopicId: string | null;
  sortIndex: number;
  title: string;
  summary: string;
  learningObjectives: string[];
  keyConcepts: string[];
  sourceLocator: SourceLocator;
  evidenceChunkIndices: number[];
  evidenceHashes: string[];
  evidenceHash: string;
  anchorHash: string;
}

export interface DocumentLearningPlan {
  id: string;
  documentId: string;
  revision: number;
  compilerVersion: number;
  outlineFingerprint: string;
  planFingerprint: string;
  summary: string;
  learningObjectives: string[];
  keyConcepts: string[];
  topics: LearningTopic[];
}

export interface StudyGuideOverview {
  summary: string;
  learningObjectives: string[];
  keyConcepts: string[];
}

export interface StudyGuideExplanation {
  heading: string;
  body: string;
}

export interface StudyGuideTerm {
  term: string;
  explanation: string;
}

export interface StudyGuideComparison {
  subjectA: string;
  subjectB: string;
  relationship: string;
}

export interface StudyGuideVisualExplanation {
  title: string;
  explanation: string;
}

export interface StudyGuideSectionContent {
  overview: string;
  keyPoints: string[];
  explanations: StudyGuideExplanation[];
  importantTerms: StudyGuideTerm[];
  processSteps: string[];
  comparisons: StudyGuideComparison[];
  commonMisconceptions: string[];
  visualExplanations: StudyGuideVisualExplanation[];
}

export interface DocumentStudyGuideSection {
  topicId: string;
  parentTopicId: string | null;
  sortIndex: number;
  title: string;
  topicSummary: string;
  content: StudyGuideSectionContent;
  contentText: string;
  contentHash: string;
  sourceLocator: SourceLocator;
}

export type StudyGuideOverlayKind =
  | "guide_note"
  | "topic_note"
  | "title_override"
  | "body_override"
  | "highlight";

export type StudyGuideOverlayState = "active" | "conflicted" | "discarded";

export interface StudyGuideOverlay {
  id: string;
  documentId: string;
  guideId: string | null;
  topicId: string | null;
  overlayKind: StudyGuideOverlayKind;
  baseContentHash: string | null;
  richDoc: RichDocument;
  plainText: string;
  state: StudyGuideOverlayState;
  revision: number;
  createdAt: number;
  updatedAt: number;
}

export interface DocumentStudyGuide {
  id: string;
  documentId: string;
  planId: string;
  jobId: string | null;
  revision: number;
  schemaVersion: number;
  promptVersion: number;
  assemblyVersion: number;
  overview: StudyGuideOverview;
  overviewText: string;
  sections: DocumentStudyGuideSection[];
  overlays: StudyGuideOverlay[];
  createdAt: number;
  updatedAt: number;
  publishedAt: number;
}

export type DocumentArtifactKind = "study_guide" | "flashcards";
export type DocumentArtifactJobState =
  | "queued"
  | "preparing"
  | "generating"
  | "committing"
  | "succeeded"
  | "failed"
  | "interrupted"
  | "canceled";

export interface DocumentArtifactJob {
  id: string;
  documentId: string;
  planId: string | null;
  artifactKind: DocumentArtifactKind;
  state: DocumentArtifactJobState;
  totalUnits: number;
  completedUnits: number;
  safeErrorCode: string;
  safeErrorMessage: string;
  apiTokenCount: number | null;
  apiTokenUsageComplete: boolean;
  createdAt: number;
  startedAt: number | null;
  updatedAt: number;
  completedAt: number | null;
}

export interface TopicCardProjection {
  topicId: string;
  noteCount: number;
  cardCount: number;
  studyableCardCount: number;
}

export interface SaveStudyGuideOverlayInput {
  overlayId?: string;
  documentId: string;
  guideId?: string;
  topicId?: string;
  overlayKind: StudyGuideOverlayKind;
  baseContentHash?: string;
  richDoc: RichDocument;
  plainText: string;
  expectedRevision?: number;
}

export interface PreparedStudyGuidePdf {
  preparedExport: PreparedExport;
  guideRevision: number;
  sectionCount: number;
  annotationCount: number;
}

export async function generateDocumentStudyGuide(
  documentId: string,
): Promise<DocumentStudyGuide> {
  return invoke<DocumentStudyGuide>("generate_document_study_guide", {
    input: { documentId },
  });
}

export async function cancelDocumentStudyGuideGeneration(documentId: string): Promise<void> {
  await invoke("cancel_document_study_guide_generation", { documentId });
}

export async function getDocumentStudyGuide(
  documentId: string,
): Promise<DocumentStudyGuide | null> {
  return invoke<DocumentStudyGuide | null>("get_document_study_guide", {
    input: { documentId },
  });
}

export async function getDocumentLearningPlan(
  documentId: string,
): Promise<DocumentLearningPlan | null> {
  return invoke<DocumentLearningPlan | null>("get_document_learning_plan", {
    input: { documentId },
  });
}

export async function getDocumentArtifactJob(jobId: string): Promise<DocumentArtifactJob> {
  return invoke<DocumentArtifactJob>("get_document_artifact_job", { jobId });
}

export async function saveDocumentStudyGuideOverlay(
  input: SaveStudyGuideOverlayInput,
): Promise<StudyGuideOverlay> {
  return invoke<StudyGuideOverlay>("save_document_study_guide_overlay", { input });
}

export async function discardDocumentStudyGuideOverlay(
  overlayId: string,
  revision: number,
): Promise<void> {
  await invoke("discard_document_study_guide_overlay", {
    input: { overlayId, revision },
  });
}

export async function getDocumentTopicCardProjection(
  documentId: string,
  topicId: string,
): Promise<TopicCardProjection> {
  return invoke<TopicCardProjection>("get_document_topic_card_projection", {
    input: { documentId, topicId },
  });
}

export async function getDocumentTopicCardIds(
  documentId: string,
  topicIds: string[],
): Promise<string[]> {
  return invoke<string[]>("get_document_topic_card_ids", {
    input: { documentId, topicIds },
  });
}

export async function prepareDocumentStudyGuidePdf(
  documentId: string,
  includeAnnotations: boolean,
  operationId = createOperationId(),
): Promise<PreparedStudyGuidePdf> {
  return invoke<PreparedStudyGuidePdf>("prepare_document_study_guide_pdf", {
    input: { documentId, operationId, includeAnnotations },
  });
}
