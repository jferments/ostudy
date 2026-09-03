import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { DocumentStudyGuide } from "../ipc/studyGuides";
import * as studyGuides from "../ipc/studyGuides";
import { StudyGuideWorkspace } from "./StudyGuideWorkspace";

vi.mock("../ipc/studyGuides", () => ({
  cancelDocumentStudyGuideGeneration: vi.fn(),
  discardDocumentStudyGuideOverlay: vi.fn(),
  generateDocumentStudyGuide: vi.fn(),
  getDocumentStudyGuide: vi.fn(),
  getDocumentTopicCardIds: vi.fn(),
  prepareDocumentStudyGuidePdf: vi.fn(),
  saveDocumentStudyGuideOverlay: vi.fn(),
}));
vi.mock("../ipc/documents", () => ({ openDocumentSource: vi.fn() }));
vi.mock("../ipc/externalFiles", () => ({
  publishPreparedExport: vi.fn(),
  releasePreparedExport: vi.fn(),
}));
vi.mock("../platform/backNavigation", () => ({
  registerPlatformBackHandler: vi.fn(() => vi.fn()),
}));

const guide = {
  id: "g".repeat(32),
  documentId: "d".repeat(32),
  revision: 4,
  overview: {
    summary: "Shock is inadequate tissue perfusion.",
    learningObjectives: ["Differentiate major shock states"],
    keyConcepts: ["tissue perfusion"],
  },
  sections: [
    {
      topicId: "t".repeat(32),
      title: "Distributive shock",
      contentText: "Distributive shock reduces systemic vascular resistance.",
      contentHash: "h".repeat(64),
    },
  ],
  overlays: [],
} as unknown as DocumentStudyGuide;

function pending<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((accept) => {
    resolve = accept;
  });
  return { promise, resolve };
}

describe("StudyGuideWorkspace", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(studyGuides.getDocumentStudyGuide).mockResolvedValue(guide);
    vi.mocked(studyGuides.cancelDocumentStudyGuideGeneration).mockResolvedValue(undefined);
  });

  it("keeps application navigation usable and presents a visible close action", async () => {
    const onClose = vi.fn();
    render(
      <>
        <nav aria-label="Primary navigation">
          <button type="button">Flashcards</button>
        </nav>
        <StudyGuideWorkspace
          documentId={guide.documentId}
          documentTitle="Shock physiology"
          showToast={vi.fn()}
          onPracticeCards={vi.fn()}
          onClose={onClose}
        />
      </>,
    );

    expect(await screen.findByRole("heading", { name: "Shock physiology" })).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Primary navigation" })).toBeTruthy();
    expect(screen.queryByRole("dialog")).toBeNull();
    const close = screen.getByRole("button", { name: "Close study guide" });
    expect(close.className).toMatch(/closeButton/u);
    fireEvent.click(close);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("turns the generating button itself into the stop control", async () => {
    const generation = pending<DocumentStudyGuide>();
    vi.mocked(studyGuides.generateDocumentStudyGuide).mockReturnValue(generation.promise);
    render(
      <StudyGuideWorkspace
        documentId={guide.documentId}
        documentTitle="Shock physiology"
        showToast={vi.fn()}
        onPracticeCards={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    await screen.findByRole("heading", { name: "Shock physiology" });
    fireEvent.click(screen.getByRole("button", { name: "Regenerate study guide" }));

    const stop = await screen.findByRole("button", { name: "Stop study guide generation" });
    expect(stop.textContent).toContain("Generating study guide");
    expect(stop.querySelector("span span")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Stop generation/u })).toBeNull();

    fireEvent.click(stop);
    await waitFor(() =>
      expect(studyGuides.cancelDocumentStudyGuideGeneration).toHaveBeenCalledWith(
        guide.documentId,
      ),
    );
    generation.resolve(guide);
  });
});
