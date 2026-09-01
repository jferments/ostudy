import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  cancelDocumentFlashcardGeneration,
  deleteDocument,
  generateMoreDocumentFlashcards,
  importDocumentAndGenerateFlashcards,
  listDocuments,
  openDocumentSource,
  retryDocumentFlashcardGeneration,
} from "./documents";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const documentId = "a".repeat(32);

describe("document IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({});
  });

  it("lists the bounded active document library without archive state", async () => {
    await listDocuments();
    expect(invoke).toHaveBeenCalledWith("list_documents");
  });

  it("establishes document identity before starting cancellable document generation", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({ documentId })
      .mockResolvedValueOnce({ deckId: "b".repeat(32) });

    await importDocumentAndGenerateFlashcards("1".repeat(32));

    expect(invoke).toHaveBeenNthCalledWith(1, "import_document", {
      input: { stagedFileId: "1".repeat(32) },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "run_document_flashcard_generation", {
      input: { documentId },
    });
  });

  it("uses narrow generation, additional-pass, cancel, native-open, and deletion commands", async () => {
    await retryDocumentFlashcardGeneration(documentId);
    expect(invoke).toHaveBeenCalledWith("run_document_flashcard_generation", {
      input: { documentId },
    });

    vi.mocked(invoke).mockClear();
    await generateMoreDocumentFlashcards(documentId);
    expect(invoke).toHaveBeenCalledWith("generate_more_pdf_flashcards", {
      input: { documentId },
    });

    vi.mocked(invoke).mockClear();
    await cancelDocumentFlashcardGeneration(documentId);
    expect(invoke).toHaveBeenCalledWith("cancel_document_flashcard_generation", { documentId });

    vi.mocked(invoke).mockClear();
    await openDocumentSource(documentId);
    expect(invoke).toHaveBeenCalledWith("open_document_source", { documentId });

    vi.mocked(invoke).mockClear();
    await deleteDocument(documentId, 4);
    expect(invoke).toHaveBeenCalledWith("delete_document", {
      input: { documentId, revision: 4 },
    });
  });
});
