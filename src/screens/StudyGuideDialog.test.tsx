import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { publishPreparedExport, releasePreparedExport } from "../ipc/externalFiles";
import {
  discardDocumentStudyGuideOverlay,
  generateDocumentStudyGuide,
  getDocumentStudyGuide,
  getDocumentTopicCardIds,
  getDocumentTopicCardProjection,
  prepareDocumentStudyGuidePdf,
  saveDocumentStudyGuideOverlay,
  type DocumentStudyGuide,
  type StudyGuideOverlay,
} from "../ipc/studyGuides";
import { StudyGuideDialog } from "./StudyGuideDialog";

vi.mock("../ipc/studyGuides", async () => {
  const actual = await vi.importActual<typeof import("../ipc/studyGuides")>(
    "../ipc/studyGuides",
  );
  return {
    ...actual,
    discardDocumentStudyGuideOverlay: vi.fn(),
    generateDocumentStudyGuide: vi.fn(),
    getDocumentStudyGuide: vi.fn(),
    getDocumentTopicCardIds: vi.fn(),
    getDocumentTopicCardProjection: vi.fn(),
    prepareDocumentStudyGuidePdf: vi.fn(),
    saveDocumentStudyGuideOverlay: vi.fn(),
  };
});

vi.mock("../ipc/externalFiles", async () => {
  const actual = await vi.importActual<typeof import("../ipc/externalFiles")>(
    "../ipc/externalFiles",
  );
  return {
    ...actual,
    publishPreparedExport: vi.fn(),
    releasePreparedExport: vi.fn(),
  };
});

const documentId = "a".repeat(32);
const topicId = "b".repeat(32);
const guideId = "c".repeat(32);
const linkedCardIds = ["1".repeat(32), "2".repeat(32)];
const onPracticeCards = vi.fn(async (_cardIds: string[]) => undefined);

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

function topicNote(): StudyGuideOverlay {
  return {
    id: "9".repeat(32),
    documentId,
    guideId: null,
    topicId,
    overlayKind: "topic_note",
    baseContentHash: null,
    richDoc: {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Remember end-diastolic volume." }],
        },
      ],
    },
    plainText: "Remember end-diastolic volume.",
    state: "active",
    revision: 3,
    createdAt: 2,
    updatedAt: 3,
  };
}

function conflictedOverride(): StudyGuideOverlay {
  return {
    id: "6".repeat(32),
    documentId,
    guideId: "5".repeat(32),
    topicId,
    overlayKind: "body_override",
    baseContentHash: "4".repeat(64),
    richDoc: {
      type: "doc",
      content: [
        {
          type: "paragraph",
          content: [{ type: "text", text: "My older preload explanation." }],
        },
      ],
    },
    plainText: "My older preload explanation.",
    state: "conflicted",
    revision: 4,
    createdAt: 2,
    updatedAt: 5,
  };
}

const mockedGetGuide = vi.mocked(getDocumentStudyGuide);
const mockedGenerate = vi.mocked(generateDocumentStudyGuide);
const mockedProjection = vi.mocked(getDocumentTopicCardProjection);
const mockedCardIds = vi.mocked(getDocumentTopicCardIds);
const mockedSaveOverlay = vi.mocked(saveDocumentStudyGuideOverlay);
const mockedDiscardOverlay = vi.mocked(discardDocumentStudyGuideOverlay);
const mockedPreparePdf = vi.mocked(prepareDocumentStudyGuidePdf);
const mockedPublish = vi.mocked(publishPreparedExport);
const mockedRelease = vi.mocked(releasePreparedExport);

beforeEach(() => {
  vi.clearAllMocks();
  onPracticeCards.mockClear();
  mockedProjection.mockResolvedValue({
    topicId,
    noteCount: 1,
    cardCount: 2,
    studyableCardCount: 2,
  });
  mockedCardIds.mockResolvedValue(linkedCardIds);
  mockedDiscardOverlay.mockResolvedValue(undefined);
  mockedRelease.mockResolvedValue(undefined);
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
        onPracticeCards={onPracticeCards}
        onClose={vi.fn()}
      />,
    );

    expect(await screen.findByText("No study guide yet")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Create study guide" }));

    expect(await screen.findByRole("heading", { name: "Preload" })).toBeTruthy();
    expect(mockedGenerate).toHaveBeenCalledWith(documentId);
    expect(showToast).toHaveBeenCalledWith("success", "Study guide created.");
  });

  it("renders structured source-grounded sections and exports through native prepared files", async () => {
    mockedGetGuide.mockResolvedValue(guide());
    mockedPreparePdf.mockResolvedValue({
      preparedExport: {
        id: "8".repeat(32),
        purpose: "study_guide_pdf",
        suggestedFilename: "Physiology-study-guide.pdf",
        mediaType: "application/pdf",
        byteSize: 2048,
        sha256: "7".repeat(64),
        expiresAt: 999999,
      },
      guideRevision: 1,
      sectionCount: 1,
      annotationCount: 0,
    });
    mockedPublish.mockResolvedValue({
      preparedExportId: "8".repeat(32),
      displayName: "Physiology-study-guide.pdf",
      destinationKind: "file",
      completedAt: 123,
    });
    const showToast = vi.fn();

    render(
      <StudyGuideDialog
        documentId={documentId}
        documentTitle="Physiology"
        showToast={showToast}
        onPracticeCards={onPracticeCards}
        onClose={vi.fn()}
      />,
    );

    expect(await screen.findByText("Sarcomere stretch changes the force of contraction.")).toBeTruthy();
    const dialog = screen.getByRole("dialog", { name: "Study guide · Physiology" });
    expect(dialog.getAttribute("data-presentation")).toBe("fullscreen");
    expect(dialog.firstElementChild?.className).toContain("workspace");
    expect(screen.getByRole("navigation", { name: "Study guide contents" })).toBeTruthy();
    expect(screen.getByRole("article")).toBeTruthy();
    expect(screen.getByText("Page 2–3")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Export PDF" }));

    await waitFor(() => {
      expect(mockedPreparePdf).toHaveBeenCalledWith(documentId, true);
      expect(mockedPublish).toHaveBeenCalledWith("8".repeat(32));
      expect(mockedRelease).toHaveBeenCalledWith("8".repeat(32));
    });
    expect(showToast).toHaveBeenCalledWith(
      "success",
      "Exported study guide revision 1 with 1 section.",
    );
  });

  it("routes exact topic-linked cards through the app practice capability", async () => {
    mockedGetGuide.mockResolvedValue(guide());

    render(
      <StudyGuideDialog
        documentId={documentId}
        documentTitle="Physiology"
        showToast={vi.fn()}
        onPracticeCards={onPracticeCards}
        onClose={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Study linked cards (2)" }));

    await waitFor(() => {
      expect(mockedCardIds).toHaveBeenCalledWith(documentId, [topicId]);
      expect(onPracticeCards).toHaveBeenCalledWith(linkedCardIds);
    });
  });

  it("saves topic notes independently of the generated guide revision", async () => {
    mockedGetGuide
      .mockResolvedValueOnce(guide())
      .mockResolvedValueOnce({
        ...guide(),
        overlays: [
          {
            ...topicNote(),
            plainText: "Remember the relationship to end-diastolic volume.",
          },
        ],
      });
    mockedSaveOverlay.mockResolvedValue({
      ...topicNote(),
      plainText: "Remember the relationship to end-diastolic volume.",
    });

    render(
      <StudyGuideDialog
        documentId={documentId}
        documentTitle="Physiology"
        showToast={vi.fn()}
        onPracticeCards={onPracticeCards}
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
    expect(await screen.findByText("Remember the relationship to end-diastolic volume.")).toBeTruthy();
  });

  it("removes an existing topic note through the discard overlay boundary", async () => {
    const withNote = { ...guide(), overlays: [topicNote()] };
    mockedGetGuide.mockResolvedValueOnce(withNote).mockResolvedValueOnce(guide());
    const showToast = vi.fn();

    render(
      <StudyGuideDialog
        documentId={documentId}
        documentTitle="Physiology"
        showToast={showToast}
        onPracticeCards={onPracticeCards}
        onClose={vi.fn()}
      />,
    );

    expect(await screen.findByText("Remember end-diastolic volume.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Remove note" }));

    await waitFor(() => {
      expect(mockedDiscardOverlay).toHaveBeenCalledWith("9".repeat(32), 3);
      expect(showToast).toHaveBeenCalledWith("success", "Topic note removed.");
    });
  });

  it("treats clearing an existing note as removal instead of invalid empty rich content", async () => {
    const withNote = { ...guide(), overlays: [topicNote()] };
    mockedGetGuide.mockResolvedValueOnce(withNote).mockResolvedValueOnce(guide());

    render(
      <StudyGuideDialog
        documentId={documentId}
        documentTitle="Physiology"
        showToast={vi.fn()}
        onPracticeCards={onPracticeCards}
        onClose={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Edit note" }));
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "   " } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => expect(mockedDiscardOverlay).toHaveBeenCalledWith("9".repeat(32), 3));
    expect(mockedSaveOverlay).not.toHaveBeenCalled();
  });

  it("shows preserved conflicted section text and lets the learner discard it", async () => {
    const conflict = conflictedOverride();
    mockedGetGuide
      .mockResolvedValueOnce({ ...guide(), overlays: [conflict] })
      .mockResolvedValueOnce(guide());
    const showToast = vi.fn();

    render(
      <StudyGuideDialog
        documentId={documentId}
        documentTitle="Physiology"
        showToast={showToast}
        onPracticeCards={onPracticeCards}
        onClose={vi.fn()}
      />,
    );

    expect(await screen.findByText("My older preload explanation.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Discard older edit" }));

    await waitFor(() => {
      expect(mockedDiscardOverlay).toHaveBeenCalledWith("6".repeat(32), 4);
      expect(showToast).toHaveBeenCalledWith("success", "Older section edit discarded.");
    });
  });
});
