import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getDocumentStudyGuideCanvas, openDocumentStudyGuidePdf } from "./studyGuides";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

const documentId = "a".repeat(32);

describe("study guide IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(null);
  });

  it("lays out the whole guide for the canvas in the chosen theme", async () => {
    await getDocumentStudyGuideCanvas(documentId, "sage");
    expect(invoke).toHaveBeenCalledWith("get_document_study_guide_canvas", {
      input: { documentId, theme: "sage" },
    });
  });

  it("opens the guide in the system's PDF reader in a theme", async () => {
    await openDocumentStudyGuidePdf(documentId, "plum");
    expect(invoke).toHaveBeenCalledWith("open_document_study_guide_pdf", {
      input: { documentId, theme: "plum" },
    });
  });
});
