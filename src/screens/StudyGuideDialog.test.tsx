import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ShowToast } from "../components/Toast";
import { publishPreparedExport, releasePreparedExport } from "../ipc/externalFiles";
import {
  cancelDocumentStudyGuideGeneration,
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
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { StudyGuideDialog } from "./StudyGuideDialog";

vi.mock("../ipc/studyGuides", async () => {
  const actual = await vi.importActual<typeof import("../ipc/studyGuides")>(
    "../ipc/studyGuides",
  );
  return {
    ...actual,
    cancelDocumentStudyGuideGeneration: vi.fn(),
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
          callouts: [],
          tables: [],
          workedExamples: [],
          retrievalChecks: [],
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
const mockedCancel = vi.mocked(cancelDocumentStudyGuideGeneration);
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
  mockedCancel.mockResolvedValue(undefined);
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

async function selectSectionAction(label: string): Promise<void> {
  const trigger = await screen.findByLabelText(/Section actions for/u);
  fireEvent.click(trigger);
  const menu = await waitFor(() => {
    const value = document.querySelector<HTMLElement>(
      '[role="menu"][aria-label^="Section actions for"]',
    );
    if (!value) throw new Error("Section actions menu did not open.");
    return value;
  });
  fireEvent.click(within(menu).getByText(label));
}

describe("StudyGuideDialog", () => {
  it("creates a guide when no active revision exists", async () => {
    mockedGetGuide.mockResolvedValue(null);
    mockedGenerate.mockResolvedValue(guide());
    const showToast = vi.fn();

    renderGuide({ showToast });

    expect(await screen.findByText("No study guide yet")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Generate study guide" }));

    expect(await screen.findByRole("heading", { name: "Preload" })).toBeTruthy();
    expect(mockedGenerate).toHaveBeenCalledWith(documentId);
    expect(showToast).toHaveBeenCalledWith("success", "Study guide created.");
  });

  it("uses the same generating button as the red stop control", async () => {
    mockedGetGuide.mockResolvedValue(null);
    let resolveGeneration!: (value: DocumentStudyGuide) => void;
    mockedGenerate.mockReturnValue(
      new Promise((resolve) => {
        resolveGeneration = resolve;
      }),
    );

    renderGuide();
    fireEvent.click(await screen.findByRole("button", { name: "Generate study guide" }));

    const stop = await screen.findByRole("button", {
      name: "Generating study guide; click to stop",
    });
    expect(stop.textContent).toContain("Generating study guide");
    expect(stop.querySelector("svg")).not.toBeNull();
    expect(screen.queryByRole("button", { name: /Stop study guide/u })).toBeNull();
    expect(screen.queryByRole("button", { name: /Stop generation/u })).toBeNull();

    fireEvent.click(stop);
    await waitFor(() => expect(mockedCancel).toHaveBeenCalledWith(documentId));
    expect(
      screen.getByRole("button", { name: "Stopping study guide generation" }),
    ).toBeTruthy();

    resolveGeneration(guide());
    expect(await screen.findByRole("heading", { name: "Preload" })).toBeTruthy();
  });

  it("renders an existing guide without revision metadata or a regenerate action", async () => {
    mockedGetGuide.mockResolvedValue(guide());
    const onClose = vi.fn();
    renderGuide({ onClose });

    expect(await screen.findByText("Sarcomere stretch changes the force of contraction.")).toBeTruthy();
    const workspace = screen.getByRole("region", { name: "Study guide · Physiology" });
    expect(workspace.getAttribute("data-inline-workspace")).toBe("true");
    expect(screen.queryByRole("dialog", { name: "Study guide · Physiology" })).toBeNull();
    expect(workspace.getAttribute("aria-modal")).toBeNull();
    expect(screen.getByRole("navigation", { name: "Study guide contents" })).toBeTruthy();
    expect(screen.getByRole("article")).toBeTruthy();
    expect(screen.getByText("Page 2–3")).toBeTruthy();
    expect(screen.getByText("1 section")).toBeTruthy();
    expect(screen.queryByText("Revision 1")).toBeNull();
    expect(screen.queryByRole("button", { name: "Regenerate study guide" })).toBeNull();

    const close = screen.getByRole("button", { name: "Close study guide" });
    expect(close.querySelector("svg")?.getAttribute("width")).toBe("24");
    fireEvent.focus(close);
    expect(screen.queryByRole("tooltip")).toBeNull();
    fireEvent.click(close);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("exports through native prepared files without exposing an internal revision", async () => {
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
      mathSourceFallback: false,
    });
    mockedPublish.mockResolvedValue({
      preparedExportId: "8".repeat(32),
      displayName: "Physiology-study-guide.pdf",
      destinationKind: "file",
      completedAt: 123,
    });
    const showToast = vi.fn();
    renderGuide({ showToast });

    fireEvent.click(await screen.findByRole("button", { name: "Export PDF" }));

    await waitFor(() => {
      expect(mockedPreparePdf).toHaveBeenCalledWith(documentId, true);
      expect(mockedPublish).toHaveBeenCalledWith("8".repeat(32));
      expect(mockedRelease).toHaveBeenCalledWith("8".repeat(32));
    });
    expect(showToast).toHaveBeenCalledWith(
      "success",
      "Exported study guide with 1 section.",
    );
  });


  it.each([true, false])("discloses PDF math and releases the prepared file when accepted=%s", async (accepted) => {
    const value = guide();
    value.overview.summary = String.raw`Solve \(x^2\).`;
    mockedGetGuide.mockResolvedValue(value);
    mockedPreparePdf.mockResolvedValue({
      preparedExport: { id: "8".repeat(32), purpose: "study_guide_pdf", suggestedFilename: "math.pdf", mediaType: "application/pdf", byteSize: 2048, sha256: "7".repeat(64), expiresAt: 999999 },
      guideRevision: 1, sectionCount: 1, annotationCount: 0, mathSourceFallback: true,
    });
    mockedPublish.mockResolvedValue({ preparedExportId: "8".repeat(32), displayName: "math.pdf", destinationKind: "file", completedAt: 123 });
    const showToast = vi.fn();
    renderGuide({ showToast });
    fireEvent.click(await screen.findByRole("button", { name: "Export PDF" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Export math as source notation?" });
    expect(mockedPublish).not.toHaveBeenCalled();
    expect(within(dialog).getByText(/readable LaTeX source rather than typeset formulas/u)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: accepted ? "Export PDF" : "Cancel" }));
    await waitFor(() => expect(mockedRelease).toHaveBeenCalledWith("8".repeat(32)));
    expect(mockedPublish).toHaveBeenCalledTimes(accepted ? 1 : 0);
    expect(mockedSaveOverlay).not.toHaveBeenCalled();
    expect(mockedGenerate).not.toHaveBeenCalled();
    if (!accepted) expect(showToast).not.toHaveBeenCalled();
  });

  it("collapses per-section controls into one accessible overflow menu", async () => {
    mockedGetGuide.mockResolvedValue(guide());
    renderGuide();
    const trigger = await screen.findByRole("button", { name: /Section actions for/u });
    expect(screen.queryByRole("button", { name: "Study linked cards (2)" })).toBeNull();
    fireEvent.click(trigger);
    const menu = await screen.findByRole("menu", { name: /Section actions for/u });
    expect(within(menu).getByRole("menuitem", { name: "Study linked cards (2)" })).toBeTruthy();
    expect(within(menu).getByRole("menuitem", { name: "Open source" })).toBeTruthy();
    expect(within(menu).getByRole("menuitem", { name: "Add note" })).toBeTruthy();
    expect(within(menu).getByRole("menuitem", { name: "Edit section" })).toBeTruthy();
  });

  it("routes exact topic-linked cards through the app practice capability", async () => {
    mockedGetGuide.mockResolvedValue(guide());
    renderGuide();

    await selectSectionAction("Study linked cards (2)");

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
    renderGuide();

    await selectSectionAction("Add note");
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
    expect(
      await screen.findByText("Remember the relationship to end-diastolic volume."),
    ).toBeTruthy();
  });

  it("removes an existing topic note through the discard overlay boundary", async () => {
    const withNote = { ...guide(), overlays: [topicNote()] };
    mockedGetGuide.mockResolvedValueOnce(withNote).mockResolvedValueOnce(guide());
    const showToast = vi.fn();
    renderGuide({ showToast });

    expect(await screen.findByText("Remember end-diastolic volume.")).toBeTruthy();
    await selectSectionAction("Remove note");

    await waitFor(() => {
      expect(mockedDiscardOverlay).toHaveBeenCalledWith("9".repeat(32), 3);
      expect(showToast).toHaveBeenCalledWith("success", "Topic note removed.");
    });
  });

  it("treats clearing an existing note as removal instead of invalid empty rich content", async () => {
    const withNote = { ...guide(), overlays: [topicNote()] };
    mockedGetGuide.mockResolvedValueOnce(withNote).mockResolvedValueOnce(guide());
    renderGuide();

    await selectSectionAction("Edit note");
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
    renderGuide({ showToast });

    expect(await screen.findByText("My older preload explanation.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Discard older edit" }));

    await waitFor(() => {
      expect(mockedDiscardOverlay).toHaveBeenCalledWith("6".repeat(32), 4);
      expect(showToast).toHaveBeenCalledWith("success", "Older section edit discarded.");
    });
  });
});


describe("Study-guide mathematical presentation", () => {
  it("renders every structured body field without mutating source text, hashes or locators", async () => {
    const loaded = guide();
    loaded.overview.summary = String.raw`Summary \(x_1\)`;
    loaded.overview.learningObjectives = [String.raw`Find \(x_2\)`];
    loaded.overview.keyConcepts = [String.raw`Know \(x_3\)`];
    const section = loaded.sections[0];
    section.title = String.raw`Topic \(x_4\)`;
    section.content = {
      overview: String.raw`Overview \(x_5\)`, keyPoints: [String.raw`Point \(x_6\)`],
      explanations: [{ heading: String.raw`Why \(x_7\)`, body: String.raw`Because \[x_8\]` }],
      importantTerms: [{ term: String.raw`Term \(x_9\)`, explanation: String.raw`Meaning \(x_{10}\)` }],
      processSteps: [String.raw`Apply \(x_{11}\)`],
      comparisons: [{ subjectA: String.raw`\(x_{12}\)`, subjectB: String.raw`\(x_{13}\)`, relationship: String.raw`Related \(x_{14}\)` }],
      commonMisconceptions: [String.raw`Not \(x_{15}\)`],
      callouts: [{ kind: "key_idea", title: String.raw`Remember \(x_{18}\)`, body: String.raw`Callout \(x_{19}\)` }],
      tables: [{ title: String.raw`Table \(x_{20}\)`, columns: [String.raw`A \(x_{21}\)`, String.raw`B \(x_{22}\)`], rows: [[String.raw`C \(x_{23}\)`, String.raw`D \(x_{24}\)`]], caption: String.raw`Caption \(x_{25}\)` }],
      workedExamples: [{ title: String.raw`Example \(x_{26}\)`, prompt: String.raw`Prompt \(x_{27}\)`, steps: [String.raw`Step \(x_{28}\)`], answer: String.raw`Answer \(x_{29}\)`, takeaway: String.raw`Takeaway \(x_{30}\)` }],
      retrievalChecks: [{ prompt: String.raw`Recall \(x_{31}\)`, answer: String.raw`Response \(x_{32}\)` }],
      visualExplanations: [{ title: String.raw`See \(x_{16}\)`, explanation: String.raw`Diagram \(x_{17}\)` }],
    };
    const before = JSON.stringify(loaded);
    mockedGetGuide.mockResolvedValue(loaded);
    const { container } = renderGuide();
    await waitFor(() => expect(container.querySelectorAll("math annotation")).toHaveLength(32));
    expect(screen.getByText("Page 2–3")).toBeTruthy();
    expect(JSON.stringify(loaded)).toBe(before);
    expect(mockedSaveOverlay).not.toHaveBeenCalled();
    expect(mockedGenerate).not.toHaveBeenCalled();
    expect(container.querySelector("p div,a a")).toBeNull();
  });

  it("shows active and conflicted note formulas and saves original LaTeX instead of renderer DOM", async () => {
    const source = String.raw`My note \(\frac{a}{b}\)`;
    const note = { ...topicNote(), plainText: source };
    const conflict = { ...conflictedOverride(), plainText: String.raw`Older edit \(x^2\)` };
    const loaded = { ...guide(), overlays: [note, conflict] };
    mockedGetGuide.mockResolvedValue(loaded);
    mockedSaveOverlay.mockResolvedValue(note);
    const { container } = renderGuide();
    await waitFor(() => expect(container.querySelectorAll("math annotation")).toHaveLength(2));
    expect(screen.getByText("Older section edit needs review.")).toBeTruthy();
    await selectSectionAction("Edit note");
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe(source);
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(mockedSaveOverlay).not.toHaveBeenCalled();
    await selectSectionAction("Edit note");
    const edited = source + String.raw` and \(c\)`;
    fireEvent.change(screen.getByRole("textbox"), { target: { value: edited } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() => expect(mockedSaveOverlay).toHaveBeenCalledWith(expect.objectContaining({
      overlayId: note.id, overlayKind: "topic_note", plainText: edited, expectedRevision: 3,
      richDoc: { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: edited }] }] },
    })));
    expect(JSON.stringify(mockedSaveOverlay.mock.calls)).not.toContain("MathML");
  });

  it("renders fresh generated content and guide notes through the same offline reader", async () => {
    const generated = guide();
    generated.sections[0].content.overview = String.raw`Generated \(x+y\)`;
    generated.overlays = [{ ...topicNote(), topicId: null, overlayKind: "guide_note", plainText: String.raw`Note \(z\)` }];
    mockedGetGuide.mockResolvedValue(null);
    mockedGenerate.mockResolvedValue(generated);
    const { container } = renderGuide();
    fireEvent.click(await screen.findByRole("button", { name: "Generate study guide" }));
    await waitFor(() => expect(container.querySelectorAll("math annotation")).toHaveLength(2));
    expect(mockedGenerate).toHaveBeenCalledTimes(1);
    expect(mockedSaveOverlay).not.toHaveBeenCalled();
  });
});

function renderGuide({
  showToast = vi.fn() as ShowToast,
  onClose = vi.fn() as () => void,
}: {
  showToast?: ShowToast;
  onClose?: () => void;
} = {}) {
  return render(
    <OverlayProvider><StudyGuideDialog
      documentId={documentId}
      documentTitle="Physiology"
      showToast={showToast}
      onPracticeCards={onPracticeCards}
      onClose={onClose}
    /></OverlayProvider>,
  );
}
