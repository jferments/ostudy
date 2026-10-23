import { invoke } from "./transport";

/**
 * What is due today in one subject, or in No subject (`subjectId` null).
 * Native code owns every count: "due", "today" and the daily limits follow the
 * same study rules a scheduled practice session applies.
 */
export interface HomeSubjectSummary {
  subjectId: string | null;
  /** Studyable review cards due now in the subject's live, unarchived decks. */
  dueReviews: number;
  /** Studyable new cards available now in those decks. */
  dueNew: number;
  /** The reviews this subject's own scheduled session studies today. */
  scheduledReviews: number;
  /** The new cards that session studies. */
  scheduledNew: number;
  /** Current cards, suspended ones included; a merged copy counts once. */
  cardCount: number;
  deckCount: number;
  /** Documents filed in the subject. */
  sourceCount: number;
  hasOverviewGuide: boolean;
  /** Active study guides of the subject's documents, plus its overview guide. */
  guideCount: number;
  lessonCount: number;
  podcastCount: number;
  /** The figure Library shows on the subject's tile, if its documents have one. */
  cover: SubjectCover | null;
  /** The decks holding the subject's due cards: its scheduled session's scope. */
  dueDeckIds: string[];
}

/** A box on one page of one of a subject's documents, in the page's own units. */
export interface SubjectCover {
  documentId: string;
  pageNumber: number;
  pageWidth: number;
  pageHeight: number;
  left: number;
  top: number;
  width: number;
  height: number;
}

/** What Start review studies today across the whole library. */
export interface HomeToday {
  dueReviews: number;
  dueNew: number;
  scheduledReviews: number;
  scheduledNew: number;
  reviewsRemaining: number;
  newRemaining: number;
  /** Subjects holding the due cards Start review studies; No subject is one. */
  subjectCount: number;
  /** Start review's scope: every deck holding a due card. */
  deckIds: string[];
  /** About how long the scheduled cards take, in whole minutes. */
  estimatedMinutes: number;
  /** The time per card behind the estimate. */
  estimateMsPerCard: number;
  /** Recent timed reviews behind the estimate; 0 when it used the default. */
  estimateReviewCount: number;
}

export interface HomeSummary {
  /** The study date the counts belong to, as YYYY-MM-DD. */
  studyDate: string;
  today: HomeToday;
  /** Every subject, in the learner's order. */
  subjects: HomeSubjectSummary[];
  noSubject: HomeSubjectSummary;
}

export async function getHomeSummary(): Promise<HomeSummary> {
  return invoke<HomeSummary>("get_home_summary");
}
