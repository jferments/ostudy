import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getDocumentSourcePath,
  importPdfAndGenerateFlashcards,
  listDocuments,
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

  it("starts the complete PDF-to-deck workflow with only a selected source path", async () => {
    await importPdfAndGenerateFlashcards("/tmp/chapter.pdf");
    expect(invoke).toHaveBeenCalledWith("import_pdf_and_generate_flashcards", {
      input: { sourcePath: "/tmp/chapter.pdf" },
    });
  });

  it("uses narrow retry, source-path, and archive commands", async () => {
    await retryPdfFlashcardGeneration(documentId);
    expect(invoke).toHaveBeenCalledWith("retry_pdf_flashcard_generation", {
      input: { documentId },
    });

    vi.mocked(invoke).mockClear();
    await getDocumentSourcePath(documentId);
    expect(invoke).toHaveBeenCalledWith("get_document_source_path", { documentId });

    vi.mocked(invoke).mockClear();
    await setDocumentArchived(documentId, 4, true);
    expect(invoke).toHaveBeenCalledWith("set_document_archived", {
      input: { documentId, revision: 4, archived: true },
    });
  });
});
