import { invoke } from "@tauri-apps/api/core";

export type DocumentLifecycleState = "processing" | "ready" | "failed" | "archived";
export type DocumentScaleMode = "standard" | "large";
export type DocumentExtractionMode = "native" | "page_text_fallback" | "ocr_fallback";

export interface DocumentSummary {
  id: string;
  title: string;
  originalFilename: string;
  byteSize: number;
  sha256: string;
  lifecycleState: DocumentLifecycleState;
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
  safeErrorCode: string;
  safeErrorMessage: string;
  createdAt: number;
  updatedAt: number;
  completedAt: number | null;
  archivedAt: number | null;
  revision: number;
  processingDurationMs: number | null;
  apiTokenCount: number | null;
  apiTokenUsageComplete: boolean;
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

export interface DeletePdfDocumentResult {
  documentId: string;
  preservedDeckCitationCount: number;
}

interface ImportedPdfDocument {
  documentId: string;
}

export async function listDocuments(includeArchived = false): Promise<DocumentSummary[]> {
  return invoke<DocumentSummary[]>("list_documents", { includeArchived });
}

export async function importPdfAndGenerateFlashcards(
  stagedFileId: string,
): Promise<DocumentGenerationResult> {
  // Establish a durable document identity before the long-running provider
  // request begins. That lets Documents expose an exact Stop action while the
  // generation command remains in flight.
  const imported = await invoke<ImportedPdfDocument>("import_pdf_document", {
    input: { stagedFileId },
  });
  return retryPdfFlashcardGeneration(imported.documentId);
}

export async function retryPdfFlashcardGeneration(
  documentId: string,
): Promise<DocumentGenerationResult> {
  return invoke<DocumentGenerationResult>("run_pdf_flashcard_generation", {
    input: { documentId },
  });
}

export async function generateMorePdfFlashcards(
  documentId: string,
): Promise<AdditionalDocumentGenerationResult> {
  return invoke<AdditionalDocumentGenerationResult>("generate_more_pdf_flashcards", {
    input: { documentId },
  });
}

export async function cancelPdfFlashcardGeneration(documentId: string): Promise<void> {
  await invoke("cancel_pdf_flashcard_generation", { documentId });
}

export async function openDocumentSource(documentId: string): Promise<void> {
  await invoke("open_document_source", { documentId });
}

export async function deletePdfDocument(
  documentId: string,
  revision: number,
): Promise<DeletePdfDocumentResult> {
  return invoke<DeletePdfDocumentResult>("delete_pdf_document", {
    input: { documentId, revision },
  });
}

export async function setDocumentArchived(
  documentId: string,
  revision: number,
  archived: boolean,
): Promise<DocumentSummary> {
  return invoke<DocumentSummary>("set_document_archived", {
    input: { documentId, revision, archived },
  });
}