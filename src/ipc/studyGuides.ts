import { invoke } from "./transport";

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

export interface StudyGuideEditorialTopic {
  topicId: string;
  title: string;
}

export interface StudyGuideChapter {
  id: string;
  title: string;
  introduction: string;
  topics: StudyGuideEditorialTopic[];
}

export interface StudyGuideEditorial {
  title: string;
  chapters: StudyGuideChapter[];
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

export type StudyGuideCalloutKind = "key_idea" | "warning" | "tip" | "note";

export interface StudyGuideCallout {
  kind: StudyGuideCalloutKind;
  title: string;
  body: string;
}

export interface StudyGuideTable {
  title: string;
  columns: string[];
  rows: string[][];
  caption: string;
}

export interface StudyGuideWorkedExample {
  title: string;
  prompt: string;
  steps: string[];
  answer: string;
  takeaway: string;
}

export interface StudyGuideRetrievalCheck {
  prompt: string;
  answer: string;
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
  callouts: StudyGuideCallout[];
  tables: StudyGuideTable[];
  workedExamples: StudyGuideWorkedExample[];
  retrievalChecks: StudyGuideRetrievalCheck[];
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
  editorial?: StudyGuideEditorial | null;
  overviewText: string;
  sections: DocumentStudyGuideSection[];
  overlays: StudyGuideOverlay[];
  createdAt: number;
  updatedAt: number;
  publishedAt: number;
}

export type StudyGuideLibrarySort =
  | "recent"
  | "oldest"
  | "title_asc"
  | "title_desc";

/** A document guide belongs to its document's subject; a subject guide (target kind "collection") to its own. */
export type StudyGuideSubjectFilter =
  | { kind: "all" }
  | { kind: "subject"; subjectId: string }
  | { kind: "none" };

export interface ListStudyGuidesInput {
  query: string;
  sort: StudyGuideLibrarySort;
  offset: number;
  limit: number;
  subject: StudyGuideSubjectFilter;
}

export type StudyGuideTargetKind = "document" | "collection";

export interface StudyGuidePageItem {
  guideId: string;
  targetKind: StudyGuideTargetKind;
  targetId: string;
  title: string;
  originalFilename: string | null;
  sourceFormat: string | null;
  sourceUnitKind: string | null;
  sourceUnitCount: number | null;
  sourceCount: number;
  guideRevision: number;
  sectionCount: number;
  noteCount: number;
  /** Sections marked read in their current text; 0 for subject guides. */
  readSectionCount: number;
  /** When a section was last opened; null for subject guides. */
  lastOpenedAt: number | null;
  publishedAt: number;
}

export interface StudyGuideLibraryPage {
  items: StudyGuidePageItem[];
  totalCount: number;
  offset: number;
  limit: number;
}

export interface DeleteDocumentStudyGuideResult {
  documentId: string;
  guideId: string;
  deletedGuideRevision: number;
  removedRevisionCount: number;
  removedOverrideCount: number;
  removedCheckpointCount: number;
  removedExportCount: number;
  removedJobCount: number;
  replayed: boolean;
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

/** The PDF's colour and type theme. Every theme has the same layout. */
export type StudyGuidePdfTheme = "navy" | "sage" | "slate" | "plum";

export interface PreparedStudyGuidePdf {
  preparedExport: PreparedExport;
  guideRevision: number;
  sectionCount: number;
  annotationCount: number;
  mathSourceFallback: boolean;
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

export async function readDocumentSourceAsset(
  documentId: string,
  assetId: string,
): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("read_document_source_asset", {
    input: { documentId, assetId },
  });
}

export async function getDocumentStudyGuide(
  documentId: string,
): Promise<DocumentStudyGuide | null> {
  return invoke<DocumentStudyGuide | null>("get_document_study_guide", {
    input: { documentId },
  });
}

export async function listStudyGuidePage(
  input: Partial<ListStudyGuidesInput> = {},
): Promise<StudyGuideLibraryPage> {
  return invoke<StudyGuideLibraryPage>("list_study_guide_page", {
    input: {
      query: input.query ?? "",
      sort: input.sort ?? "recent",
      offset: input.offset ?? 0,
      limit: input.limit ?? 40,
      subject: input.subject ?? { kind: "all" },
    },
  });
}

export async function deleteDocumentStudyGuide(
  documentId: string,
  guideId: string,
  expectedRevision: number,
  operationId = createOperationId(),
): Promise<DeleteDocumentStudyGuideResult> {
  return invoke<DeleteDocumentStudyGuideResult>("delete_document_study_guide", {
    input: { documentId, guideId, expectedRevision, operationId },
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

/** One section of a document's study guide set as a designed page in a PDF theme. */
export interface StudyGuideSectionPage {
  documentId: string;
  topicId: string;
  guideRevision: number;
  theme: StudyGuidePdfTheme;
  /** The page as a standalone SVG image. */
  svg: string;
}

export async function getDocumentStudyGuideSectionPage(
  documentId: string,
  topicId: string,
  theme: StudyGuidePdfTheme,
): Promise<StudyGuideSectionPage> {
  return invoke<StudyGuideSectionPage>("get_document_study_guide_section_page", {
    input: { documentId, topicId, theme },
  });
}

export async function prepareDocumentStudyGuidePdf(
  documentId: string,
  includeAnnotations: boolean,
  theme: StudyGuidePdfTheme = "navy",
  operationId = createOperationId(),
): Promise<PreparedStudyGuidePdf> {
  return invoke<PreparedStudyGuidePdf>("prepare_document_study_guide_pdf", {
    input: { documentId, operationId, includeAnnotations, theme },
  });
}
