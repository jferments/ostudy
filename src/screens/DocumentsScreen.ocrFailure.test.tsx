import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as documentIpc from "../ipc/documents";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { DocumentsScreen } from "./DocumentsScreen";

vi.mock("../platform/files", () => ({ selectExternalImport: vi.fn() }));
vi.mock("../ipc/documents", () => ({
  listDocuments: vi.fn(),
  importPdfAndGenerateFlashcards: vi.fn(),
  retryPdfFlashcardGeneration: vi.fn(),
  generateMorePdfFlashcards: vi.fn(),
  cancelPdfFlashcardGeneration: vi.fn(),
  openDocumentSource: vi.fn(),
  deletePdfDocument: vi.fn(),
  setDocumentArchived: vi.fn(),
}));

const documentId = "c".repeat(32);

function failedOcrDocument(): documentIpc.DocumentSummary {
  return {
    id: documentId,
    title: "Malformed text-layer regression",
    originalFilename: "malformed-text.pdf",
    byteSize: 4096,
    sha256: "0".repeat(64),
    lifecycleState: "failed",
    sourceFormat: "pdf",
    sourceUnitKind: "page",
    sourceUnitCount: 138,
    pageCount: 138,
    extractedPageCount: 138,
    renderedPageCount: 138,
    extractionMode: "ocr_fallback",
    ocrPageCount: 138,
    ocrCompletedPageCount: 0,
    ocrMeanConfidence: null,
    ocrEngine: "tesseract-cli",
    scaleMode: "standard",
    estimatedSourceTokens: 0,
    outlineWindowCount: 0,
    outlineCompletedWindowCount: 0,
    generationWindowCount: 0,
    generationCompletedWindowCount: 0,
    generatedNoteCount: 0,
    generatedCardCount: 0,
    generatedDeckId: null,
    safeErrorCode: "document_ocr_text_unusable",
    safeErrorMessage:
      "The PDF text layer on page 1 is unreliable, and OCR could not recover trustworthy text from the rendered page.",
    createdAt: 1_700_000_000_000,
    updatedAt: 1_700_000_000_000,
    completedAt: null,
    archivedAt: null,
    revision: 3,
    processingDurationMs: 12_345,
    apiTokenCount: 45_678,
    apiTokenUsageComplete: true,
  };
}

describe("DocumentsScreen OCR failure messaging", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([failedOcrDocument()]);
    vi.mocked(documentIpc.openDocumentSource).mockResolvedValue(undefined);
    vi.mocked(documentIpc.cancelPdfFlashcardGeneration).mockResolvedValue(undefined);
  });

  it("never claims that failed OCR candidates were recovered", async () => {
    render(
      <OverlayProvider>
        <DocumentsScreen refreshDecks={vi.fn().mockResolvedValue([])} showToast={vi.fn()} />
      </OverlayProvider>,
    );

    expect(await screen.findByText(/page 1 is unreliable/u)).toBeTruthy();
    expect(screen.queryByText(/so ostudy recovered those pages with OCR/u)).toBeNull();
    expect(screen.queryByText(/Mean OCR confidence/u)).toBeNull();
  });
});
