import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getStudyGuideReading,
  openStudyGuideSection,
  setStudyGuideSectionRead,
} from "./studyGuideReading";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

const documentId = "a".repeat(32);
const guideId = "b".repeat(32);
const topicId = "c".repeat(32);

describe("study guide reading IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(null);
  });

  it("reads a document guide's reading", async () => {
    await getStudyGuideReading(documentId);
    expect(invoke).toHaveBeenCalledWith("get_document_study_guide_reading", {
      input: { documentId },
    });
  });

  it("names the guide a section was opened or marked in", async () => {
    await openStudyGuideSection(documentId, guideId, topicId);
    expect(invoke).toHaveBeenLastCalledWith("open_document_study_guide_section", {
      input: { documentId, guideId, topicId },
    });
    await setStudyGuideSectionRead(documentId, guideId, topicId, false);
    expect(invoke).toHaveBeenLastCalledWith(
      "set_document_study_guide_section_read",
      { input: { documentId, guideId, topicId, read: false } },
    );
  });
});
