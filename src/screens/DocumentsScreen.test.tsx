import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as documentIpc from "../ipc/documents";
import * as platformFiles from "../platform/files";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { DocumentsScreen } from "./DocumentsScreen";

vi.mock("../platform/files", () => ({ openFile: vi.fn() }));
vi.mock("../ipc/documents", () => ({
  listDocuments: vi.fn(),
  importPdfAndGenerateFlashcards: vi.fn(),
  retryPdfFlashcardGeneration: vi.fn(),
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
    pageCount: 10,
    extractedPageCount: 10,
    renderedPageCount: 10,
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
    vi.mocked(platformFiles.openFile).mockResolvedValue(null);
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([]);
    vi.mocked(documentIpc.openDocumentSource).mockResolvedValue(undefined);
    vi.mocked(documentIpc.cancelPdfFlashcardGeneration).mockResolvedValue(undefined);
  });

  it("shows a simple upload-first empty state", async () => {
    renderDocuments();
    expect(await screen.findByRole("heading", { name: "No PDFs yet" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Upload PDF" })).toBeTruthy();
    expect(screen.queryByText(/review generated/u)).toBeNull();
    expect(screen.queryByText(/JSON/u)).toBeNull();
  });

  it("uploads one PDF and immediately creates a normal deck without a review step", async () => {
    const completed = makeDocument();
    vi.mocked(platformFiles.openFile).mockResolvedValue("/tmp/cardiac-physiology.pdf");
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
    await screen.findByRole("heading", { name: "No PDFs yet" });

    fireEvent.click(screen.getByRole("button", { name: "Upload PDF" }));

    await waitFor(() => {
      expect(documentIpc.importPdfAndGenerateFlashcards).toHaveBeenCalledWith(
        "/tmp/cardiac-physiology.pdf",
      );
      expect(refreshDecks).toHaveBeenCalled();
      expect(showToast).toHaveBeenCalledWith("success", "Created 7 flashcards in a new deck.");
    });
    expect(await screen.findByRole("heading", { name: "Cardiac physiology" })).toBeTruthy();
    expect(screen.getByText(/normal editing, scheduling, archive, and deletion tools/u)).toBeTruthy();
  });

  it("keeps a failed PDF in the library and offers a direct retry", async () => {
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
    expect(screen.queryByRole("button", { name: "Delete PDF" })).toBeNull();
  });

  it("offers regeneration when the previously generated deck was deleted", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([
      makeDocument({ generatedDeckId: null, lifecycleState: "ready" }),
    ]);
    renderDocuments();

    expect(await screen.findByText("Ready to regenerate")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Retry generation" })).toBeTruthy();
    expect(screen.getByText(/previous generated deck was deleted/u)).toBeTruthy();
  });

  it("opens the managed PDF through the native document command", async () => {
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([makeDocument()]);
    renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "Open PDF" }));

    await waitFor(() => {
      expect(documentIpc.openDocumentSource).toHaveBeenCalledWith(documentId);
    });
  });

  it("permanently deletes a PDF only after confirmation and preserves deck citations", async () => {
    vi.mocked(documentIpc.listDocuments)
      .mockResolvedValueOnce([makeDocument()])
      .mockResolvedValue([]);
    vi.mocked(documentIpc.deletePdfDocument).mockResolvedValue({
      documentId,
      preservedDeckCitationCount: 1,
    });
    const { showToast, refreshDecks } = renderDocuments();

    await screen.findByRole("heading", { name: "Cardiac physiology" });
    fireEvent.click(screen.getByRole("button", { name: "Delete PDF" }));
    const dialog = await screen.findByRole("dialog", { name: "Delete this PDF permanently?" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Delete PDF" }));

    await waitFor(() => {
      expect(documentIpc.deletePdfDocument).toHaveBeenCalledWith(documentId, 2);
      expect(refreshDecks).toHaveBeenCalled();
      expect(showToast).toHaveBeenCalledWith(
        "success",
        "PDF deleted. Preserved text source citations in 1 generated deck.",
      );
    });
  });
});
