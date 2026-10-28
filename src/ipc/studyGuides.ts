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

/** One page of the whole guide as the reader's canvas, in points, with the key its marks are saved under. */
export interface StudyGuideCanvasPage {
  /** `front:1`, `chapter:<id>:2`, `body:3`, `cram:1`, `test:1`: the part of the guide and the page within it. */
  key: string;
  width: number;
  height: number;
  /** The page as a standalone SVG image. */
  svg: string;
}

export type StudyGuideCanvasEntryKind = "chapter" | "section" | "cram" | "selfTest";

/** A chapter, section, cram sheet or self-test, where it starts on the canvas. */
export interface StudyGuideCanvasEntry {
  kind: StudyGuideCanvasEntryKind;
  /** The section's topic id, the chapter's id, or the part's name. */
  key: string;
  title: string;
  /** The number the page prints, such as "2.1"; empty for the cram sheet and self-test. */
  number: string;
  /** 0-based. */
  pageIndex: number;
  /** Points from the top of that page. */
  y: number;
}

/** The room a section heading keeps for its Practice button, in points. */
export interface StudyGuideCanvasButton {
  topicId: string;
  pageIndex: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Where a key point's source number landed, in points. */
export interface StudyGuideCanvasCitation {
  topicId: string;
  number: number;
  pageIndex: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A passage a section cites, numbered as its key points name it. */
export interface StudyGuideCanvasSource {
  topicId: string;
  number: number;
  documentId: string;
  /** "p. 12" or "pp. 12–14", as the section's Sources line prints it. */
  label: string;
  /** The first cited page, when it can be opened: a page of a PDF. */
  pageNumber: number | null;
  /** The cited passages on these pages, as the document says them. */
  excerpts: string[];
}

/** A document's whole study guide laid out as the PDF export sets it. */
export interface StudyGuideCanvas {
  documentId: string;
  guideRevision: number;
  theme: StudyGuidePdfTheme;
  pages: StudyGuideCanvasPage[];
  /** In page order. */
  contents: StudyGuideCanvasEntry[];
  practice: StudyGuideCanvasButton[];
  /** Each key point's source numbers. */
  citations: StudyGuideCanvasCitation[];
  sources: StudyGuideCanvasSource[];
}

export async function getDocumentStudyGuideCanvas(
  documentId: string,
  theme: StudyGuidePdfTheme,
): Promise<StudyGuideCanvas> {
  return invoke<StudyGuideCanvas>("get_document_study_guide_canvas", {
    input: { documentId, theme },
  });
}

/** Opens the guide as a PDF in the system's PDF reader, without asking where to save it. */
export async function openDocumentStudyGuidePdf(documentId: string, theme: StudyGuidePdfTheme): Promise<void> {
  await invoke<void>("open_document_study_guide_pdf", { input: { documentId, theme } });
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
