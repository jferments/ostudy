import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { applyDocumentAnnotationChanges, getDocumentAnnotations } from "./documentAnnotations";

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
});
