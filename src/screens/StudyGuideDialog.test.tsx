import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  generateDocumentStudyGuide,
  getDocumentStudyGuide,
  getDocumentTopicCardIds,
  getDocumentTopicCardProjection,
  saveDocumentStudyGuideOverlay,
  type DocumentStudyGuide,
} from "../ipc/studyGuides";
import { StudyGuideDialog } from "./StudyGuideDialog";

vi.mock("../ipc/studyGuides", async () => {
  const actual = await vi.importActual<typeof import("../ipc/studyGuides")>(
    "../ipc/studyGuides",
  );
  return {
    ...actual,
    generateDocumentStudyGuide: vi.fn(),
    getDocumentStudyGuide: vi.fn(),
    getDocumentTopicCardIds: vi.fn(),
    getDocumentTopicCardProjection: vi.fn(),
    saveDocumentStudyGuideOverlay: vi.fn(),
  };
});

const documentId = "a".repeat(32);
const topicId = "b".repeat(32);
const guideId = "c".repeat(32);

function guide(): DocumentStudyGuide {
  return {
    id: guideId,
    documentId,
    planId: "d".repeat(32),
    jobId: "e".repeat(32),
    revision: 1,
    schemaVersion: 1,
    promptVersion: 1,
    assemblyVersion: 1,
    overview: {
      summary: "How cardiac output is regulated.",
      learningObjectives: ["Explain preload"],
      keyConcepts: ["Frank-Starling mechanism"],
    },
    overviewText: "How cardiac output is regulated.",
    sections: [
      {
        topicId,
        parentTopicId: null,
        sortIndex: 0,
        title: "Preload",
        topicSummary: "Ventricular filling before contraction.",
        content: {
          overview: "Preload reflects ventricular filling and stretch before systole.",
          keyPoints: ["More filling can increase stroke volume."],
          explanations: [
            {
              heading: "Mechanism",
              body: "Sarcomere stretch changes the force of contraction.",
            },
          ],
          importantTerms: [
            { term: "Preload", explanation: "End-diastolic ventricular stretch." },
          ],
          processSteps: [],
          comparisons: [],
          commonMisconceptions: [],
          visualExplanations: [],
        },
        contentText: "Preload reflects ventricular filling and stretch before systole.",
        contentHash: "f".repeat(64),
        sourceLocator: {
          unitKind: "page",
          unitNumbers: [2, 3],
          assetIds: [],
        },
      },
    ],
    overlays: [],
    createdAt: 1,
    updatedAt: 1,
    publishedAt: 1,
  };
}

const mockedGetGuide = vi.mocked(getDocumentStudyGuide);
const mockedGenerate = vi.mocked(generateDocumentStudyGuide);
const mockedProjection = vi.mocked(getDocumentTopicCardProjection);
const mockedCardIds = vi.mocked(getDocumentTopicCardIds);
const mockedSaveOverlay = vi.mocked(saveDocumentStudyGuideOverlay);

beforeEach(() => {
  vi.clearAllMocks();
  mockedProjection.mockResolvedValue({
    topicId,
    noteCount: 1,
    cardCount: 2,
    studyableCardCount: 2,
  });
  mockedCardIds.mockResolvedValue(["1".repeat(32), "2".repeat(32)]);
});

describe("StudyGuideDialog", () => {
  it("creates a guide when no active revision exists", async () => {
    mockedGetGuide.mockResolvedValue(null);
    mockedGenerate.mockResolvedValue(guide());
    const showToast = vi.fn();

    render(
      <StudyGuideDialog
        documentId={documentId}
        documentTitle="Physiology"
        showToast={showToast}
        onClose={vi.fn()}
      />,
    );

    expect(await screen.findByText("No study guide yet")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Create study guide" }));

    expect(await screen.findByRole("heading", { name: "Preload" })).toBeInTheDocument();
    expect(mockedGenerate).toHaveBeenCalledWith(documentId);
    expect(showToast).toHaveBeenCalledWith("success", "Study guide created.");
  });

  it("renders structured source-grounded sections and a PDF export action", async () => {
    mockedGetGuide.mockResolvedValue(guide());
    const print = vi.spyOn(window, "print").mockImplementation(() => undefined);

    render(
      <StudyGuideDialog
        documentId={documentId}
        documentTitle="Physiology"
        showToast={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    expect(await screen.findByText("Sarcomere stretch changes the force of contraction.")).toBeInTheDocument();
    expect(screen.getByText("Page 2–3")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Export PDF" }));
    expect(print).toHaveBeenCalledOnce();
    print.mockRestore();
  });

  it("saves topic notes independently of the generated guide revision", async () => {
    mockedGetGuide
      .mockResolvedValueOnce(guide())
      .mockResolvedValueOnce({
        ...guide(),
        overlays: [
          {
            id: "9".repeat(32),
            documentId,
            guideId: null,
            topicId,
            overlayKind: "topic_note",
            baseContentHash: null,
            richDoc: { type: "doc", content: [] },
            plainText: "Remember the relationship to end-diastolic volume.",
            state: "active",
            revision: 1,
            createdAt: 2,
            updatedAt: 2,
          },
        ],
      });
    mockedSaveOverlay.mockResolvedValue({
      id: "9".repeat(32),
      documentId,
      guideId: null,
      topicId,
      overlayKind: "topic_note",
      baseContentHash: null,
      richDoc: { type: "doc", content: [] },
      plainText: "Remember the relationship to end-diastolic volume.",
      state: "active",
      revision: 1,
      createdAt: 2,
      updatedAt: 2,
    });

    render(
      <StudyGuideDialog
        documentId={documentId}
        documentTitle="Physiology"
        showToast={vi.fn()}
        onClose={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Add note" }));
    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: "Remember the relationship to end-diastolic volume." },
    });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(mockedSaveOverlay).toHaveBeenCalledWith(
        expect.objectContaining({
          documentId,
          topicId,
          overlayKind: "topic_note",
          guideId: undefined,
        }),
      ),
    );
    expect(await screen.findByText("Remember the relationship to end-diastolic volume.")).toBeInTheDocument();
  });
});
