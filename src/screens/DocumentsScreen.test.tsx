import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as documentIpc from "../ipc/documents";
import * as platformFiles from "../platform/files";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { DocumentsScreen } from "./DocumentsScreen";

vi.mock("../platform/files", () => ({ selectExternalImport: vi.fn() }));
vi.mock("../ipc/documents", () => ({
  listDocuments: vi.fn(),
  getDocumentGenerationProgress: vi.fn(),
  importPdfAndGenerateFlashcards: vi.fn(),
  retryPdfFlashcardGeneration: vi.fn(),
  generateMorePdfFlashcards: vi.fn(),
  cancelPdfFlashcardGeneration: vi.fn(),
  openDocumentSource: vi.fn(),
  deletePdfDocument: vi.fn(),
  setDocumentArchived: vi.fn(),
}));
vi.mock("../ipc/common", async (importOriginal) => {
  const original = await importOriginal<typeof import("../ipc/common")>();
  return {
    ...original,
    getErrorCode: (error: unknown) =>
      error && typeof error === "object" && "code" in error ? String(error.code) : "unknown",
    getErrorMessage: (error: unknown) =>
      error instanceof Error ? error.message : "Unexpected document error",
  };
});

const documentId = "a".repeat(32);

function makeDocument(
  overrides: Partial<documentIpc.DocumentSummary> = {},
): documentIpc.DocumentSummary {
  return {
    id: documentId,
    title: "Cardiac physiology",
    originalFilename: "cardiac-physiology.pdf",
    byteSize: 2048,
    sha256: "0".repeat(64),
    lifecycleState: "ready",
    sourceFormat: "pdf",
    sourceUnitKind: "page",
    sourceUnitCount: 10,
    pageCount: 10,
    extractedPageCount: 10,
    renderedPageCount: 10,
    extractionMode: "native",
    ocrPageCount: 0,
    ocrCompletedPageCount: 0,
    ocrMeanConfidence: null,
    ocrEngine: "",
    scaleMode: "standard",
    estimatedSourceTokens: 0,
    outlineWindowCount: 0,
    outlineCompletedWindowCount: 0,
    generationWindowCount: 0,
    generationCompletedWindowCount: 0,
    generatedNoteCount: 6,
    generatedCardCount: 7,
    generatedDeckId: "b".repeat(32),
    safeErrorCode: "",
    safeErrorMessage: "",
    createdAt: 1_700_000_000_000,
    updatedAt: 1_700_000_000_000,
    completedAt: 1_700_000_000_000,
    archivedAt: null,
    revision: 2,
    processingDurationMs: 12_345,
    apiTokenCount: 45_678,
    apiTokenUsageComplete: true,
    ...overrides,
  };
}

function renderDocuments(showToast = vi.fn(), refreshDecks = vi.fn().mockResolvedValue([])) {
  render(
    <OverlayProvider>
      <DocumentsScreen refreshDecks={refreshDecks} showToast={showToast} />
    </OverlayProvider>,
  );
  return { showToast, refreshDecks };
}

describe("DocumentsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue(null);
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([]);
    vi.mocked(documentIpc.getDocumentGenerationProgress).mockResolvedValue(null);
    vi.mocked(documentIpc.openDocumentSource).mockResolvedValue(undefined);
    vi.mocked(documentIpc.cancelPdfFlashcardGeneration).mockResolvedValue(undefined);
  });

  it("shows a simple upload-first empty state", async () => {
    renderDocuments();
    expect(await screen.findByRole("heading", { name: "No documents yet" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Upload document" })).toBeTruthy();
    expect(screen.queryByText(/review generated/u)).toBeNull();
    expect(screen.queryByText(/JSON/u)).toBeNull();
  });

  it("uploads one document, announces live progress, and creates a normal deck", async () => {
    const completed = makeDocument();
    vi.mocked(platformFiles.selectExternalImport).mockResolvedValue({
      id: "1".repeat(32),
      purpose: "document",
      detectedKind: "pdf",
      displayName: "cardiac-physiology.pdf",
      mediaType: "application/pdf",
      byteSize: 128,
      sha256: "0".repeat(64),
      expiresAt: Date.now() + 60_000,
    });
    vi.mocked(documentIpc.importPdfAndGenerateFlashcards).mockResolvedValue({
      document: completed,
      deckId: completed.generatedDeckId ?? "",
      createdNoteCount: 6,
      createdCardCount: 7,
      skippedDuplicateCount: 1,
    });
    vi.mocked(documentIpc.listDocuments)
      .mockResolvedValueOnce([])
      .mockResolvedValue([completed]);
    const { showToast, refreshDecks } = renderDocuments();
    await screen.findByRole("heading", { name: "No documents yet" });

    fireEvent.click(screen.getByRole("button", { name: "Upload document" }));

    await waitFor(() => {
      expect(platformFiles.selectExternalImport).toHaveBeenCalledWith("document");
      expect(showToast).toHaveBeenCalledWith(
        "info",
        "Document processing started. Live stage, source-unit, request, and token progress will appear here.",
      );
      expect(documentIpc.importPdfAndGenerateFlashcards).toHaveBeenCalledWith("1".repeat(32));
      expect(refreshDecks).toHaveBeenCalled();
      expect(showToast).toHaveBeenCalledWith("success", "Created 7 flashcards in a new deck.");
    });
    expect(await screen.findByRole("heading", { name: "Cardiac physiology" })).toBeTruthy();
    expect(screen.getByText("PDF · 2.0 KB")).toBeTruthy();
    expect(screen.getByText("12 sec")).toBeTruthy();
    expect(screen.getByText("45,678")).toBeTruthy();
  });

  it("shows detected non-PDF formats with truthful canonical section counts", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        originalFilename: "cardiac-physiology.docx",
        sourceFormat: "docx",
        sourceUnitKind: "section",
        sourceUnitCount: 8,
        pageCount: 8,
        extractedPageCount: 8,
        renderedPageCount: 0,
      }),
    ]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    expect(screen.getByText("Word · 2.0 KB")).toBeTruthy();
    expect(screen.getByText("Sections")).toBeTruthy();
    expect(screen.getByText("8")).toBeTruthy();
    expect(screen.queryByText(/rendered/u)).toBeNull();
  });

  it("keeps completed OCR recovery out of permanent document-list copy", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        extractionMode: "ocr_fallback",
        ocrPageCount: 18,
        ocrCompletedPageCount: 18,
        ocrMeanConfidence: 0.91,
        ocrEngine: "Tesseract CLI via oxidize-pdf OcrProvider",
      }),
    ]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    expect(screen.queryByText(/Native PDF text was unreliable/u)).toBeNull();
    expect(screen.queryByText(/Mean OCR confidence/u)).toBeNull();
  });

  it("shows live OCR recovery progress after normal rendering completes", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        lifecycleState: "processing",
        generatedDeckId: null,
        generatedCardCount: 0,
        completedAt: null,
        extractionMode: "ocr_fallback",
        ocrPageCount: 12,
        ocrCompletedPageCount: 5,
        ocrMeanConfidence: null,
        ocrEngine: "tesseract-cli",
      }),
    ]);
    renderDocuments();

    expect((await screen.findAllByText("Recovering OCR 5/12")).length).toBeGreaterThan(0);
  });

  it("keeps a failed document in the library and offers a direct retry", async () => {
    const failed = makeDocument({
      lifecycleState: "failed",
      generatedDeckId: null,
      generatedNoteCount: 0,
      generatedCardCount: 0,
      safeErrorCode: "ai_initial_response_timeout",
      safeErrorMessage: "The provider never acknowledged the streamed request.",
    });
    const completed = makeDocument();
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([failed]);
    vi.mocked(documentIpc.retryPdfFlashcardGeneration).mockResolvedValue({
      document: completed,
      deckId: completed.generatedDeckId ?? "",
      createdNoteCount: 6,
      createdCardCount: 7,
      skippedDuplicateCount: 0,
    });
    const { showToast } = renderDocuments();

    expect(await screen.findByText(failed.safeErrorMessage)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Retry generation" }));

    await waitFor(() => {
      expect(documentIpc.retryPdfFlashcardGeneration).toHaveBeenCalledWith(documentId);
      expect(showToast).toHaveBeenCalledWith("success", "Created 7 flashcards in a new deck.");
    });
  });

  it("shows a lightweight local-processing phase before source-unit metadata is committed", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        lifecycleState: "processing",
        sourceUnitCount: 0,
        pageCount: 0,
        extractedPageCount: 0,
        renderedPageCount: 0,
        generatedDeckId: null,
        generatedCardCount: 0,
        completedAt: null,
      }),
    ]);
    renderDocuments();

    expect((await screen.findAllByText("Processing")).length).toBeGreaterThan(0);
    expect(screen.getAllByText("Extracting document content").length).toBeGreaterThan(0);
  });

  it("shows live rendered-page progress for PDFs once the total page count is known", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        lifecycleState: "processing",
        sourceUnitCount: 11,
        pageCount: 11,
        extractedPageCount: 11,
        renderedPageCount: 4,
        generatedDeckId: null,
        generatedCardCount: 0,
        completedAt: null,
      }),
    ]);
    renderDocuments();

    expect(await screen.findByText("4/11 rendered")).toBeTruthy();
    expect(screen.getAllByText("Rendering pages 4/11").length).toBeGreaterThan(0);
  });

  it("offers an exact stop action while a document generation task is processing", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({
        lifecycleState: "processing",
        generatedDeckId: null,
        completedAt: null,
      }),
    ]);
    const { showToast } = renderDocuments();

    expect(await screen.findByRole("button", { name: "Stop generation" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Stop generation" }));

    await waitFor(() => {
      expect(documentIpc.cancelPdfFlashcardGeneration).toHaveBeenCalledWith(documentId);
      expect(showToast).toHaveBeenCalledWith("info", "Stopping the current AI request…");
    });
    expect(screen.getByRole("button", { name: "Stopping…" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Delete document" })).toBeNull();
  });

  it("generates an additional coverage pass into the existing deck", async () => {
    const existing = makeDocument();
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([existing]);
    vi.mocked(documentIpc.generateMorePdfFlashcards).mockResolvedValue({
      documentId,
      deckId: existing.generatedDeckId ?? "",
      createdNoteCount: 4,
      createdCardCount: 5,
      skippedDuplicateCount: 2,
    });
    const { showToast, refreshDecks } = renderDocuments();

    expect(await screen.findByRole("button", { name: "Generate more cards" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Generate more cards" }));

    await waitFor(() => {
      expect(documentIpc.generateMorePdfFlashcards).toHaveBeenCalledWith(documentId);
      expect(refreshDecks).toHaveBeenCalled();
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Added 5 new flashcards to the existing deck.",
      );
    });
  });

  it("offers regeneration when the previously generated deck was deleted", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({ generatedDeckId: null, lifecycleState: "ready" }),
    ]);
    renderDocuments();

    expect(await screen.findByRole("button", { name: "Generate study guide" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Retry generation" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Generate more cards" })).toBeNull();
    expect(screen.getByText(/previous generated deck was deleted/u)).toBeTruthy();
    expect(screen.getByText(/retained document/u)).toBeTruthy();
  });

  it("opens the managed source through the native document command", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "Open source" }));

    await waitFor(() => {
      expect(documentIpc.openDocumentSource).toHaveBeenCalledWith(documentId);
    });
  });

  it("permanently deletes a document only after confirmation and preserves deck citations", async () => {
    vi.mocked(documentIpc.listDocuments)
      .mockResolvedValueOnce([makeDocument()])
      .mockResolvedValue([]);
    vi.mocked(documentIpc.deletePdfDocument).mockResolvedValue({
      documentId,
      preservedDeckCitationCount: 1,
    });
    const { showToast, refreshDecks } = renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "Delete document" }));
    const dialog = await screen.findByRole("alertdialog", {
      name: "Delete this document permanently?",
    });
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete document" }));

    await waitFor(() => {
      expect(documentIpc.deletePdfDocument).toHaveBeenCalledWith(documentId, 2);
      expect(refreshDecks).toHaveBeenCalled();
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "Document deleted. Preserved text source citations in 1 generated deck.",
      );
    });
  });

  it("does not claim citation preservation after the only generated deck was deleted", async () => {
    const withoutDeck = makeDocument({
      generatedDeckId: null,
      lifecycleState: "ready",
      generatedCardCount: 7,
    });
    vi.mocked(documentIpc.listDocuments)
      .mockResolvedValueOnce([withoutDeck])
      .mockResolvedValue([]);
    vi.mocked(documentIpc.deletePdfDocument).mockResolvedValue({
      documentId,
      preservedDeckCitationCount: 0,
    });
    const { showToast } = renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "Delete document" }));
    const dialog = await screen.findByRole("alertdialog", {
      name: "Delete this document permanently?",
    });
    expect(within(dialog).getByText(/No active generated deck remains/u)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete document" }));

    await waitFor(() => {
      expect(showToast).toHaveBeenCalledWith("success", "Document deleted from Documents.");
    });
  });
});
