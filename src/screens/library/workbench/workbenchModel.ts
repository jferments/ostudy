import type {
  AvailabilityFilter,
  CardBrowserRow,
  CardSearchInput,
  CardSort,
  CardStatusFilter,
  ReviewedFilter,
  SelectionCard,
} from "../../../ipc/cards";
import type { DeckSummary } from "../../../ipc/decks";
import type { Subject, SubjectLibrary } from "../../../ipc/subjects";
import type { LibrarySubjectScope } from "../../../navigation/appDestinations";

/**
 * The library-wide card lists. The tree offers only All cards; the others are
 * kept so older launches still name a list, and read as the Show filter.
 */
export type WorkbenchView = "all" | "due" | "new" | "often_missed" | "suspended";

/**
 * What the card list shows: all cards, a subject, one deck, or one tag. A tag
 * chosen inside a subject keeps to that subject's decks.
 */
export type WorkbenchScope =
  | { kind: "view"; view: WorkbenchView }
  | { kind: "subject"; subjectId: string | null }
  | { kind: "deck"; deckId: string }
  | { kind: "tag"; tagId: string; subjectId?: string | null };

/** The Show filter above the card list, inside the chosen scope. */
export type CardFilter = WorkbenchView;

export const DEFAULT_SCOPE: WorkbenchScope = { kind: "view", view: "all" };

export const CARD_FILTERS: ReadonlyArray<{ value: CardFilter; label: string }> = [
  { value: "all", label: "All cards" },
  { value: "due", label: "Due today" },
  { value: "new", label: "New" },
  { value: "often_missed", label: "Often missed" },
  { value: "suspended", label: "Suspended" },
];

export const SORT_OPTIONS: ReadonlyArray<{ value: CardSort; label: string }> = [
  { value: "source_order", label: "Source order" },
  { value: "due_asc", label: "Due first" },
  { value: "updated_desc", label: "Recently edited" },
  { value: "last_reviewed_desc", label: "Recently studied" },
  { value: "relevance", label: "Best match" },
];

export function sameScope(left: WorkbenchScope, right: WorkbenchScope): boolean {
  if (left.kind !== right.kind) return false;
  switch (left.kind) {
    case "view":
      return left.view === (right as typeof left).view;
    case "subject":
      return left.subjectId === (right as typeof left).subjectId;
    case "deck":
      return left.deckId === (right as typeof left).deckId;
    case "tag":
      return left.tagId === (right as typeof left).tagId && left.subjectId === (right as typeof left).subjectId;
  }
}

/** Where the Workbench starts for Library's chosen subject: all of its cards. */
export function subjectRootScope(subject: LibrarySubjectScope): WorkbenchScope {
  if (subject.kind === "subject") return { kind: "subject", subjectId: subject.subjectId };
  if (subject.kind === "none") return { kind: "subject", subjectId: null };
  return DEFAULT_SCOPE;
}

/** The subject Library's choice names, `null` for No subject, or undefined for Everything. */
export function subjectFilterOf(subject: LibrarySubjectScope): string | null | undefined {
  return subject.kind === "subject" ? subject.subjectId : subject.kind === "none" ? null : undefined;
}

/** The sort a scope opens with: a deck or subject in source order, the due list by due date. */
export function defaultSort(scope: WorkbenchScope): CardSort {
  if (scope.kind === "deck" || scope.kind === "subject") return "source_order";
  if (scope.kind === "view" && scope.view === "due") return "due_asc";
  return "updated_desc";
}

interface Filters {
  availabilityFilter: AvailabilityFilter;
  statusFilter: CardStatusFilter;
  reviewedFilter: ReviewedFilter;
}

const FILTERS: Readonly<Record<WorkbenchView, Filters>> = {
  all: { availabilityFilter: "all", statusFilter: "all", reviewedFilter: "all" },
  due: { availabilityFilter: "ready_now", statusFilter: "active", reviewedFilter: "all" },
  new: { availabilityFilter: "all", statusFilter: "active", reviewedFilter: "never" },
  often_missed: { availabilityFilter: "all", statusFilter: "needs_more_practice", reviewedFilter: "all" },
  suspended: { availabilityFilter: "all", statusFilter: "suspended", reviewedFilter: "all" },
};

/**
 * The card search for a scope and its Show filter. A library-wide view other
 * than All cards (from an older launch) applies until another filter is
 * chosen. Subject scopes list the subject's live decks; a subject with no
 * decks searches none, never the whole library.
 */
export function searchInputFor(
  scope: WorkbenchScope,
  filter: CardFilter,
  query: string,
  sort: CardSort,
  library: WorkbenchLibrary,
  limit: number,
): CardSearchInput | null {
  const view = filter === "all" && scope.kind === "view" ? scope.view : filter;
  let deckIds: string[] = [];
  let tagIds: string[] = [];
  if (scope.kind === "deck") deckIds = [scope.deckId];
  if (scope.kind === "subject") {
    deckIds = decksInSubject(library, scope.subjectId).map((deck) => deck.id);
    if (deckIds.length === 0) return null;
  }
  if (scope.kind === "tag") {
    tagIds = [scope.tagId];
    if (scope.subjectId !== undefined) {
      deckIds = decksInSubject(library, scope.subjectId).map((deck) => deck.id);
      if (deckIds.length === 0) return null;
    }
  }
  const trimmed = query.trim();
  const effectiveSort: CardSort = trimmed === "" && sort === "relevance" ? defaultSort(scope) : sort;
  return { deckIds, query: trimmed, tagIds, ...FILTERS[view], sort: effectiveSort, limit, offset: 0 };
}

/** The decks and subjects the tree and the actions work from. */
export interface WorkbenchLibrary {
  subjects: Subject[];
  decks: DeckSummary[];
  organization: SubjectLibrary | null;
}

export interface SubjectNode {
  /** null is No subject. */
  subjectId: string | null;
  subject: Subject | null;
  name: string;
  decks: DeckSummary[];
  cardCount: number;
  dueCount: number;
}

export function subjectOfDeck(library: WorkbenchLibrary, deckId: string): string | null {
  return library.organization?.decks.find((placement) => placement.deckId === deckId)?.subjectId ?? null;
}

/** A subject's live decks in the learner's order. */
export function decksInSubject(library: WorkbenchLibrary, subjectId: string | null): DeckSummary[] {
  const byId = new Map(library.decks.map((deck) => [deck.id, deck]));
  const placements = library.organization?.decks ?? [];
  const placed = new Set(placements.map((placement) => placement.deckId));
  const ordered = placements
    .filter((placement) => placement.subjectId === subjectId)
    .sort((left, right) => left.sortOrder - right.sortOrder)
    .map((placement) => byId.get(placement.deckId))
    .filter((deck): deck is DeckSummary => deck !== undefined && deck.archivedAt === null);
  // A deck the organization has not caught up with yet waits under No subject.
  if (subjectId === null) {
    ordered.push(...library.decks.filter((deck) => deck.archivedAt === null && !placed.has(deck.id)));
  }
  return ordered;
}

/**
 * Subjects in the learner's order, then No subject when it holds anything.
 * Counts are the decks' card and due counts, which the deck list already
 * carries, so the tree needs no extra queries.
 */
export function subjectNodes(library: WorkbenchLibrary): SubjectNode[] {
  const subjects = [...library.subjects].sort((left, right) => left.sortOrder - right.sortOrder);
  const nodes: SubjectNode[] = subjects.map((subject) => node(library, subject.id, subject, subject.name));
  const unfiled = node(library, null, null, "No subject");
  if (unfiled.decks.length > 0) nodes.push(unfiled);
  return nodes;
}

function node(library: WorkbenchLibrary, subjectId: string | null, subject: Subject | null, name: string): SubjectNode {
  const decks = decksInSubject(library, subjectId);
  return {
    subjectId,
    subject,
    name,
    decks,
    cardCount: decks.reduce((total, deck) => total + deck.generatedCardCount, 0),
    dueCount: decks.reduce((total, deck) => total + deck.studyableDueCount, 0),
  };
}

export function archivedDecks(library: WorkbenchLibrary): DeckSummary[] {
  return library.decks
    .filter((deck) => deck.archivedAt !== null)
    .sort((left, right) => left.name.localeCompare(right.name, undefined, { sensitivity: "base" }));
}

export function scopeTitle(scope: WorkbenchScope, library: WorkbenchLibrary, tagName: (tagId: string) => string | undefined): string {
  switch (scope.kind) {
    case "view":
      return CARD_FILTERS.find((entry) => entry.value === scope.view)?.label ?? "All cards";
    case "subject":
      return scope.subjectId === null
        ? "No subject"
        : (library.subjects.find((subject) => subject.id === scope.subjectId)?.name ?? "Subject");
    case "deck":
      return library.decks.find((deck) => deck.id === scope.deckId)?.name ?? "Deck";
    case "tag":
      return tagName(scope.tagId) ?? "Tag";
  }
}

/** Whether a scope still names something that exists. */
export function scopeExists(scope: WorkbenchScope, library: WorkbenchLibrary): boolean {
  if (scope.kind === "deck") return library.decks.some((deck) => deck.id === scope.deckId);
  if (scope.kind === "subject" && scope.subjectId !== null) {
    return library.subjects.some((subject) => subject.id === scope.subjectId);
  }
  return true;
}

export function selectionFromRow(row: CardBrowserRow): SelectionCard {
  return { cardId: row.cardId, noteId: row.noteId, deckId: row.deckId };
}

/** One entry per note: tags, moves and deletes act on notes, not on each presentation. */
export function uniqueNotes(cards: readonly SelectionCard[]): SelectionCard[] {
  return [...new Map(cards.map((card) => [card.noteId, card])).values()];
}

const DAY = 86_400_000;

export type DueTone = "due" | "new" | "suspended" | "later";

/** The due column: "today", "in 3 d", "new" or "suspended", from the row's own schedule. */
export function dueLabel(row: CardBrowserRow, todayStartAt: number): { text: string; tone: DueTone } {
  if (row.suspendedAt !== null) return { text: "suspended", tone: "suspended" };
  if (row.reviewCount === 0) return { text: "new", tone: "new" };
  const tomorrowStart = todayStartAt + DAY;
  if (row.dueAt < tomorrowStart) return { text: "today", tone: "due" };
  const days = Math.max(1, Math.ceil((row.dueAt - tomorrowStart + 1) / DAY));
  return { text: `in ${days} d`, tone: "later" };
}

export function intervalLabel(row: CardBrowserRow): string {
  return row.reviewCount === 0 || row.intervalDays <= 0 ? "–" : `${row.intervalDays} d`;
}

const UNIT_LABELS: Readonly<Record<string, string>> = {
  page: "Page",
  slide: "Slide",
  chapter: "Chapter",
  section: "Section",
  segment: "Part",
  image: "Image",
  time_segment: "Part",
};

export function sourceUnitLabel(kind: string | null): string {
  return (kind && UNIT_LABELS[kind]) ?? "Source";
}

/** The column heading for a list's source units: "Slide" when every cited card names slides. */
export function sourceColumnLabel(rows: readonly CardBrowserRow[]): string | null {
  const kinds = new Set(rows.filter((row) => row.sourceUnit !== null).map((row) => row.sourceUnitKind));
  if (kinds.size === 0) return null;
  return kinds.size === 1 ? sourceUnitLabel([...kinds][0] ?? null) : "Source";
}
