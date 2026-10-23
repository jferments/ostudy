import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { requiredTestValue as required } from "../test/factories";
import { layoutWidths, matchesLayoutQuery, type ViewportLayout } from "../test/viewport";
import type { ShowToast } from "../components/Toast";
import { openDocumentSource } from "../ipc/documents";
import { getMaterialInk, readMaterialInk } from "../ipc/materialInk";
import { publishPreparedExport, releasePreparedExport } from "../ipc/externalFiles";
import { getSettings, type AppSettings } from "../ipc/settings";
import {
  cancelDocumentStudyGuideGeneration,
  discardDocumentStudyGuideOverlay,
  generateDocumentStudyGuide,
  getDocumentStudyGuide,
  getDocumentTopicCardIds,
  getDocumentTopicCardProjection,
  getDocumentStudyGuideSectionPage,
  getDocumentStudyGuideGlancePage,
  getDocumentStudyGuideCanvas,
  openDocumentStudyGuidePdf,
  prepareDocumentStudyGuidePdf,
  saveDocumentStudyGuideOverlay,
  type DocumentStudyGuide,
  type StudyGuideCanvas,
  type PreparedStudyGuidePdf,
  type StudyGuideOverlay,
} from "../ipc/studyGuides";
import {
  getStudyGuideReading,
  openStudyGuideSection,
  setStudyGuideSectionRead,
  type StudyGuideReading,
} from "../ipc/studyGuideReading";
import { LayoutModeProvider } from "../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { StudyGuideDialog } from "./StudyGuideDialog";
import type { GuideSourceSummary } from "./study-guides/guideModel";
import { TutorContext } from "./tutor/TutorContext";
import type { TutorController } from "./tutor/useTutorController";

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
    getDocumentStudyGuideSectionPage: vi.fn(),
    getDocumentStudyGuideGlancePage: vi.fn(),
    getDocumentStudyGuideCanvas: vi.fn(),
    openDocumentStudyGuidePdf: vi.fn(),
    prepareDocumentStudyGuidePdf: vi.fn(),
    saveDocumentStudyGuideOverlay: vi.fn(),
  };
});

vi.mock("../ipc/materialInk", () => ({ getMaterialInk: vi.fn(), readMaterialInk: vi.fn(), applyMaterialInkChanges: vi.fn() }));

vi.mock("../ipc/studyGuideReading", () => ({
  getStudyGuideReading: vi.fn(),
  openStudyGuideSection: vi.fn(),
  setStudyGuideSectionRead: vi.fn(),
}));

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

vi.mock("../ipc/settings", () => ({ getSettings: vi.fn() }));

vi.mock("../ipc/documents", async () => {
  const actual = await vi.importActual<typeof import("../ipc/documents")>(
    "../ipc/documents",
  );
  return { ...actual, openDocumentSource: vi.fn() };
});

const documentId = "a".repeat(32);
const topicId = "b".repeat(32);
const guideId = "c".repeat(32);
const afterloadTopicId = "3".repeat(32);
const linkedCardIds = ["1".repeat(32), "2".repeat(32)];
const onPracticeCards = vi.fn(async (_cardIds: string[]) => undefined);
const pdfSource: GuideSourceSummary = { format: "pdf", filename: "physiology.pdf", unitKind: "page", unitCount: 12 };

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
      keyConcepts: ["Frank-Starling mechanism — Increased ventricular filling can raise stroke volume by increasing myocardial fiber stretch."],
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

/** Two sections, the first with every kind of block a section page draws. */
function twoSectionGuide(): DocumentStudyGuide {
  const value = guide();
  const first = required(value.sections[0], "first section");
  first.content = {
    ...first.content,
    callouts: [{ kind: "warning", title: "Do not confuse", body: "Preload is not afterload." }],
    tables: [{ title: "Hemodynamic comparison", columns: ["Measure", "Effect"], rows: [["Preload", "Stretch"]], caption: "Simplified." }],
    workedExamples: [{
      title: "A fluid bolus",
      prompt: "A patient receives a liter of fluid.",
      steps: ["Venous return rises."],
      answer: "Stroke volume rises.",
      takeaway: "Filling sets the stretch.",
    }],
    commonMisconceptions: ["Preload is the same as blood pressure."],
    retrievalChecks: [
      { prompt: "What raises preload?", answer: "More venous return raises end-diastolic volume." },
      { prompt: "When is preload measured?", answer: "At the end of diastole." },
    ],
  };
  value.sections.push({
    ...first,
    topicId: afterloadTopicId,
    sortIndex: 1,
    title: "Afterload",
    topicSummary: "The load against ejection.",
    content: {
      ...first.content,
      overview: "Afterload is the load the ventricle pumps against.",
      keyPoints: [],
      explanations: [],
      importantTerms: [],
      callouts: [],
      tables: [],
      workedExamples: [],
      commonMisconceptions: [],
      retrievalChecks: [],
    },
    contentText: "Afterload is the load the ventricle pumps against.",
    contentHash: "2".repeat(64),
    sourceLocator: { unitKind: "page", unitNumbers: [4], assetIds: [] },
  });
  return value;
}

const secondTopicId = "7".repeat(32);

function editorialGuide(): DocumentStudyGuide {
  const value = guide();
  const first = required(value.sections[0], "first editorial section");
  const second = {
    ...first,
    topicId: secondTopicId,
    sortIndex: 1,
    title: "raas / sodium balance / vascular tone",
    topicSummary: "RAAS signaling coordinates vascular tone and sodium balance.",
    content: {
      ...first.content,
      overview: "Renin–angiotensin signaling links vascular resistance with aldosterone-mediated sodium balance. Drug classes that interrupt this pathway share mechanistic benefits while differing in contraindications and monitoring.",
      keyPoints: [],
      explanations: [],
      importantTerms: [],
    },
    contentText: "Renin–angiotensin signaling links vascular resistance with sodium balance.",
    contentHash: "8".repeat(64),
    sourceLocator: {
      unitKind: "page",
      unitNumbers: [18, 19],
      assetIds: [],
    },
  };
  first.title = "preload / cardiac output / venous return";
  first.content.keyPoints = [];
  value.schemaVersion = 3;
  value.promptVersion = 7;
  value.assemblyVersion = 4;
  value.overview = {
    summary: "Cardiovascular pharmacology connects drug mechanisms with hemodynamic treatment goals and safety.\n\nThe guide begins with physiologic foundations, then relates major therapeutic pathways to clinical monitoring and decision making.",
    learningObjectives: [
      "Explain how major cardiovascular drug classes alter physiology and treatment goals.",
      "Compare indications, contraindications, and monitoring priorities across therapies.",
    ],
    keyConcepts: [],
  };
  value.editorial = {
    title: "Cardiovascular drugs",
    chapters: [{
      id: "6".repeat(32),
      title: "Hemodynamic foundations",
      introduction: "These sections establish the physiologic relationships needed to understand how cardiovascular drugs change preload, vascular tone, and downstream treatment decisions.",
      topics: [
        { topicId, title: "Preload and cardiac output" },
        { topicId: secondTopicId, title: "Renin–angiotensin system" },
      ],
    }],
  };
  value.sections = [first, second];
  return value;
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

function guideNote(): StudyGuideOverlay {
  return {
    ...topicNote(),
    id: "4".repeat(32),
    topicId: null,
    overlayKind: "guide_note",
    plainText: "Focus on the pressure-volume loop.",
    revision: 2,
  };
}

function sectionOverride(): StudyGuideOverlay {
  return {
    ...topicNote(),
    id: "5".repeat(32),
    guideId,
    overlayKind: "body_override",
    baseContentHash: "f".repeat(64),
    plainText: "In my words: filling stretches the muscle.",
    revision: 1,
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

function preparedPdf(overrides: Partial<PreparedStudyGuidePdf> = {}): PreparedStudyGuidePdf {
  return {
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
    ...overrides,
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
const mockedOpenSource = vi.mocked(openDocumentSource);
const mockedGetReading = vi.mocked(getStudyGuideReading);
const mockedOpenSection = vi.mocked(openStudyGuideSection);
const mockedSetRead = vi.mocked(setStudyGuideSectionRead);
const originalMatchMedia = window.matchMedia;

/** A stand-in for the native reading store: marks and the last opened section. */
function readingStore(initial: Partial<StudyGuideReading> = {}) {
  let state: StudyGuideReading = {
    documentId, guideId, sectionCount: 2, readTopicIds: [], lastTopicId: null, lastOpenedAt: null, ...initial,
  };
  mockedGetReading.mockImplementation(async () => state);
  mockedOpenSection.mockImplementation(async (_documentId, _guideId, topic) => {
    state = { ...state, lastTopicId: topic, lastOpenedAt: 10 };
    return state;
  });
  mockedSetRead.mockImplementation(async (_documentId, _guideId, topic, read) => {
    const others = state.readTopicIds.filter((id) => id !== topic);
    state = { ...state, readTopicIds: read ? [...others, topic] : others };
    return state;
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  // The reader opens as plain text unless a test opens the canvas.
  window.localStorage.setItem("ostudy.guidePageStyle", "plain");
  // Sections read as plain pages unless a test lays one out.
  vi.mocked(getDocumentStudyGuideSectionPage).mockRejectedValue(new Error("no layout in tests"));
  vi.mocked(getDocumentStudyGuideGlancePage).mockRejectedValue(new Error("no layout in tests"));
  vi.mocked(getMaterialInk).mockResolvedValue([]);
  vi.mocked(getSettings).mockResolvedValue({} as AppSettings);
  onPracticeCards.mockClear();
  mockedCancel.mockResolvedValue(undefined);
  mockedProjection.mockImplementation(async (_documentId, topic) => ({
    topicId: topic,
    noteCount: 1,
    cardCount: 2,
    studyableCardCount: 2,
  }));
  mockedCardIds.mockResolvedValue(linkedCardIds);
  mockedDiscardOverlay.mockResolvedValue(undefined);
  mockedRelease.mockResolvedValue(undefined);
  mockedOpenSource.mockResolvedValue(undefined);
  readingStore();
});

afterEach(() => {
  window.matchMedia = originalMatchMedia;
});

async function openSection(title: string): Promise<HTMLElement> {
  const contents = await screen.findByRole("navigation", { name: "Study guide contents" });
  fireEvent.click(within(contents).getByRole("link", { name: title }));
  return screen.findByRole("heading", { level: 1, name: title });
}

async function openMenu(trigger: HTMLElement): Promise<HTMLElement> {
  // Base UI hands a menu trigger its click handling and aria-expanded in the same
  // render, after a passive effect connects the trigger to its root. A click that
  // lands before that is dropped, so wait for the trigger to report closed first.
  await waitFor(() => expect(trigger.getAttribute("aria-expanded")).toBe("false"));
  fireEvent.click(trigger);
  return screen.findByRole("menu", { name: trigger.getAttribute("aria-label") ?? "" });
}

async function chooseFromMenu(trigger: HTMLElement, label: string): Promise<void> {
  const menu = await openMenu(trigger);
  fireEvent.click(within(menu).getByRole("menuitem", { name: label }));
}

async function chooseSectionAction(label: string): Promise<void> {
  await chooseFromMenu(await screen.findByRole("button", { name: /^Section actions for/u }), label);
}

async function chooseGuideAction(label: string): Promise<void> {
  await chooseFromMenu(await screen.findByRole("button", { name: "Study guide actions" }), label);
}

describe("StudyGuideDialog", () => {
  it("creates a guide when no active revision exists", async () => {
    mockedGetGuide.mockResolvedValue(null);
    mockedGenerate.mockResolvedValue(guide());
    const showToast = vi.fn();

    renderGuide({ showToast });

    expect(await screen.findByText("No study guide yet")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Generate study guide" }));

    expect(await screen.findByRole("heading", { level: 1, name: "Physiology" })).toBeTruthy();
    expect(screen.getByRole("link", { name: "Start reading 1 · Preload" })).toBeTruthy();
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
    const showToast = vi.fn();

    renderGuide({ showToast });
    fireEvent.click(await screen.findByRole("button", { name: "Generate study guide" }));

    const stop = await screen.findByRole("button", {
      name: "Generating study guide; click to stop",
    });
    expect(stop.textContent).toContain("Generating study guide");
    expect(stop.querySelector("svg")).not.toBeNull();
    expect(screen.queryByRole("button", { name: /Stop study guide/u })).toBeNull();
    expect(screen.queryByRole("button", { name: /Stop generation/u })).toBeNull();
    // The reader cannot be left while generation runs.
    expect((screen.getByRole("button", { name: "Back to Study Guides" }) as HTMLButtonElement).disabled).toBe(true);

    fireEvent.click(stop);
    await waitFor(() => expect(mockedCancel).toHaveBeenCalledWith(documentId));
    expect(showToast).toHaveBeenCalledWith("info", "Stopping study-guide generation…");
    expect(
      screen.getByRole("button", { name: "Stopping study guide generation" }),
    ).toBeTruthy();

    resolveGeneration(guide());
    expect(await screen.findByRole("heading", { level: 1, name: "Physiology" })).toBeTruthy();
  });

  it("reports a stopped generation without an error", async () => {
    mockedGetGuide.mockResolvedValue(null);
    mockedGenerate.mockRejectedValue({ code: "document_study_guide_canceled", message: "Canceled." });
    const showToast = vi.fn();
    renderGuide({ showToast });

    fireEvent.click(await screen.findByRole("button", { name: "Generate study guide" }));

    await waitFor(() => expect(showToast).toHaveBeenCalledWith(
      "info",
      "Study-guide generation stopped. Saved topic checkpoints can be reused on retry.",
    ));
    // The toast fires before the reader re-renders, so wait for the button to come back.
    expect(await screen.findByRole("button", { name: "Generate study guide" })).toBeTruthy();
    expect(screen.queryByText("Canceled.")).toBeNull();
  });

  it("shows a load failure with a retry instead of offering to generate", async () => {
    mockedGetGuide.mockRejectedValueOnce(new Error("The library is busy.")).mockResolvedValueOnce(guide());
    renderGuide();

    expect(await screen.findByText("The library is busy.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Generate study guide" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));

    expect(await screen.findByRole("heading", { level: 1, name: "Physiology" })).toBeTruthy();
    expect(screen.queryByText("The library is busy.")).toBeNull();
  });

  it("opens on the overview page inside the Study Guides page, not a modal", async () => {
    mockedGetGuide.mockResolvedValue(guide());
    const onClose = vi.fn();
    renderGuide({ onClose, source: pdfSource });

    const overview = await screen.findByRole("article", { name: "Physiology" });
    expect(within(overview).getByRole("heading", { level: 1, name: "Physiology" })).toBeTruthy();
    expect(within(overview).getByText("1 section · about 1 minute")).toBeTruthy();
    const minute = within(overview).getByRole("region", { name: "In one minute" });
    expect(within(minute).getByText("How cardiac output is regulated.")).toBeTruthy();
    expect(within(overview).getByRole("heading", { level: 2, name: "Learning objectives" })).toBeTruthy();
    expect(within(overview).getByText("Explain preload")).toBeTruthy();
    expect(within(overview).getByRole("heading", { level: 2, name: "Key concepts" })).toBeTruthy();
    expect(within(overview).getByText("Frank-Starling mechanism").closest("dt")).toBeTruthy();
    expect(within(overview).getByText(/Increased ventricular filling can raise stroke volume/u)).toBeTruthy();
    expect(screen.queryByText("Revision 1")).toBeNull();
    expect(screen.queryByRole("button", { name: "Regenerate study guide" })).toBeNull();

    const workspace = screen.getByRole("region", { name: "Physiology" });
    expect(workspace.getAttribute("data-inline-workspace")).toBe("true");
    expect(workspace.getAttribute("aria-modal")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(within(workspace).getByText("Study guide")).toBeTruthy();

    const back = screen.getByRole("button", { name: "Back to Study Guides" });
    fireEvent.focus(back);
    expect(screen.queryByRole("tooltip")).toBeNull();
    fireEvent.click(back);
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("names the source with its file kind and opens it from the overview and the section", async () => {
    mockedGetGuide.mockResolvedValue(guide());
    renderGuide({ source: pdfSource });

    const chip = await screen.findByRole("button", { name: "Open source: Physiology · 12 pages" });
    expect(chip.textContent).toContain("PDF");
    fireEvent.click(chip);
    await waitFor(() => expect(mockedOpenSource).toHaveBeenCalledWith(documentId));

    await openSection("Preload");
    fireEvent.click(screen.getByRole("button", { name: "Open source: Physiology · pp. 2–3" }));
    const rail = screen.getByRole("complementary", { name: "About this section" });
    const source = within(rail).getByRole("region", { name: "Source" });
    expect(within(source).getByText("pp. 2–3")).toBeTruthy();
    fireEvent.click(within(source).getByRole("button", { name: "Open source" }));
    await waitFor(() => expect(mockedOpenSource).toHaveBeenCalledTimes(3));
  });

  it("pages through sections with the outline and the previous and next links", async () => {
    mockedGetGuide.mockResolvedValue(twoSectionGuide());
    renderGuide();

    const contents = await screen.findByRole("navigation", { name: "Study guide contents" });
    expect(within(contents).getByRole("link", { name: "Overview" }).getAttribute("aria-current")).toBe("page");
    expect(within(contents).queryByRole("list", { name: "In this section" })).toBeNull();

    const heading = await openSection("Preload");
    expect(screen.getByText("Section 1 of 2 · about 1 minute")).toBeTruthy();
    await waitFor(() => expect(document.activeElement).toBe(heading));
    expect(within(contents).getByRole("link", { name: "Preload" }).getAttribute("aria-current")).toBe("page");
    expect(within(contents).getByRole("link", { name: "Overview" }).getAttribute("aria-current")).toBeNull();

    const blocks = within(contents).getByRole("list", { name: "In this section" });
    expect(within(blocks).getAllByRole("link").map((link) => link.textContent)).toEqual([
      "In one minute", "Mechanism", "Worked example", "Common misconceptions", "Check yourself",
    ]);
    fireEvent.click(within(blocks).getByRole("link", { name: "Mechanism" }));
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole("heading", { level: 2, name: "Mechanism" })));

    const pager = screen.getByRole("navigation", { name: "Previous and next" });
    expect(within(pager).getByRole("link", { name: "Previous Overview" })).toBeTruthy();
    fireEvent.click(within(pager).getByRole("link", { name: "Next 2 · Afterload" }));
    const next = await screen.findByRole("heading", { level: 1, name: "Afterload" });
    expect(screen.getByText("Section 2 of 2 · about 1 minute")).toBeTruthy();
    await waitFor(() => expect(document.activeElement).toBe(next));
    const lastPager = screen.getByRole("navigation", { name: "Previous and next" });
    expect(within(lastPager).queryByRole("link", { name: /^Next/u })).toBeNull();

    fireEvent.click(within(lastPager).getByRole("link", { name: "Previous 1 · Preload" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Preload" })).toBeTruthy();
    fireEvent.click(within(screen.getByRole("navigation", { name: "Previous and next" })).getByRole("link", { name: "Previous Overview" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Physiology" })).toBeTruthy();
  });

  it("starts reading from the overview's link to the first section", async () => {
    mockedGetGuide.mockResolvedValue(twoSectionGuide());
    renderGuide();

    fireEvent.click(await screen.findByRole("link", { name: "Start reading 1 · Preload" }));

    expect(await screen.findByRole("heading", { level: 1, name: "Preload" })).toBeTruthy();
  });

  it("draws every block of a section page in reading order", async () => {
    mockedGetGuide.mockResolvedValue(twoSectionGuide());
    renderGuide();
    await openSection("Preload");

    const page = screen.getByRole("article", { name: "Preload" });
    const minute = within(page).getByRole("region", { name: "In one minute" });
    expect(within(minute).getByText("Preload reflects ventricular filling and stretch before systole.")).toBeTruthy();
    expect(within(minute).getByText("More filling can increase stroke volume.")).toBeTruthy();
    expect(within(page).getByRole("table", { name: "Hemodynamic comparison" })).toBeTruthy();
    expect(within(page).getByRole("columnheader", { name: "Measure" })).toBeTruthy();
    const warning = within(page).getByRole("note", { name: "Warning" });
    expect(within(warning).getByText("Do not confuse")).toBeTruthy();
    const example = within(page).getByRole("region", { name: "A fluid bolus" });
    expect(within(example).getByText("Stroke volume rises.")).toBeTruthy();
    expect(within(example).getByText("Filling sets the stretch.")).toBeTruthy();
    expect(within(page).getByText("Preload is the same as blood pressure.")).toBeTruthy();
    const headings = within(page).getAllByRole("heading", { level: 2 }).map((heading) => heading.textContent);
    expect(headings.indexOf("In one minute")).toBeLessThan(headings.indexOf("Mechanism"));
    expect(headings.indexOf("A fluid bolus")).toBeLessThan(headings.indexOf("Common misconceptions"));
    expect(headings.indexOf("Common misconceptions")).toBeLessThan(headings.indexOf("Check yourself"));
  });

  it("reveals check-yourself answers, counts them and keeps them while the guide is open", async () => {
    mockedGetGuide.mockResolvedValue(twoSectionGuide());
    renderGuide();
    await openSection("Preload");

    const check = screen.getByRole("region", { name: "Check yourself" });
    expect(within(check).getByText("0 of 2 answered")).toBeTruthy();
    expect(within(check).queryByText("More venous return raises end-diastolic volume.")).toBeNull();
    const first = required(within(check).getByText("What raises preload?").closest("li"), "first check");
    fireEvent.click(within(first).getByRole("button", { name: "Show answer" }));

    expect(await within(first).findByText("More venous return raises end-diastolic volume.")).toBeTruthy();
    await waitFor(() => expect(document.activeElement?.textContent).toContain("More venous return raises end-diastolic volume."));
    expect(within(check).getByText("1 of 2 answered")).toBeTruthy();
    const result = within(first).getByRole("radiogroup", { name: "Did you get it?" });
    fireEvent.click(within(result).getByRole("radio", { name: "Not yet" }));
    await waitFor(() => expect(within(result).getByRole("radio", { name: "Not yet" }).getAttribute("aria-checked")).toBe("true"));
    expect(within(result).getByRole("radio", { name: "Yes" }).getAttribute("aria-checked")).toBe("false");

    fireEvent.click(screen.getByRole("link", { name: "Next 2 · Afterload" }));
    await screen.findByRole("heading", { level: 1, name: "Afterload" });
    expect(screen.queryByRole("region", { name: "Check yourself" })).toBeNull();
    fireEvent.click(screen.getByRole("link", { name: "Previous 1 · Preload" }));
    const again = await screen.findByRole("region", { name: "Check yourself" });
    expect(within(again).getByText("1 of 2 answered")).toBeTruthy();
    expect(within(again).getByRole("radio", { name: "Not yet" }).getAttribute("aria-checked")).toBe("true");
    expect(within(again).getAllByRole("button", { name: "Show answer" })).toHaveLength(1);
  });

  it("practices the section's cards, and says so quietly when a section has none", async () => {
    mockedGetGuide.mockResolvedValue(twoSectionGuide());
    mockedProjection.mockImplementation(async (_documentId, topic) => ({
      topicId: topic,
      noteCount: topic === topicId ? 1 : 0,
      cardCount: topic === topicId ? 2 : 0,
      studyableCardCount: topic === topicId ? 2 : 0,
    }));
    renderGuide();
    await openSection("Preload");

    expect(await screen.findByRole("heading", { name: "2 cards come from this section" })).toBeTruthy();
    // The end of the section and the margin rail both offer the same practice.
    const practice = screen.getAllByRole("button", { name: "Practice 2 cards" });
    expect(practice).toHaveLength(2);
    fireEvent.click(required(practice[0], "practice button"));
    await waitFor(() => {
      expect(mockedCardIds).toHaveBeenCalledWith(documentId, [topicId]);
      expect(onPracticeCards).toHaveBeenCalledWith(linkedCardIds);
    });

    fireEvent.click(screen.getByRole("link", { name: "Next 2 · Afterload" }));
    expect(await screen.findByText("No flashcards come from this section yet.")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Practice/u })).toBeNull();
  });

  it("tells the learner when a section's linked cards have gone", async () => {
    mockedGetGuide.mockResolvedValue(guide());
    mockedCardIds.mockResolvedValue([]);
    const showToast = vi.fn();
    renderGuide({ showToast });
    await openSection("Preload");

    fireEvent.click(required((await screen.findAllByRole("button", { name: "Practice 2 cards" }))[0], "practice button"));

    await waitFor(() => expect(showToast).toHaveBeenCalledWith("info", "No flashcards are linked to this section yet."));
    expect(onPracticeCards).not.toHaveBeenCalled();
  });

  it("keeps the section's terms and note beside the text on wide screens", async () => {
    mockedGetGuide.mockResolvedValue({ ...guide(), overlays: [topicNote()] });
    renderGuide();
    await openSection("Preload");

    const rail = screen.getByRole("complementary", { name: "About this section" });
    expect(rail.closest("article")).toBeNull();
    const terms = within(rail).getByRole("region", { name: "Important terms" });
    expect(within(terms).getByText("End-diastolic ventricular stretch.")).toBeTruthy();
    expect(screen.getAllByText("End-diastolic ventricular stretch.")).toHaveLength(1);
    const note = within(rail).getByRole("region", { name: "Your note" });
    expect(within(note).getByText("Remember end-diastolic volume.")).toBeTruthy();
    expect(within(note).getByRole("button", { name: "Edit note" })).toBeTruthy();
    expect(within(note).getByRole("button", { name: "Remove note" })).toBeTruthy();
  });

  it("hides term meanings to test yourself, shows one at a time, and remembers the choice", async () => {
    window.localStorage.removeItem("ostudy.guideHideTermMeanings");
    mockedGetGuide.mockResolvedValue(guide());
    renderGuide();
    await openSection("Preload");

    const terms = within(screen.getByRole("complementary", { name: "About this section" })).getByRole("region", { name: "Important terms" });
    fireEvent.click(within(terms).getByRole("switch", { name: "Hide meanings" }));
    expect(within(terms).queryByText("End-diastolic ventricular stretch.")).toBeNull();
    expect(window.localStorage.getItem("ostudy.guideHideTermMeanings")).toBe("true");
    fireEvent.click(within(terms).getByRole("button", { name: "Show what Preload means" }));
    expect(within(terms).getByText("End-diastolic ventricular stretch.")).toBeTruthy();

    fireEvent.click(within(terms).getByRole("switch", { name: "Hide meanings" }));
    expect(window.localStorage.getItem("ostudy.guideHideTermMeanings")).toBe("false");
    expect(within(terms).getByText("End-diastolic ventricular stretch.")).toBeTruthy();
    window.localStorage.removeItem("ostudy.guideHideTermMeanings");
  });

  it("keeps section actions in one menu and guide actions in the top bar", async () => {
    mockedGetGuide.mockResolvedValue(guide());
    renderGuide();

    // Opening the guide in a PDF reader sits in the bar; saving a copy and the rest are in its menu.
    expect(await screen.findByRole("button", { name: "Open in PDF reader" })).toBeTruthy();
    const guideMenu = await openMenu(screen.getByRole("button", { name: "Study guide actions" }));
    expect(within(guideMenu).getAllByRole("menuitem").map((item) => item.textContent))
      .toEqual(["Save a copy as PDF…", "Add guide note", "Remake the study guide"]);
    fireEvent.keyDown(guideMenu, { key: "Escape" });

    await openSection("Preload");
    const sectionMenu = await openMenu(screen.getByRole("button", { name: "Section actions for Preload" }));
    expect(within(sectionMenu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual(["Mark as read", "Add note", "Edit or rewrite this section"]);
  });

  it("opens in a page theme as the guide's PDF pages on one canvas, with its contents beside them", async () => {
    window.localStorage.setItem("ostudy.guidePageStyle", "sage");
    vi.mocked(getDocumentStudyGuideCanvas).mockResolvedValue(guideCanvas());
    vi.mocked(readMaterialInk).mockResolvedValue({ marks: [], unavailableLinkIds: [] });
    Object.defineProperty(URL, "createObjectURL", { configurable: true, writable: true, value: vi.fn(() => "blob:page") });
    Object.defineProperty(URL, "revokeObjectURL", { configurable: true, writable: true, value: vi.fn() });
    mockedGetGuide.mockResolvedValue(twoSectionGuide());
    renderGuide();

    const contents = await screen.findByRole("navigation", { name: "Study guide contents" });
    expect(getDocumentStudyGuideCanvas).toHaveBeenCalledWith(documentId, "sage");
    expect(readMaterialInk).toHaveBeenCalledWith({ kind: "studyGuide", documentId });
    expect(within(contents).getAllByRole("button").map((button) => button.textContent?.trim()).filter(Boolean)).toEqual([
      "Cover and contents", "1. Loads", "1.1Preload", "1.2Afterload", "Cram sheet", "Self-test",
    ]);
    expect(document.querySelectorAll("[data-annotator-page]")).toHaveLength(4);
    expect(screen.getByRole("button", { name: "Add link" })).toBeTruthy();

    // Each section's Practice button starts Practice on its linked cards.
    fireEvent.click(await screen.findByRole("button", { name: "Practice flashcards for 1.1 Preload" }));
    await waitFor(() => expect(onPracticeCards).toHaveBeenCalledWith(linkedCardIds));
    expect(mockedCardIds).toHaveBeenCalledWith(documentId, [topicId]);

    // A section's own actions sit in its contents row.
    const menu = await openMenu(within(contents).getByRole("button", { name: "1.2 Afterload actions" }));
    expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent))
      .toEqual(["Practice this section's flashcards", "Mark as read", "Edit or rewrite this section"]);
  });

  it("opens the guide in the system's PDF reader in the chosen theme", async () => {
    window.localStorage.setItem("ostudy.guidePageStyle", "plum");
    vi.mocked(getDocumentStudyGuideCanvas).mockResolvedValue(guideCanvas());
    vi.mocked(readMaterialInk).mockResolvedValue({ marks: [], unavailableLinkIds: [] });
    vi.mocked(openDocumentStudyGuidePdf).mockResolvedValue(undefined);
    Object.defineProperty(URL, "createObjectURL", { configurable: true, writable: true, value: vi.fn(() => "blob:page") });
    Object.defineProperty(URL, "revokeObjectURL", { configurable: true, writable: true, value: vi.fn() });
    mockedGetGuide.mockResolvedValue(guide());
    renderGuide();

    fireEvent.click(await screen.findByRole("button", { name: "Open in PDF reader" }));
    await waitFor(() => expect(openDocumentStudyGuidePdf).toHaveBeenCalledWith(documentId, "plum"));
  });

  it("offers the plain text when the pages cannot be laid out", async () => {
    window.localStorage.setItem("ostudy.guidePageStyle", "navy");
    vi.mocked(getDocumentStudyGuideCanvas).mockRejectedValue(new Error("no fonts"));
    mockedGetGuide.mockResolvedValue(guide());
    renderGuide();

    expect(await screen.findByText("The study guide's pages could not be laid out: no fonts")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Read as plain text" }));
    expect(await screen.findByRole("article", { name: "Physiology" })).toBeTruthy();
    expect(window.localStorage.getItem("ostudy.guidePageStyle")).toBe("plain");
  });

  it("reopens on the section last opened and records each section opened", async () => {
    readingStore({ lastTopicId: afterloadTopicId, readTopicIds: [topicId] });
    mockedGetGuide.mockResolvedValue(twoSectionGuide());
    renderGuide();

    expect(await screen.findByRole("heading", { level: 1, name: "Afterload" })).toBeTruthy();
    await waitFor(() => expect(mockedOpenSection).toHaveBeenCalledWith(documentId, guideId, afterloadTopicId));
    const contents = screen.getByRole("navigation", { name: "Study guide contents" });
    expect(within(contents).getByRole("link", { name: "Preload, read" })).toBeTruthy();
    expect(screen.getByRole("progressbar", { name: "Sections read" }).getAttribute("aria-valuetext")).toBe("1 of 2 sections read");

    fireEvent.click(within(contents).getByRole("link", { name: "Preload, read" }));
    await screen.findByRole("heading", { level: 1, name: "Preload" });
    await waitFor(() => expect(mockedOpenSection).toHaveBeenLastCalledWith(documentId, guideId, topicId));
  });

  it("counts a section read on moving on, and the last one from its own button", async () => {
    mockedGetGuide.mockResolvedValue(twoSectionGuide());
    renderGuide();

    await openSection("Preload");
    fireEvent.click(within(screen.getByRole("navigation", { name: "Previous and next" })).getByRole("link", { name: "Next 2 · Afterload" }));
    await screen.findByRole("heading", { level: 1, name: "Afterload" });
    await waitFor(() => expect(mockedSetRead).toHaveBeenCalledWith(documentId, guideId, topicId, true));
    const contents = screen.getByRole("navigation", { name: "Study guide contents" });
    expect(await within(contents).findByRole("link", { name: "Preload, read" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Mark as read" }));
    await waitFor(() => expect(mockedSetRead).toHaveBeenLastCalledWith(documentId, guideId, afterloadTopicId, true));
    expect(await screen.findByText("Read")).toBeTruthy();
    await waitFor(() => expect(screen.getByRole("progressbar", { name: "Sections read" }).getAttribute("aria-valuetext")).toBe("2 of 2 sections read"));

    await chooseSectionAction("Mark as unread");
    await waitFor(() => expect(mockedSetRead).toHaveBeenLastCalledWith(documentId, guideId, afterloadTopicId, false));
    expect(await screen.findByRole("button", { name: "Mark as read" })).toBeTruthy();
  });

  it("keeps reading when the reading store cannot be reached", async () => {
    mockedGetReading.mockRejectedValue(new Error("offline"));
    mockedOpenSection.mockRejectedValue(new Error("offline"));
    mockedGetGuide.mockResolvedValue(twoSectionGuide());
    renderGuide();

    expect(await screen.findByRole("heading", { level: 1, name: "Physiology" })).toBeTruthy();
    expect(await openSection("Preload")).toBeTruthy();
  });

  it("saves a topic note through the note dialog, independently of the guide revision", async () => {
    const saved = { ...topicNote(), plainText: "Remember the relationship to end-diastolic volume." };
    mockedGetGuide
      .mockResolvedValueOnce(guide())
      .mockResolvedValueOnce({ ...guide(), overlays: [saved] });
    mockedSaveOverlay.mockResolvedValue(saved);
    const showToast = vi.fn();
    renderGuide({ showToast });
    await openSection("Preload");

    const note = within(screen.getByRole("complementary", { name: "About this section" })).getByRole("region", { name: "Your note" });
    fireEvent.click(within(note).getByRole("button", { name: "Add note" }));
    const dialog = await screen.findByRole("dialog", { name: "Add a note" });
    const field = within(dialog).getByRole("textbox", { name: "Note" });
    await waitFor(() => expect(document.activeElement).toBe(field));
    fireEvent.change(field, { target: { value: "Remember the relationship to end-diastolic volume." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));

    await waitFor(() =>
      expect(mockedSaveOverlay).toHaveBeenCalledWith(
        expect.objectContaining({
          documentId,
          topicId,
          overlayKind: "topic_note",
          guideId: undefined,
          overlayId: undefined,
        }),
      ),
    );
    expect(await within(note).findByText("Remember the relationship to end-diastolic volume.")).toBeTruthy();
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add a note" })).toBeNull());
    expect(showToast).toHaveBeenCalledWith("success", "Note saved.");
  });

  it("removes an existing topic note through the discard overlay boundary", async () => {
    const withNote = { ...guide(), overlays: [topicNote()] };
    mockedGetGuide.mockResolvedValueOnce(withNote).mockResolvedValueOnce(guide());
    const showToast = vi.fn();
    renderGuide({ showToast });
    await openSection("Preload");

    expect(screen.getByText("Remember end-diastolic volume.")).toBeTruthy();
    await chooseSectionAction("Remove note");

    await waitFor(() => {
      expect(mockedDiscardOverlay).toHaveBeenCalledWith("9".repeat(32), 3);
      expect(showToast).toHaveBeenCalledWith("success", "Topic note removed.");
    });
    await waitFor(() => expect(screen.queryByText("Remember end-diastolic volume.")).toBeNull());
  });

  it("treats clearing an existing note as removal instead of invalid empty rich content", async () => {
    const withNote = { ...guide(), overlays: [topicNote()] };
    mockedGetGuide.mockResolvedValueOnce(withNote).mockResolvedValueOnce(guide());
    const showToast = vi.fn();
    renderGuide({ showToast });
    await openSection("Preload");

    await chooseSectionAction("Edit note");
    const dialog = await screen.findByRole("dialog", { name: "Edit note" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Note" }), { target: { value: "   " } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));

    await waitFor(() => expect(mockedDiscardOverlay).toHaveBeenCalledWith("9".repeat(32), 3));
    expect(mockedSaveOverlay).not.toHaveBeenCalled();
    expect(showToast).toHaveBeenCalledWith("success", "Note removed.");
  });

  it("rewrites a section without losing the generated text and resets it", async () => {
    const override = sectionOverride();
    mockedGetGuide
      .mockResolvedValueOnce(guide())
      .mockResolvedValueOnce({ ...guide(), overlays: [override] })
      .mockResolvedValueOnce(guide());
    mockedSaveOverlay.mockResolvedValue(override);
    const showToast = vi.fn();
    renderGuide({ showToast });
    await openSection("Preload");

    await chooseSectionAction("Edit or rewrite this section");
    const dialog = await screen.findByRole("dialog", { name: "Edit or rewrite this section" });
    const field = within(dialog).getByRole("textbox", { name: "Section text" }) as HTMLTextAreaElement;
    expect(field.value).toBe("Preload reflects ventricular filling and stretch before systole.");
    fireEvent.change(field, { target: { value: "In my words: filling stretches the muscle." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));

    await waitFor(() => expect(mockedSaveOverlay).toHaveBeenCalledWith(expect.objectContaining({
      documentId,
      topicId,
      guideId,
      overlayKind: "body_override",
      baseContentHash: "f".repeat(64),
      plainText: "In my words: filling stretches the muscle.",
    })));
    // The editor's own text box holds the new words until the saved guide comes back and closes it.
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Edit or rewrite this section" })).toBeNull());
    expect(screen.getByText("In my words: filling stretches the muscle.")).toBeTruthy();
    expect(screen.getByText("Edited by you")).toBeTruthy();
    expect(screen.queryByText("Sarcomere stretch changes the force of contraction.")).toBeNull();
    const contents = screen.getByRole("navigation", { name: "Study guide contents" });
    expect(within(contents).queryByRole("list", { name: "In this section" })).toBeNull();
    expect(showToast).toHaveBeenCalledWith("success", "Section edit saved.");

    await chooseSectionAction("Reset section edit");
    await waitFor(() => expect(mockedDiscardOverlay).toHaveBeenCalledWith("5".repeat(32), 1));
    expect(showToast).toHaveBeenCalledWith("success", "Section edit reset.");
    expect(await screen.findByText("Sarcomere stretch changes the force of contraction.")).toBeTruthy();
  });

  it("shows preserved conflicted section text and lets the learner discard it", async () => {
    const conflict = conflictedOverride();
    mockedGetGuide
      .mockResolvedValueOnce({ ...guide(), overlays: [conflict] })
      .mockResolvedValueOnce(guide());
    const showToast = vi.fn();
    renderGuide({ showToast });
    await openSection("Preload");

    expect(screen.getByText("Older section edit needs review.")).toBeTruthy();
    expect(screen.getByText("My older preload explanation.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Discard older edit" }));

    await waitFor(() => {
      expect(mockedDiscardOverlay).toHaveBeenCalledWith("6".repeat(32), 4);
      expect(showToast).toHaveBeenCalledWith("success", "Older section edit discarded.");
    });
  });

  it("adds a guide note on the overview and removes it from the top bar menu", async () => {
    const note = guideNote();
    mockedGetGuide
      .mockResolvedValueOnce(guide())
      .mockResolvedValueOnce({ ...guide(), overlays: [note] })
      .mockResolvedValueOnce(guide());
    mockedSaveOverlay.mockResolvedValue(note);
    const showToast = vi.fn();
    renderGuide({ showToast });

    const about = await screen.findByRole("complementary", { name: "About this guide" });
    fireEvent.click(within(about).getByRole("button", { name: "Add note" }));
    const dialog = await screen.findByRole("dialog", { name: "Add a guide note" });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Note" }), { target: { value: "Focus on the pressure-volume loop." } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Save" }));

    await waitFor(() => expect(mockedSaveOverlay).toHaveBeenCalledWith(expect.objectContaining({
      documentId,
      overlayKind: "guide_note",
      topicId: undefined,
      guideId: undefined,
    })));
    expect(await within(about).findByText("Focus on the pressure-volume loop.")).toBeTruthy();

    await chooseGuideAction("Remove guide note");
    await waitFor(() => expect(mockedDiscardOverlay).toHaveBeenCalledWith("4".repeat(32), 2));
    expect(showToast).toHaveBeenCalledWith("success", "Guide note removed.");
  });

  it("groups sections under editorial chapters with their editorial and overridden titles", async () => {
    const value = editorialGuide();
    value.overlays = [{
      ...topicNote(),
      id: "2".repeat(32),
      guideId,
      topicId: secondTopicId,
      overlayKind: "title_override",
      plainText: "RAAS in brief",
    }];
    mockedGetGuide.mockResolvedValue(value);
    renderGuide();

    const overview = await screen.findByRole("article", { name: "Cardiovascular drugs" });
    expect(screen.getByRole("region", { name: "Cardiovascular drugs" })).toBeTruthy();
    expect(within(overview).getByText("1 chapter · 2 sections · about 2 minutes")).toBeTruthy();
    expect(within(overview).getByText(/Cardiovascular pharmacology connects drug mechanisms/iu)).toBeTruthy();
    const chapter = within(overview).getByRole("region", { name: "Hemodynamic foundations" });
    expect(within(chapter).getByText(/physiologic relationships needed to understand/iu)).toBeTruthy();
    expect(within(chapter).getAllByRole("link").map((link) => link.textContent)).toEqual([
      "1 · Preload and cardiac output", "2 · RAAS in brief",
    ]);

    const contents = screen.getByRole("navigation", { name: "Study guide contents" });
    const group = within(contents).getByRole("list", { name: "Hemodynamic foundations" });
    expect(within(group).getAllByRole("link")).toHaveLength(2);
    expect(within(group).getByRole("link", { name: "Preload and cardiac output" })).toBeTruthy();
    expect(within(group).getByRole("link", { name: "RAAS in brief" })).toBeTruthy();
    expect(screen.queryByText(/preload \/ cardiac output/iu)).toBeNull();
    expect(screen.queryByText(/raas \/ sodium balance/iu)).toBeNull();

    await openSection("RAAS in brief");
    expect(screen.getByText("Section 2 of 2 · about 1 minute")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Key points" })).toBeNull();
  });

  it.each([true, false])("exports the whole guide as a PDF with includeNotes=%s", async (includeNotes) => {
    mockedGetGuide.mockResolvedValue(guide());
    mockedPreparePdf.mockResolvedValue(preparedPdf());
    mockedPublish.mockResolvedValue({
      preparedExportId: "8".repeat(32),
      displayName: "Physiology-study-guide.pdf",
      destinationKind: "file",
      completedAt: 123,
    });
    const showToast = vi.fn();
    vi.mocked(getSettings).mockResolvedValue({ guidePdfTheme: "plum", guidePdfIncludeNotes: includeNotes } as AppSettings);
    renderGuide({ showToast });

    await chooseGuideAction("Save a copy as PDF…");

    await waitFor(() => {
      expect(mockedPreparePdf).toHaveBeenCalledWith(documentId, includeNotes, "plum");
      expect(mockedPublish).toHaveBeenCalledWith("8".repeat(32));
      expect(mockedRelease).toHaveBeenCalledWith("8".repeat(32));
    });
    expect(showToast).toHaveBeenCalledWith(
      "success",
      "Exported study guide with 1 section.",
    );
    expect(screen.queryByRole("dialog", { name: "Export as PDF" })).toBeNull();
  });

  it("exports in Sage with notes when Settings has no PDF choices yet", async () => {
    mockedGetGuide.mockResolvedValue(guide());
    mockedPreparePdf.mockResolvedValue(preparedPdf());
    mockedPublish.mockResolvedValue(null);
    renderGuide();

    await chooseGuideAction("Save a copy as PDF…");

    await waitFor(() => expect(mockedPreparePdf).toHaveBeenCalledWith(documentId, true, "sage"));
  });

  it("reports a PDF that could not be prepared and publishes nothing", async () => {
    mockedGetGuide.mockResolvedValue(guide());
    mockedPreparePdf.mockRejectedValue(new Error("The PDF could not be prepared."));
    const showToast = vi.fn();
    renderGuide({ showToast });

    await chooseGuideAction("Save a copy as PDF…");

    await waitFor(() => expect(showToast).toHaveBeenCalledWith("error", "The PDF could not be prepared."));
    expect(mockedPublish).not.toHaveBeenCalled();
  });

  it.each([true, false])("discloses PDF math and releases the prepared file when accepted=%s", async (accepted) => {
    const value = guide();
    value.overview.summary = String.raw`Solve \(x^2\).`;
    mockedGetGuide.mockResolvedValue(value);
    mockedPreparePdf.mockResolvedValue(preparedPdf({ mathSourceFallback: true }));
    mockedPublish.mockResolvedValue({ preparedExportId: "8".repeat(32), displayName: "math.pdf", destinationKind: "file", completedAt: 123 });
    const showToast = vi.fn();
    renderGuide({ showToast });
    await chooseGuideAction("Save a copy as PDF…");
    const confirmation = await screen.findByRole("alertdialog", { name: "Export math as source notation?" });
    expect(mockedPublish).not.toHaveBeenCalled();
    expect(within(confirmation).getByText(/outside the native PDF math subset.*readable LaTeX source/iu)).toBeTruthy();
    fireEvent.click(within(confirmation).getByRole("button", { name: accepted ? "Export PDF" : "Cancel" }));
    await waitFor(() => expect(mockedRelease).toHaveBeenCalledWith("8".repeat(32)));
    expect(mockedPublish).toHaveBeenCalledTimes(accepted ? 1 : 0);
    expect(mockedSaveOverlay).not.toHaveBeenCalled();
    expect(mockedGenerate).not.toHaveBeenCalled();
    if (!accepted) expect(showToast).not.toHaveBeenCalled();
  });

  it("asks the tutor about the whole guide or the section being read", async () => {
    mockedGetGuide.mockResolvedValue(guide());
    const openContext = vi.fn();
    renderGuide({ tutor: { busy: false, pending: null, openContext } as unknown as TutorController });

    const ask = await screen.findByRole("button", { name: "Ask tutor about this material" });
    // Wide screens name the action in the top bar, as the mockups do.
    expect(ask.textContent).toBe("Ask Tutor");
    fireEvent.click(ask);
    expect(openContext).toHaveBeenCalledWith({
      title: "Physiology",
      scope: { kind: "guide", id: documentId },
      focus: { kind: "guide", documentId, guideId, topicId: null, contentHash: null },
      mode: "explain",
    });

    await openSection("Preload");
    await chooseSectionAction("Ask Tutor about this section");
    expect(openContext).toHaveBeenLastCalledWith({
      title: "Preload",
      scope: { kind: "guide", id: documentId },
      focus: { kind: "guide", documentId, guideId, topicId, contentHash: "f".repeat(64) },
      mode: "explain",
    });
  });

  it("opens the outline in a Contents sheet on medium screens and puts the rail below the text", async () => {
    mockedGetGuide.mockResolvedValue(twoSectionGuide());
    const onClose = vi.fn();
    renderGuide({ layout: "medium", onClose });

    await screen.findByRole("heading", { level: 1, name: "Physiology" });
    expect(screen.queryByRole("navigation", { name: "Study guide contents" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Contents" }));
    const sheet = await screen.findByRole("dialog", { name: "Contents" });
    // Escape closes the sheet, not the reader behind it.
    fireEvent.keyDown(sheet, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Contents" })).toBeNull());
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Contents" }));
    const reopened = await screen.findByRole("dialog", { name: "Contents" });
    const outline = within(reopened).getByRole("navigation", { name: "Study guide contents" });
    fireEvent.click(within(outline).getByRole("link", { name: "Afterload" }));
    const heading = await screen.findByRole("heading", { level: 1, name: "Afterload" });
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Contents" })).toBeNull());
    await waitFor(() => expect(document.activeElement).toBe(heading));
    const page = screen.getByRole("article", { name: "Afterload" });
    expect(within(page).getByRole("complementary", { name: "About this section" })).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "Section actions" })).toBeNull();
  });

  it("gives phones a bottom bar with the neighbors and the section's practice", async () => {
    mockedGetGuide.mockResolvedValue(twoSectionGuide());
    renderGuide({ layout: "compact" });

    fireEvent.click(await screen.findByRole("link", { name: "Start reading 1 · Preload" }));
    await screen.findByRole("heading", { level: 1, name: "Preload" });
    const bar = screen.getByRole("navigation", { name: "Section actions" });
    expect(within(bar).getByRole("button", { name: "Previous: Overview" })).toBeTruthy();
    fireEvent.click(await within(bar).findByRole("button", { name: "Practice 2 cards" }));
    await waitFor(() => expect(onPracticeCards).toHaveBeenCalledWith(linkedCardIds));

    fireEvent.click(within(bar).getByRole("button", { name: "Next section: Afterload" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Afterload" })).toBeTruthy();
    const lastBar = screen.getByRole("navigation", { name: "Section actions" });
    fireEvent.click(within(lastBar).getByRole("button", { name: "Previous section: Preload" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Preload" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Contents" })).toBeTruthy();
  });

  it("lets the phone bar give way while the end of the section is on screen", async () => {
    const observers: { callback: IntersectionObserverCallback; targets: Element[]; instance: IntersectionObserver }[] = [];
    vi.stubGlobal("IntersectionObserver", class implements IntersectionObserver {
      readonly root = null;
      readonly rootMargin = "0px";
      readonly thresholds = [0];
      private readonly record: (typeof observers)[number];
      constructor(callback: IntersectionObserverCallback) {
        this.record = { callback, targets: [], instance: this };
        observers.push(this.record);
      }
      observe(target: Element): void { this.record.targets.push(target); }
      unobserve(): void { /* The reader only disconnects. */ }
      disconnect(): void { this.record.targets = []; }
      takeRecords(): IntersectionObserverEntry[] { return []; }
    });
    try {
      mockedGetGuide.mockResolvedValue(twoSectionGuide());
      renderGuide({ layout: "compact" });
      fireEvent.click(await screen.findByRole("link", { name: "Start reading 1 · Preload" }));
      await screen.findByRole("heading", { level: 1, name: "Preload" });
      expect(screen.getByRole("navigation", { name: "Section actions" })).toBeTruthy();

      // The watched end holds the section's own practice and its previous and next links.
      const watching = required(observers.find((observer) => observer.targets.length > 0), "section end observer");
      const end = required(watching.targets[0], "section end");
      expect(within(end as HTMLElement).getByRole("navigation", { name: "Previous and next" })).toBeTruthy();
      expect(await within(end as HTMLElement).findByRole("button", { name: "Practice 2 cards" })).toBeTruthy();
      const report = (isIntersecting: boolean) => act(() => watching.callback(
        [{ target: end, isIntersecting } as IntersectionObserverEntry], watching.instance));

      report(true);
      expect(screen.queryByRole("navigation", { name: "Section actions" })).toBeNull();
      report(false);
      expect(screen.getByRole("navigation", { name: "Section actions" })).toBeTruthy();
    } finally {
      vi.unstubAllGlobals();
    }
  });
});

describe("Study-guide mathematical presentation", () => {
  const expectedAnnotations = Array.from({ length: 32 }, (_, index) => index < 9 ? `x_${index + 1}` : `x_{${index + 1}}`);

  it("renders every structured body field without mutating source text, hashes or locators", async () => {
    const loaded = guide();
    loaded.overview.summary = String.raw`Summary \(x_1\)`;
    loaded.overview.learningObjectives = [String.raw`Find \(x_2\)`];
    loaded.overview.keyConcepts = [String.raw`Know \(x_3\)`];
    const section = required(loaded.sections[0], "generated section");
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
    const annotations = () => Array.from(container.querySelectorAll("math annotation"), (node) => node.textContent ?? "");

    await waitFor(() => expect(annotations()).toEqual(expect.arrayContaining(["x_1", "x_2", "x_3", "x_4"])));
    expect(container.querySelector("p div,a a")).toBeNull();
    const seen = new Set(annotations());

    // Names are not computed here: jsdom cannot style the MathML inside these links.
    fireEvent.click(required(screen.getByText("Start reading").closest("a"), "start reading link"));
    fireEvent.click(await screen.findByText("Show answer"));
    await waitFor(() => expect(annotations()).toContain("x_{32}"));
    for (const annotation of annotations()) seen.add(annotation);

    expect([...seen].sort()).toEqual([...expectedAnnotations].sort());
    expect(screen.getAllByText(/pp\. 2–3/u).length).toBeGreaterThan(0);
    expect(container.querySelector("p div,a a")).toBeNull();
    expect(JSON.stringify(loaded)).toBe(before);
    expect(mockedSaveOverlay).not.toHaveBeenCalled();
    expect(mockedGenerate).not.toHaveBeenCalled();
  });

  it("shows active and conflicted note formulas and saves original LaTeX instead of renderer DOM", async () => {
    const source = String.raw`My note \(\frac{a}{b}\)`;
    const note = { ...topicNote(), plainText: source };
    const conflict = { ...conflictedOverride(), plainText: String.raw`Older edit \(x^2\)` };
    const loaded = { ...guide(), overlays: [note, conflict] };
    mockedGetGuide.mockResolvedValue(loaded);
    mockedSaveOverlay.mockResolvedValue(note);
    const { container } = renderGuide();
    await openSection("Preload");
    await waitFor(() => expect(container.querySelectorAll("math annotation")).toHaveLength(2));
    expect(screen.getByText("Older section edit needs review.")).toBeTruthy();

    await chooseSectionAction("Edit note");
    const dialog = await screen.findByRole("dialog", { name: "Edit note" });
    expect((within(dialog).getByRole("textbox", { name: "Note" }) as HTMLTextAreaElement).value).toBe(source);
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Edit note" })).toBeNull());
    expect(mockedSaveOverlay).not.toHaveBeenCalled();

    await chooseSectionAction("Edit note");
    const edited = source + String.raw` and \(c\)`;
    const reopened = await screen.findByRole("dialog", { name: "Edit note" });
    fireEvent.change(within(reopened).getByRole("textbox", { name: "Note" }), { target: { value: edited } });
    fireEvent.click(within(reopened).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(mockedSaveOverlay).toHaveBeenCalledWith(expect.objectContaining({
      overlayId: note.id, overlayKind: "topic_note", plainText: edited, expectedRevision: 3,
      richDoc: { type: "doc", content: [{ type: "paragraph", content: [{ type: "text", text: edited }] }] },
    })));
    expect(JSON.stringify(mockedSaveOverlay.mock.calls)).not.toContain("MathML");
  });

  it("renders fresh generated content and guide notes through the same offline reader", async () => {
    const generated = guide();
    required(generated.sections[0], "generated section").content.overview = String.raw`Generated \(x+y\)`;
    generated.overlays = [{ ...guideNote(), plainText: String.raw`Note \(z\)` }];
    mockedGetGuide.mockResolvedValue(null);
    mockedGenerate.mockResolvedValue(generated);
    const { container } = renderGuide();
    const annotations = () => Array.from(container.querySelectorAll("math annotation"), (node) => node.textContent ?? "");

    fireEvent.click(await screen.findByRole("button", { name: "Generate study guide" }));
    await waitFor(() => expect(annotations()).toEqual(["z"]));
    await openSection("Preload");
    await waitFor(() => expect(annotations()).toEqual(["x+y"]));
    expect(mockedGenerate).toHaveBeenCalledTimes(1);
    expect(mockedSaveOverlay).not.toHaveBeenCalled();
  });
});

/** The two-section guide laid out on four pages: the cover, one chapter, the cram sheet and the self-test. */
function guideCanvas(): StudyGuideCanvas {
  const page = (key: string) => ({ key, width: 538.58, height: 700, svg: '<svg viewBox="0 0 538.58 700" xmlns="http://www.w3.org/2000/svg"/>' });
  return {
    documentId,
    guideRevision: 1,
    theme: "sage",
    pages: [page("front:1"), page("chapter:loads:1"), page("cram:1"), page("test:1")],
    contents: [
      { kind: "chapter", key: "loads", title: "Loads", number: "1", pageIndex: 1, y: 40 },
      { kind: "section", key: topicId, title: "Preload", number: "1.1", pageIndex: 1, y: 120 },
      { kind: "section", key: afterloadTopicId, title: "Afterload", number: "1.2", pageIndex: 1, y: 420 },
      { kind: "cram", key: "cram", title: "Cram sheet", number: "", pageIndex: 2, y: 40 },
      { kind: "selfTest", key: "selfTest", title: "Self-test", number: "", pageIndex: 3, y: 40 },
    ],
    practice: [
      { topicId, pageIndex: 1, x: 450, y: 118, width: 68, height: 18.4 },
      { topicId: afterloadTopicId, pageIndex: 1, x: 450, y: 418, width: 68, height: 18.4 },
    ],
  };
}

function setLayout(layout: ViewportLayout): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: matchesLayoutQuery(query, layoutWidths[layout]),
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

function renderGuide({
  showToast = vi.fn() as ShowToast,
  onClose = vi.fn() as () => void,
  source = null,
  layout,
  tutor = null,
}: {
  showToast?: ShowToast;
  onClose?: () => void;
  source?: GuideSourceSummary | null;
  /** Renders inside the layout provider at a simulated viewport; omitted, the reader is wide. */
  layout?: ViewportLayout;
  tutor?: TutorController | null;
} = {}) {
  let reader = (
    <OverlayProvider><StudyGuideDialog
      documentId={documentId}
      documentTitle="Physiology"
      source={source}
      showToast={showToast}
      onPracticeCards={onPracticeCards}
      onClose={onClose}
    /></OverlayProvider>
  );
  if (tutor) reader = <TutorContext.Provider value={tutor}>{reader}</TutorContext.Provider>;
  if (!layout) return render(reader);
  setLayout(layout);
  return render(<LayoutModeProvider>{reader}</LayoutModeProvider>);
}
