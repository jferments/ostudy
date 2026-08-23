import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as studyGuides from "../ipc/studyGuides";
import { StudyGuideLibrarySection } from "./StudyGuideLibrarySection";

vi.mock("../ipc/studyGuides", () => ({
  listDocumentStudyGuides: vi.fn(),
}));
vi.mock("./StudyGuideDialog", () => ({
  StudyGuideDialog: ({ documentTitle, onClose }: { documentTitle: string; onClose: () => void }) => (
    <div role="dialog" aria-label={`Guide ${documentTitle}`}>
      <button type="button" onClick={onClose}>
        Close guide
      </button>
    </div>
  ),
}));

describe("StudyGuideLibrarySection", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("lists generated guides inline and opens the selected guide", async () => {
    vi.mocked(studyGuides.listDocumentStudyGuides).mockResolvedValue([
      {
        documentId: "a".repeat(32),
        documentTitle: "Distributive shock",
        originalFilename: "shock.pdf",
        sourceFormat: "pdf",
        sourceUnitKind: "page",
        sourceUnitCount: 12,
        guideRevision: 3,
        publishedAt: 10,
        archived: false,
      },
    ]);

    render(<StudyGuideLibrarySection showToast={vi.fn()} onPracticeCards={vi.fn()} />);

    expect(await screen.findByRole("heading", { name: "Study guides" })).toBeTruthy();
    expect(await screen.findByText("Distributive shock")).toBeTruthy();
    expect(screen.getByText(/12 pages/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Open study guide" }));
    expect(screen.getByRole("dialog", { name: "Guide Distributive shock" })).toBeTruthy();
  });

  it("directs learners to Documents when no guide has been generated", async () => {
    vi.mocked(studyGuides.listDocumentStudyGuides).mockResolvedValue([]);

    render(<StudyGuideLibrarySection showToast={vi.fn()} onPracticeCards={vi.fn()} />);

    expect(await screen.findByText("No study guides yet")).toBeTruthy();
    expect(screen.getByText(/Generate one from the Documents screen/u)).toBeTruthy();
  });
});
