import type { HomeSubjectSummary, HomeSummary } from "../../ipc/home";
import type { Subject, SubjectLibrary } from "../../ipc/subjects";

export type SubjectSort = "most-due" | "name" | "your-order";

export const SUBJECT_SORT_OPTIONS: ReadonlyArray<{ value: SubjectSort; label: string }> = [
  { value: "most-due", label: "Most due first" },
  { value: "name", label: "Name" },
  { value: "your-order", label: "Your order" },
];

/**
 * The theme's six subject colors as complete utilities. A subject's badge is
 * drawn in its color with the accent ink, and its due count on the soft tint.
 */
export const SUBJECT_COLORS = [
  { badge: "tw:bg-subject-1", soft: "tw:bg-subject-1-soft tw:text-subject-1" },
  { badge: "tw:bg-subject-2", soft: "tw:bg-subject-2-soft tw:text-subject-2" },
  { badge: "tw:bg-subject-3", soft: "tw:bg-subject-3-soft tw:text-subject-3" },
  { badge: "tw:bg-subject-4", soft: "tw:bg-subject-4-soft tw:text-subject-4" },
  { badge: "tw:bg-subject-5", soft: "tw:bg-subject-5-soft tw:text-subject-5" },
  { badge: "tw:bg-subject-6", soft: "tw:bg-subject-6-soft tw:text-subject-6" },
] as const;

/**
 * Subjects store no color. Each takes the theme's subject colors in creation
 * order (oldest first, ties by id): the first six subjects get six different
 * colors, and moving or re-sorting subjects never repaints one. Deleting a
 * subject moves the colors of the subjects created after it by one place.
 */
export function subjectColorIndexes(subjects: readonly Subject[]): Map<string, number> {
  const byCreation = [...subjects].sort(
    (left, right) =>
      left.createdAt - right.createdAt || (left.id < right.id ? -1 : left.id > right.id ? 1 : 0),
  );
  return new Map(
    byCreation.map((subject, index) => [subject.id, index % SUBJECT_COLORS.length]),
  );
}

const MINOR_WORDS = new Set([
  "a",
  "an",
  "and",
  "at",
  "by",
  "for",
  "from",
  "in",
  "into",
  "of",
  "on",
  "or",
  "the",
  "to",
  "with",
]);

/**
 * Two letters for a subject's badge: the first letters of its first two words
 * that are not short joining words ("Spanish for clinicians" is SC), or the
 * first two letters of a one-word name ("Biology" is BI).
 */
export function subjectInitials(name: string): string {
  const words = name
    .split(/\s+/u)
    .map((word) => Array.from(word.replace(/[^\p{L}\p{N}]/gu, "")))
    .filter((letters) => letters.length > 0);
  const significant = words.filter((letters) => !MINOR_WORDS.has(letters.join("").toLowerCase()));
  const source = significant.length > 0 ? significant : words;
  const [first, second] = source;
  if (!first) return "?";
  const initials = second ? `${first[0]}${second[0]}` : first.slice(0, 2).join("");
  return initials.toLocaleUpperCase();
}

/** A study date (`YYYY-MM-DD`) as "Monday, October 5", independent of time zone. */
export function formatStudyDate(studyDate: string): string {
  const date = new Date(`${studyDate}T12:00:00Z`);
  if (Number.isNaN(date.getTime())) return studyDate;
  return new Intl.DateTimeFormat("en-US", {
    weekday: "long",
    month: "long",
    day: "numeric",
    timeZone: "UTC",
  }).format(date);
}

export function countLabel(count: number, singular: string, plural = `${singular}s`): string {
  return `${count.toLocaleString()} ${count === 1 ? singular : plural}`;
}

/** The cards one scheduled session over the subject studies today. */
export function subjectDueCount(subject: HomeSubjectSummary): number {
  return subject.scheduledReviews + subject.scheduledNew;
}

/** A document of the subject being read or written about right now. */
export interface SubjectPreparation {
  step: string;
  documentTitle: string;
}

export interface SubjectTile {
  /** A subject id, or "no-subject". */
  key: string;
  subjectId: string | null;
  name: string;
  initials: string;
  /** An index into SUBJECT_COLORS; null for No subject, which stays neutral. */
  colorIndex: number | null;
  /** The subject's place in the learner's order. */
  position: number;
  summary: HomeSubjectSummary;
  dueCount: number;
  preparing: SubjectPreparation | null;
}

/** Running document work, by document id, as DocumentJobsProvider reports it. */
export type DocumentPreparations = ReadonlyMap<string, SubjectPreparation>;

/**
 * One tile per subject, in the learner's order, plus No subject when unfiled
 * decks or documents exist. A subject the summary does not know yet (created
 * a moment ago) shows as empty until the next read.
 */
export function buildSubjectTiles(
  summary: HomeSummary,
  library: SubjectLibrary,
  preparations: DocumentPreparations,
): { subjects: SubjectTile[]; noSubject: SubjectTile | null } {
  const summaries = new Map(
    summary.subjects.flatMap((entry) => (entry.subjectId ? [[entry.subjectId, entry] as const] : [])),
  );
  const colors = subjectColorIndexes(library.subjects);
  const preparingBySubject = new Map<string | null, SubjectPreparation>();
  for (const placement of library.documents) {
    const preparation = preparations.get(placement.documentId);
    if (preparation && !preparingBySubject.has(placement.subjectId)) {
      preparingBySubject.set(placement.subjectId, preparation);
    }
  }
  const subjects = library.subjects.map((subject, position): SubjectTile => {
    const entry = summaries.get(subject.id) ?? emptySummary(subject.id);
    return {
      key: subject.id,
      subjectId: subject.id,
      name: subject.name,
      initials: subjectInitials(subject.name),
      colorIndex: colors.get(subject.id) ?? 0,
      position,
      summary: entry,
      dueCount: subjectDueCount(entry),
      preparing: preparingBySubject.get(subject.id) ?? null,
    };
  });
  const unfiled = summary.noSubject;
  const noSubject =
    unfiled.deckCount > 0 || unfiled.sourceCount > 0
      ? {
          key: "no-subject",
          subjectId: null,
          name: "No subject",
          initials: "",
          colorIndex: null,
          position: subjects.length,
          summary: unfiled,
          dueCount: subjectDueCount(unfiled),
          preparing: preparingBySubject.get(null) ?? null,
        }
      : null;
  return { subjects, noSubject };
}

export function sortSubjectTiles(tiles: readonly SubjectTile[], sort: SubjectSort): SubjectTile[] {
  const sorted = [...tiles];
  if (sort === "your-order") return sorted.sort((left, right) => left.position - right.position);
  if (sort === "name") {
    return sorted.sort(
      (left, right) =>
        left.name.localeCompare(right.name, undefined, { numeric: true, sensitivity: "base" }) ||
        left.position - right.position,
    );
  }
  return sorted.sort(
    (left, right) => right.dueCount - left.dueCount || left.position - right.position,
  );
}

function emptySummary(subjectId: string): HomeSubjectSummary {
  return {
    subjectId,
    dueReviews: 0,
    dueNew: 0,
    scheduledReviews: 0,
    scheduledNew: 0,
    cardCount: 0,
    deckCount: 0,
    sourceCount: 0,
    hasOverviewGuide: false,
    dueDeckIds: [],
  };
}
