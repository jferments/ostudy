import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  cancelPdfFlashcardGeneration,
  deletePdfDocument,
  importPdfAndGenerateFlashcards,
  listDocuments,
  openDocumentSource,
  retryPdfFlashcardGeneration,
  setDocumentArchived,
} from "./documents";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const documentId = "a".repeat(32);

describe("document IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({});
  });

  it("lists active or archived documents through a bounded command", async () => {
    await listDocuments(true);
    expect(invoke).toHaveBeenCalledWith("list_documents", { includeArchived: true });
  });

  it("establishes document identity before starting cancellable PDF generation", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ documentId })
      .mockResolvedValueOnce({ deckId: "b".repeat(32) });

    await importPdfAndGenerateFlashcards("/tmp/chapter.pdf");

    expect(invoke).toHaveBeenNthCalledWith(1, "import_pdf_document", {
      input: { sourcePath: "/tmp/chapter.pdf" },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "retry_pdf_flashcard_generation", {
      input: { documentId },
    });
  });

  it("uses narrow retry, cancel, native-open, deletion, and archive commands", async () => {
    await retryPdfFlashcardGeneration(documentId);
    expect(invoke).toHaveBeenCalledWith("retry_pdf_flashcard_generation", {
      input: { documentId },
    });

    vi.mocked(invoke).mockClear();
    await cancelPdfFlashcardGeneration(documentId);
    expect(invoke).toHaveBeenCalledWith("cancel_pdf_flashcard_generation", { documentId });

    vi.mocked(invoke).mockClear();
    await openDocumentSource(documentId);
    expect(invoke).toHaveBeenCalledWith("open_document_source", { documentId });

    vi.mocked(invoke).mockClear();
    await deletePdfDocument(documentId, 4);
    expect(invoke).toHaveBeenCalledWith("delete_pdf_document", {
      input: { documentId, revision: 4 },
    });

    vi.mocked(invoke).mockClear();
    await setDocumentArchived(documentId, 4, true);
    expect(invoke).toHaveBeenCalledWith("set_document_archived", {
      input: { documentId, revision: 4, archived: true },
    });
  });
});
