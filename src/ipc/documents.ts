import { invoke } from "./transport";

import { generationInstructionsInput, type GenerationInstructionsInput } from "./generationInstructions";
import type { QuestionFamily, QuestionGenerationPolicy } from "./questionGeneration";
import type { SourceLocator } from "./studyGuides";

export type DocumentLifecycleState = "processing" | "ready" | "failed";
export type DocumentScaleMode = "standard" | "large";
export type DocumentExtractionMode = "native" | "page_text_fallback" | "ocr_fallback";

export interface DocumentGenerationProgress {
  startedAt: number;
  phase: string;
  phaseStartedAt: number;
  currentWindow: number | null;
  currentWindowEnd?: number | null;
  completedWindows: number;
  totalWindows: number;
  currentPageStart: number | null;
  currentPageEnd: number | null;
  currentPartition: number | null;
  providerKind: string | null;
  modelId: string | null;
  requestedReasoningEffort: string | null;
  reasoningEffort: string;
  currentRequestStartedAt: number | null;
  currentRequestStage: string | null;
  currentRequestBytes: number | null;
  currentRequestTextBytes: number | null;
  currentRequestImageCount: number | null;
  currentRequestEvidenceBytes: number | null;
  currentRequestMaxOutputTokens: number | null;
  providerRequestCount: number;
  retryCount: number;
  transientRetryCount?: number;
  compatibilityRetryCount?: number;
  contractRepairRetryCount?: number;
  outputCompactionRetryCount?: number;
  promptTokens: number;
  completionTokens: number;
  reasoningTokens: number;
  totalTokens: number;
  cachedPromptTokens: number;
  cacheWriteTokens: number;
  usageReportedRequests: number;
  usageUnreportedRequests: number;
  providerActivity: "preparing" | "connecting" | "responsive" | "streaming";
  lastProviderActivityAt: number | null;
  lastModelActivityAt: number | null;
}

/** Immutable native report for one successful generation pass, not live inventory. */
export interface GenerationReport {
  schemaVersion: 1;
  generatedAt: number;
  requestedPolicy: QuestionGenerationPolicy & GenerationInstructionsInput;
  candidateNoteCount: number;
  selectedNoteCount: number;
  curationRemovedCount: number;
  savedNoteCount: number;
  savedCardCount: number;
  skippedDuplicateCount: number;
  availableKnowledgeUnits: number;
  linkedKnowledgeUnits: number;
  evidenceBindingCount: number;
  itemCount: number;
  types: { family: QuestionFamily; noteCount: number; cardCount: number; itemCount: number }[];
}

export interface DocumentSummary {
  id: string;
  title: string;
  originalFilename: string;
  byteSize: number;
  sha256: string;
  lifecycleState: DocumentLifecycleState;
  sourceFormat: string;
  sourceUnitKind: string;
  sourceUnitCount: number;
  pageCount: number;
  extractedPageCount: number;
  renderedPageCount: number;
  extractionMode: DocumentExtractionMode;
  ocrPageCount: number;
  ocrCompletedPageCount: number;
  ocrMeanConfidence: number | null;
  ocrEngine: string;
  scaleMode: DocumentScaleMode;
  estimatedSourceTokens: number;
  outlineWindowCount: number;
  outlineCompletedWindowCount: number;
  generationWindowCount: number;
  generationCompletedWindowCount: number;
  generatedNoteCount: number;
  generatedCardCount: number;
  generatedDeckId: string | null;
  /** Unresolved analysis proposals; usable generated cards may still exist. */
  analysisPendingItems?: number;
  /** Absent for older jobs without captured summary inputs. */
  generationSummary?: GenerationReport | null;
  safeErrorCode: string;
  safeErrorMessage: string;
  createdAt: number;
  updatedAt: number;
  completedAt: number | null;
  revision: number;
  processingDurationMs: number | null;
  apiTokenCount: number | null;
  apiTokenUsageComplete: boolean;
}

export interface SourceCitation {
  citationId: string;
  schemaVersion: number;
  sourceDocumentId: string;
  sourceIdentity: string;
  sourceFingerprintSha256: string;
  sourceTitle: string;
  originalFilename: string;
  sourceFormat: string;
  sourceMediaType: string;
  sourceUnitKind: string;
  sourceUnitCount: number;
  sourceRemoved: boolean;
  sourceLocator: SourceLocator | null;
}

export interface DocumentGenerationResult {
  document: DocumentSummary;
  deckId: string;
  createdNoteCount: number;
  createdCardCount: number;
  skippedDuplicateCount: number;
}

export interface AdditionalDocumentGenerationResult {
  documentId: string;
  deckId: string;
  createdNoteCount: number;
  createdCardCount: number;
  skippedDuplicateCount: number;
}

export interface DeleteDocumentResult {
  documentId: string;
  preservedDeckCitationCount: number;
}

interface ImportedDocument {
  documentId: string;
}

export async function listDocuments(): Promise<DocumentSummary[]> {
  return invoke<DocumentSummary[]>("list_documents");
}

export async function getDocumentGenerationProgress(
  documentId: string,
): Promise<DocumentGenerationProgress | null> {
  return invoke<DocumentGenerationProgress | null>("get_document_generation_progress", {
    documentId,
  });
}

export async function listDeckSourceCitations(deckId: string): Promise<SourceCitation[]> {
  return invoke<SourceCitation[]>("list_deck_source_citations", { deckId });
}

export async function getNoteSourceCitation(noteId: string): Promise<SourceCitation | null> {
  return invoke<SourceCitation | null>("get_note_source_citation", { noteId });
}

export async function importDocument(stagedFileId: string): Promise<ImportedDocument> {
  return invoke<ImportedDocument>("import_document", {
    input: { stagedFileId },
  });
}

export async function importDocumentAndGenerateFlashcards(
  stagedFileId: string,
  generationInstructions?: string,
): Promise<DocumentGenerationResult> {
  const guidance = generationInstructionsInput(generationInstructions);
  // Establish a durable document identity before the long-running provider
  // request begins. That lets Documents expose an exact Stop action while the
  // generation command remains in flight.
  const imported = await importDocument(stagedFileId);
  return retryDocumentFlashcardGeneration(imported.documentId, guidance.generationInstructions);
}

export async function startDocumentFlashcardGeneration(
  documentId: string,
  generationInstructions?: string,
): Promise<void> {
  await invoke("start_document_flashcard_generation", {
    input: { documentId, ...generationInstructionsInput(generationInstructions) },
  });
}

export async function retryDocumentFlashcardGeneration(
  documentId: string,
  generationInstructions?: string,
): Promise<DocumentGenerationResult> {
  return invoke<DocumentGenerationResult>("run_document_flashcard_generation", {
    input: { documentId, ...generationInstructionsInput(generationInstructions) },
  });
}

export async function generateMoreDocumentFlashcards(
  documentId: string,
): Promise<AdditionalDocumentGenerationResult> {
  return invoke<AdditionalDocumentGenerationResult>("generate_more_pdf_flashcards", {
    input: { documentId },
  });
}

export async function cancelDocumentFlashcardGeneration(documentId: string): Promise<void> {
  await invoke("cancel_document_flashcard_generation", { documentId });
}

export async function openDocumentSource(documentId: string): Promise<void> {
  await invoke("open_document_source", { documentId });
}

export async function deleteDocument(
  documentId: string,
  revision: number,
): Promise<DeleteDocumentResult> {
  return invoke<DeleteDocumentResult>("delete_document", {
    input: { documentId, revision },
  });
}
