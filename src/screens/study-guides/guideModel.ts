import type { StudyGuideSourceSummary } from "../../controllers/useStudyGuidesController";
import type {
  DocumentStudyGuide,
  DocumentStudyGuideSection,
  SourceLocator,
  StudyGuideOverlay,
  StudyGuideSectionContent,
} from "../../ipc/studyGuides";

/** What the library already knows about a guide's source document. */
export type GuideSourceSummary = StudyGuideSourceSummary;

/** One section in reading order, with the title the learner sees. */
export interface GuideSectionEntry {
  section: DocumentStudyGuideSection;
  /** 1-based position in reading order. */
  number: number;
  title: string;
}

/** A run of sections under an editorial chapter, or the whole guide when it has none. */
export interface GuideSectionGroup {
  id: string;
  title: string | null;
  introduction: string;
  entries: GuideSectionEntry[];
}

export interface GuideReadingOrder {
  entries: GuideSectionEntry[];
  groups: GuideSectionGroup[];
}

/**
 * Sections in editorial chapter order when the guide has chapters, and in generated order
 * otherwise. Native assembly places every topic in exactly one chapter; a section that is
 * somehow missing from the chapters still appears, after them, so nothing becomes unreachable.
 */
export function guideReadingOrder(guide: DocumentStudyGuide): GuideReadingOrder {
  const byTopic = new Map(guide.sections.map((section) => [section.topicId, section]));
  const placed = new Set<string>();
  const entries: GuideSectionEntry[] = [];
  const groups: GuideSectionGroup[] = [];
  function place(section: DocumentStudyGuideSection, editorialTitle: string | null): GuideSectionEntry {
    placed.add(section.topicId);
    const entry = {
      section,
      number: entries.length + 1,
      title: effectiveSectionTitle(section, guide.overlays, guide.id, editorialTitle),
    };
    entries.push(entry);
    return entry;
  }
  for (const chapter of guide.editorial?.chapters ?? []) {
    const chapterEntries = chapter.topics.flatMap((topic) => {
      const section = byTopic.get(topic.topicId);
      return section && !placed.has(section.topicId) ? [place(section, topic.title)] : [];
    });
    if (chapterEntries.length > 0) {
      groups.push({ id: chapter.id, title: chapter.title, introduction: chapter.introduction, entries: chapterEntries });
    }
  }
  const remaining = guide.sections.filter((section) => !placed.has(section.topicId)).map((section) => place(section, null));
  if (remaining.length > 0) groups.push({ id: "sections", title: null, introduction: "", entries: remaining });
  return { entries, groups };
}

/** A learner's title override wins, then the editorial title, then the generated one. */
export function effectiveSectionTitle(
  section: DocumentStudyGuideSection,
  overlays: StudyGuideOverlay[],
  guideId: string,
  editorialTitle: string | null,
): string {
  const override = overlays.find((overlay) => overlay.topicId === section.topicId
    && overlay.overlayKind === "title_override" && overlay.state === "active"
    && overlay.guideId === guideId && overlay.plainText.trim() !== "");
  return override?.plainText.trim() || editorialTitle?.trim() || section.title;
}

const WORDS_PER_MINUTE = 200;

/** Whole minutes at an unhurried 200 words a minute, and never less than one. */
export function readingMinutes(text: string): number {
  const words = text.trim().split(/\s+/u).filter(Boolean).length;
  return Math.max(1, Math.round(words / WORDS_PER_MINUTE));
}

export function formatMinutes(minutes: number): string {
  return `about ${minutes} minute${minutes === 1 ? "" : "s"}`;
}

export function plural(count: number, singular: string, pluralForm = `${singular}s`): string {
  return `${count} ${count === 1 ? singular : pluralForm}`;
}

const unitNames: Record<string, readonly [string, string]> = {
  page: ["p.", "pp."],
  slide: ["slide", "slides"],
  chapter: ["chapter", "chapters"],
  section: ["section", "sections"],
  segment: ["segment", "segments"],
  image: ["image", "images"],
  time_segment: ["part", "parts"],
  sheet: ["sheet", "sheets"],
  row_group: ["row group", "row groups"],
};

/** "p. 4", "pp. 6–8", "slides 14–19, 22": where in the source a section comes from. */
export function formatLocatorRange(locator: Pick<SourceLocator, "unitKind" | "unitNumbers" | "sectionLabel">): string | null {
  const numbers = [...new Set(locator.unitNumbers.filter((value) => Number.isInteger(value) && value > 0))]
    .sort((left, right) => left - right);
  if (numbers.length === 0) return locator.sectionLabel?.trim() || null;
  const ranges: string[] = [];
  let start = numbers[0] ?? 0;
  let previous = start;
  for (const value of [...numbers.slice(1), Number.NaN]) {
    if (value === previous + 1) { previous = value; continue; }
    ranges.push(start === previous ? `${start}` : `${start}–${previous}`);
    start = value;
    previous = value;
  }
  const names = unitNames[locator.unitKind] ?? [locator.unitKind.replaceAll("_", " "), locator.unitKind.replaceAll("_", " ")];
  return `${numbers.length === 1 ? names[0] : names[1]} ${ranges.join(", ")}`;
}

/** "12 pages", "1 slide": the length of the whole source document. */
export function formatSourceLength(source: GuideSourceSummary | null): string | null {
  if (!source?.unitCount || source.unitCount <= 0) return null;
  const kind = source.unitKind ?? "page";
  const names: Record<string, readonly [string, string]> = { ...unitNames, page: ["page", "pages"] };
  const [one, many] = names[kind] ?? [kind.replaceAll("_", " "), `${kind.replaceAll("_", " ")}s`];
  return `${source.unitCount.toLocaleString()} ${source.unitCount === 1 ? one : many}`;
}

const formatLabels: Record<string, string> = {
  pdf: "PDF", doc: "DOC", docx: "DOCX", ppt: "PPT", pptx: "PPTX", odt: "ODT", odp: "ODP", rtf: "RTF",
  epub: "EPUB", excel: "XLSX", xls: "XLS", xlsx: "XLSX", ods: "ODS", csv: "CSV", html: "HTML",
  plain_text: "TXT", image_bundle: "IMG",
};

/** The short file-kind badge, such as PDF or PPTX, from the file name or else the detected format. */
export function fileKindLabel(source: GuideSourceSummary | null): string | null {
  const extension = source?.filename?.match(/\.([a-z0-9]{2,4})$/iu)?.[1];
  if (extension && /[a-z]/iu.test(extension)) return extension.toUpperCase();
  return source?.format ? formatLabels[source.format] ?? null : null;
}

/** A block of a section page that the outline can jump to. */
export interface GuideBlock {
  key: string;
  label: string;
}

/**
 * The headed blocks of a section page in reading order. The page and its outline both read this
 * list, so an outline entry always has a heading to land on.
 */
export function sectionBlocks(content: StudyGuideSectionContent): GuideBlock[] {
  const blocks: GuideBlock[] = [];
  if (content.overview.trim() || content.keyPoints.length > 0) blocks.push({ key: "minute", label: "In one minute" });
  content.explanations.forEach((explanation, index) => blocks.push({ key: `explanation-${index}`, label: explanation.heading }));
  if (content.comparisons.length > 0) blocks.push({ key: "comparisons", label: "Comparisons" });
  if (content.processSteps.length > 0) blocks.push({ key: "process", label: "Process" });
  content.workedExamples.forEach((_, index) => blocks.push({
    key: `example-${index}`,
    label: content.workedExamples.length === 1 ? "Worked example" : `Worked example ${index + 1}`,
  }));
  if (content.commonMisconceptions.length > 0) blocks.push({ key: "misconceptions", label: "Common misconceptions" });
  if (content.retrievalChecks.length > 0) blocks.push({ key: "check", label: "Check yourself" });
  return blocks;
}

/** The plain text a learner reads on a section page, for its reading time. */
export function sectionReadingText(section: DocumentStudyGuideSection, override: StudyGuideOverlay | null): string {
  return override?.state === "active" ? override.plainText : section.contentText;
}

/** Splits "Term — explanation" into its two parts for the key concept list. */
export function splitOverviewConcept(value: string): [string, string] {
  const separator = value.indexOf(" — ");
  if (separator < 0) return [value, ""];
  return [value.slice(0, separator).trim(), value.slice(separator + 3).trim()];
}

export type EditingMode = "note" | "override";

export function findEditingOverlay(
  overlays: StudyGuideOverlay[],
  topicId: string | null,
  mode: EditingMode,
  guideId?: string,
): StudyGuideOverlay | null {
  const kind = mode === "override" ? "body_override" : topicId ? "topic_note" : "guide_note";
  return overlays.find((overlay) => overlay.overlayKind === kind && overlay.topicId === topicId
    && overlay.state === "active" && (mode !== "override" || overlay.guideId === guideId)) ?? null;
}

export function findConflictedOverride(overlays: StudyGuideOverlay[], topicId: string): StudyGuideOverlay | null {
  return overlays.find((overlay) => overlay.topicId === topicId && overlay.overlayKind === "body_override"
    && overlay.state === "conflicted") ?? null;
}

export function plainTextRichDocument(value: string): Record<string, unknown> {
  return {
    type: "doc",
    content: value.split(/\n{2,}/u).map((paragraph) => paragraph.trim()).filter(Boolean)
      .map((paragraph) => ({ type: "paragraph", content: [{ type: "text", text: paragraph }] })),
  };
}
