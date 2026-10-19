import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { applyDocumentAnnotationChanges, getDocumentAnnotations, getDocumentPageText, renderDocumentPageTile } from "./documentAnnotations";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

const documentId = "a".repeat(32);

describe("document annotation IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({});
  });

  it("reads a document's pages and annotations through one narrow command", async () => {
    await getDocumentAnnotations(documentId);
    expect(invoke).toHaveBeenCalledWith("get_document_annotations", { input: { documentId } });
  });

  it("writes whole annotation states and deletions, so a repeated request is safe", async () => {
    const shape = { kind: "line" as const, x1: 1, y1: 2, x2: 30, y2: 40, color: "#2563eb", strokeWidth: 3 };
    await applyDocumentAnnotationChanges(documentId, [
      { op: "put", id: "b".repeat(32), pageNumber: 2, shape },
      { op: "delete", id: "c".repeat(32) },
    ]);
    expect(invoke).toHaveBeenCalledWith("apply_document_annotation_changes", {
      input: {
        documentId,
        changes: [
          { op: "put", id: "b".repeat(32), pageNumber: 2, shape },
          { op: "delete", id: "c".repeat(32) },
        ],
      },
    });
  });

  it("asks for one tile of one page drawn at a chosen size", async () => {
    const tile = { pageNumber: 3, pageWidth: 2040, pageHeight: 2640, left: 512, top: 1024, width: 512, height: 512 };
    await renderDocumentPageTile(documentId, tile);
    expect(invoke).toHaveBeenCalledWith("render_document_page_tile", { input: { documentId, ...tile } });
  });

  it("reads one page's positioned text", async () => {
    await getDocumentPageText(documentId, 4);
    expect(invoke).toHaveBeenCalledWith("get_document_page_text", { input: { documentId, pageNumber: 4 } });
  });
});
