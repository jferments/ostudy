import { describe, expect, it } from "vitest";

import type { DocumentStudyGuide, DocumentStudyGuideSection, StudyGuideOverlay, StudyGuideSectionContent } from "../../ipc/studyGuides";
import {
  fileKindLabel,
  findConflictedOverride,
  findEditingOverlay,
  formatLocatorRange,
  formatMinutes,
  formatSourceLength,
  guideReadingOrder,
  plainTextRichDocument,
  readingMinutes,
  sectionBlocks,
  splitOverviewConcept,
} from "./guideModel";

const guideId = "c".repeat(32);

function content(overrides: Partial<StudyGuideSectionContent> = {}): StudyGuideSectionContent {
  return {
    overview: "",
    keyPoints: [],
    explanations: [],
    importantTerms: [],
    processSteps: [],
    comparisons: [],
    commonMisconceptions: [],
    callouts: [],
    tables: [],
    workedExamples: [],
    retrievalChecks: [],
    visualExplanations: [],
    ...overrides,
  };
}

function section(topicId: string, title: string): DocumentStudyGuideSection {
  return {
    topicId,
    parentTopicId: null,
    sortIndex: 0,
    title,
    topicSummary: "",
    content: content({ overview: `${title} overview.` }),
    contentText: `${title} overview.`,
    contentHash: "f".repeat(64),
    sourceLocator: { unitKind: "page", unitNumbers: [1], assetIds: [] },
  };
}

function overlay(overrides: Partial<StudyGuideOverlay>): StudyGuideOverlay {
  return {
    id: "9".repeat(32),
    documentId: "a".repeat(32),
    guideId: null,
    topicId: null,
    overlayKind: "guide_note",
    baseContentHash: null,
    richDoc: {},
    plainText: "",
    state: "active",
    revision: 1,
    createdAt: 1,
    updatedAt: 1,
    ...overrides,
  };
}

function guide(sections: DocumentStudyGuideSection[], overrides: Partial<DocumentStudyGuide> = {}): DocumentStudyGuide {
  return {
    id: guideId,
    documentId: "a".repeat(32),
    planId: "d".repeat(32),
    jobId: "e".repeat(32),
    revision: 1,
    schemaVersion: 1,
    promptVersion: 1,
    assemblyVersion: 1,
    overview: { summary: "", learningObjectives: [], keyConcepts: [] },
    overviewText: "",
    sections,
    overlays: [],
    createdAt: 1,
    updatedAt: 1,
    publishedAt: 1,
    ...overrides,
  };
}

describe("guide reading order", () => {
  it("numbers sections in generated order when the guide has no chapters", () => {
    const order = guideReadingOrder(guide([section("1".repeat(32), "Preload"), section("2".repeat(32), "Afterload")]));
    expect(order.entries.map((entry) => [entry.number, entry.title])).toEqual([[1, "Preload"], [2, "Afterload"]]);
    expect(order.groups).toHaveLength(1);
    expect(order.groups[0]?.title).toBeNull();
  });

  it("follows editorial chapters, uses their titles and keeps an unplaced section reachable", () => {
    const first = section("1".repeat(32), "preload / venous return");
    const second = section("2".repeat(32), "raas / sodium");
    const stray = section("3".repeat(32), "Stray topic");
    const order = guideReadingOrder(guide([first, second, stray], {
      editorial: {
        title: "Cardiovascular drugs",
        chapters: [{
          id: "6".repeat(32),
          title: "Foundations",
          introduction: "Start here.",
          topics: [
            { topicId: second.topicId, title: "Renin–angiotensin system" },
            { topicId: first.topicId, title: "Preload" },
          ],
        }],
      },
    }));
    expect(order.entries.map((entry) => [entry.number, entry.title])).toEqual([
      [1, "Renin–angiotensin system"],
      [2, "Preload"],
      [3, "Stray topic"],
    ]);
    expect(order.groups.map((group) => [group.title, group.entries.length])).toEqual([["Foundations", 2], [null, 1]]);
  });

  it("lets an active title override for this guide revision win", () => {
    const first = section("1".repeat(32), "Preload");
    const renamed = guide([first], {
      overlays: [
        overlay({ topicId: first.topicId, overlayKind: "title_override", guideId, plainText: "Filling pressure" }),
        overlay({ id: "8".repeat(32), topicId: first.topicId, overlayKind: "title_override", guideId: "5".repeat(32), plainText: "Old revision" }),
      ],
    });
    expect(guideReadingOrder(renamed).entries[0]?.title).toBe("Filling pressure");
    const stale = guide([first], {
      overlays: [overlay({ topicId: first.topicId, overlayKind: "title_override", guideId: "5".repeat(32), plainText: "Old revision" })],
    });
    expect(guideReadingOrder(stale).entries[0]?.title).toBe("Preload");
  });
});

describe("guide labels", () => {
  it("reads at about 200 words a minute and never says less than a minute", () => {
    expect(readingMinutes("")).toBe(1);
    expect(readingMinutes("word ".repeat(120))).toBe(1);
    expect(readingMinutes("word ".repeat(500))).toBe(3);
    expect(formatMinutes(1)).toBe("about 1 minute");
    expect(formatMinutes(7)).toBe("about 7 minutes");
  });

  it("names page and slide ranges from the source locator", () => {
    expect(formatLocatorRange({ unitKind: "page", unitNumbers: [4] })).toBe("p. 4");
    expect(formatLocatorRange({ unitKind: "page", unitNumbers: [8, 6, 7] })).toBe("pp. 6–8");
    expect(formatLocatorRange({ unitKind: "slide", unitNumbers: [14, 15, 16, 17, 18, 19, 22] })).toBe("slides 14–19, 22");
    expect(formatLocatorRange({ unitKind: "time_segment", unitNumbers: [2] })).toBe("part 2");
    expect(formatLocatorRange({ unitKind: "chapter", unitNumbers: [], sectionLabel: "Introduction" })).toBe("Introduction");
    expect(formatLocatorRange({ unitKind: "page", unitNumbers: [] })).toBeNull();
  });

  it("describes the source document's length and file kind", () => {
    expect(formatSourceLength({ format: "pdf", filename: "lecture.pdf", unitKind: "page", unitCount: 12 })).toBe("12 pages");
    expect(formatSourceLength({ format: "pptx", filename: "deck.pptx", unitKind: "slide", unitCount: 1 })).toBe("1 slide");
    expect(formatSourceLength({ format: "pdf", filename: null, unitKind: null, unitCount: null })).toBeNull();
    expect(fileKindLabel({ format: "pdf", filename: "Lecture 4.PDF", unitKind: null, unitCount: null })).toBe("PDF");
    expect(fileKindLabel({ format: "pptx", filename: "deck.pptx", unitKind: null, unitCount: null })).toBe("PPTX");
    expect(fileKindLabel({ format: "plain_text", filename: "notes", unitKind: null, unitCount: null })).toBe("TXT");
    expect(fileKindLabel({ format: null, filename: "scan.2024", unitKind: null, unitCount: null })).toBeNull();
    expect(fileKindLabel(null)).toBeNull();
  });

  it("splits a key concept into its term and explanation", () => {
    expect(splitOverviewConcept("Frank-Starling mechanism — Filling raises stroke volume.")).toEqual(["Frank-Starling mechanism", "Filling raises stroke volume."]);
    expect(splitOverviewConcept("Preload")).toEqual(["Preload", ""]);
  });
});

describe("section blocks", () => {
  it("lists the headed blocks in page order for the outline", () => {
    const blocks = sectionBlocks(content({
      overview: "Summary.",
      explanations: [{ heading: "Mechanism", body: "Body." }, { heading: "Regulation", body: "Body." }],
      processSteps: ["Step"],
      workedExamples: [
        { title: "One", prompt: "", steps: [], answer: "", takeaway: "" },
        { title: "Two", prompt: "", steps: [], answer: "", takeaway: "" },
      ],
      commonMisconceptions: ["Myth"],
      retrievalChecks: [{ prompt: "Why?", answer: "Because." }],
    }));
    expect(blocks.map((block) => block.label)).toEqual([
      "In one minute", "Mechanism", "Regulation", "Process", "Worked example 1", "Worked example 2", "Common misconceptions", "Check yourself",
    ]);
    expect(new Set(blocks.map((block) => block.key)).size).toBe(blocks.length);
  });

  it("leaves out blocks the section does not have", () => {
    expect(sectionBlocks(content())).toEqual([]);
    expect(sectionBlocks(content({ workedExamples: [{ title: "Only", prompt: "", steps: [], answer: "", takeaway: "" }] })))
      .toEqual([{ key: "example-0", label: "Worked example" }]);
  });
});

describe("guide overlays", () => {
  const topicId = "1".repeat(32);
  it("finds the active note or this revision's section edit, and a conflicted edit separately", () => {
    const note = overlay({ topicId, overlayKind: "topic_note", plainText: "Note" });
    const edit = overlay({ id: "7".repeat(32), topicId, overlayKind: "body_override", guideId, plainText: "Mine" });
    const conflict = overlay({ id: "6".repeat(32), topicId, overlayKind: "body_override", guideId: "5".repeat(32), state: "conflicted" });
    const guideNote = overlay({ id: "4".repeat(32), plainText: "Guide" });
    const overlays = [note, edit, conflict, guideNote];
    expect(findEditingOverlay(overlays, topicId, "note")).toBe(note);
    expect(findEditingOverlay(overlays, topicId, "override", guideId)).toBe(edit);
    expect(findEditingOverlay(overlays, topicId, "override", "5".repeat(32))).toBeNull();
    expect(findEditingOverlay(overlays, null, "note")).toBe(guideNote);
    expect(findConflictedOverride(overlays, topicId)).toBe(conflict);
  });

  it("stores plain text as paragraphs without empty ones", () => {
    expect(plainTextRichDocument("First\n\n\nSecond  \n\n")).toEqual({
      type: "doc",
      content: [
        { type: "paragraph", content: [{ type: "text", text: "First" }] },
        { type: "paragraph", content: [{ type: "text", text: "Second" }] },
      ],
    });
  });
});
