import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as documentIpc from "../ipc/documents";
import * as platformFiles from "../platform/files";
import { DocumentsScreen } from "./DocumentsScreen";

vi.mock("@tauri-apps/plugin-opener", () => ({ openPath: vi.fn() }));
vi.mock("../platform/files", () => ({ openFile: vi.fn() }));
vi.mock("../ipc/documents", () => ({
  listDocuments: vi.fn(),
  importPdfAndGenerateFlashcards: vi.fn(),
  retryPdfFlashcardGeneration: vi.fn(),
  getDocumentSourcePath: vi.fn(),
  setDocumentArchived: vi.fn(),
}));
vi.mock("../ipc/common", async (importOriginal) => {
  const original = await importOriginal<typeof import("../ipc/common")>();
  return {
    ...original,
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
  render(<DocumentsScreen refreshDecks={refreshDecks} showToast={showToast} />);
  return { showToast, refreshDecks };
}

describe("DocumentsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(platformFiles.openFile).mockResolvedValue(null);
    vi.mocked(documentIpc.listDocuments).mockResolvedValue([]);
    vi.mocked(documentIpc.getDocumentSourcePath).mockResolvedValue("/managed/source.pdf");
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
      safeErrorCode: "ai_request_timeout",
      safeErrorMessage: "The AI provider did not respond before the configured timeout.",
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
});
