import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getDocumentStudyGuideSectionPage } from "./studyGuides";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

const documentId = "a".repeat(32);
const topicId = "c".repeat(32);

describe("study guide IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(null);
  });

  it("lays out one section as a designed page in the chosen theme", async () => {
    await getDocumentStudyGuideSectionPage(documentId, topicId, "sage");
    expect(invoke).toHaveBeenCalledWith("get_document_study_guide_section_page", {
      input: { documentId, topicId, theme: "sage" },
    });
  });
});
