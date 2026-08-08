import { invoke } from "@tauri-apps/api/core";

export type DocumentLifecycleState = "processing" | "ready" | "failed" | "archived";

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
}

export interface DocumentGenerationResult {
  document: DocumentSummary;
  deckId: string;
  createdNoteCount: number;
  createdCardCount: number;
  skippedDuplicateCount: number;
}

export async function listDocuments(includeArchived = false): Promise<DocumentSummary[]> {
  return invoke<DocumentSummary[]>("list_documents", { includeArchived });
}

export async function importPdfAndGenerateFlashcards(
  sourcePath: string,
): Promise<DocumentGenerationResult> {
  return invoke<DocumentGenerationResult>("import_pdf_and_generate_flashcards", {
    input: { sourcePath },
  });
}

export async function retryPdfFlashcardGeneration(
  documentId: string,
): Promise<DocumentGenerationResult> {
  return invoke<DocumentGenerationResult>("retry_pdf_flashcard_generation", {
    input: { documentId },
  });
}

export async function getDocumentSourcePath(documentId: string): Promise<string> {
  return invoke<string>("get_document_source_path", { documentId });
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
