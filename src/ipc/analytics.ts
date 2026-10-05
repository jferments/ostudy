import { invoke } from "./transport";

import type { ReviewOutcome } from "./practice";
import type { QuestionBody } from "./questions";

export type AnalyticsRange = "7d" | "30d" | "90d" | "1y" | "all" | "custom";
export type TagAnalyticsStatus =
  | "building_data"
  | "needs_attention"
  | "on_track"
  | "strong";
export type AttributionSource = "captured" | "legacy_current_state_backfill";

export interface AnalyticsQuery {
  range: AnalyticsRange;
  deckIds: string[];
  tagIds: string[];
  customStartLocalDate?: string;
  customEndLocalDate?: string;
  highlightedSessionId?: string;
}

export interface AnalyticsScope {
  range: AnalyticsRange;
  startLocalDate: string | null;
  endLocalDate: string;
  startAt: number | null;
  endAt: number;
  bucketGranularity: "day" | "week" | "month";
  /** The current study date, which starts at the "New day starts at" hour. */
  todayLocalDate: string;
  deckIds: string[];
  tagIds: string[];
  highlightedSessionId: string | null;
}

export interface CurrentInventorySummary {
  currentDeckCount: number;
  sourceNoteCount: number;
  generatedCardCount: number;
  studyableCardCount: number;
  studyableDueCount: number;
  needsMorePracticeCount: number;
  /** Ready now, split into due reviews and new cards. */
  readyReviewCount: number;
  readyNewCount: number;
  /** Reviews due before the next study day starts, including ready ones. */
  dueTodayReviewCount: number;
}

export interface HistoricalReviewSummary {
  cardsPracticed: number;
  reviewCount: number;
  correctCount: number;
  correctRate: number | null;
  averageConfidence: number | null;
  averageResponseTimeMs: number | null;
  totalStudyTimeMs: number;
  sessionCount: number;
  /** Reviews of Review-state cards that were due; true retention's evidence. */
  retentionReviewCount: number;
  retentionRememberedCount: number;
  /** Remembered due reviews over due reviews, comparable with desired retention. */
  trueRetention: number | null;
}

export interface DailyLimitUsage {
  newLimit: number;
  reviewLimit: number;
  newCompleted: number;
  reviewsCompleted: number;
  newRemaining: number;
  reviewsRemaining: number;
}

export interface RatingDistribution {
  again: number;
  hard: number;
  good: number;
  easy: number;
}

export interface TimelineBucket {
  startLocalDate: string;
  endLocalDate: string;
  startAt: number;
  endAt: number;
  cardsPracticed: number;
  reviewCount: number;
  newCards: number;
  rememberedCount: number;
  wrongCount: number;
  hardCount: number;
  mediumCount: number;
  easyCount: number;
  correctRate: number | null;
  averageConfidence: number | null;
  averageResponseTimeMs: number | null;
  studyTimeMs: number;
  sessionCount: number;
  containsHighlightedSession: boolean;
  /** Reviews of Review-state cards that were due; true retention's evidence. */
  retentionReviewCount: number;
  retentionRememberedCount: number;
  /** Remembered due reviews over due reviews, comparable with desired retention. */
  trueRetention: number | null;
}

export interface DeckAnalytics {
  deckId: string;
  deckName: string;
  isArchived: boolean;
  sourceNoteCount: number;
  generatedCardCount: number;
  studyableCardCount: number;
  studyableDueCount: number;
  needsMorePracticeCount: number;
  cardsPracticed: number;
  reviewCount: number;
  correctCount: number;
  correctRate: number | null;
  averageConfidence: number | null;
  averageResponseTimeMs: number | null;
  totalStudyTimeMs: number;
  lastPracticedAt: number | null;
  retentionReviewCount: number;
  retentionRememberedCount: number;
  trueRetention: number | null;
}

export interface TagAnalytics {
  tagId: string;
  tagName: string;
  isCurrent: boolean;
  sourceNoteCount: number;
  generatedCardCount: number;
  studyableCardCount: number;
  studyableDueCount: number;
  needsMorePracticeCount: number;
  cardsPracticed: number;
  reviewCount: number;
  correctCount: number;
  correctRate: number | null;
  averageConfidence: number | null;
  averageResponseTimeMs: number | null;
  totalStudyTimeMs: number;
  lastPracticedAt: number | null;
  retentionReviewCount: number;
  retentionRememberedCount: number;
  trueRetention: number | null;
  trendDelta: number | null;
  status: TagAnalyticsStatus;
}

export interface RecentReview {
  id: string;
  deckName: string;
  capturedDeckName: string;
  cardId: string;
  cardLabel: string;
  questionExcerpt: string;
  tags: RecentReviewTag[];
  outcome: ReviewOutcome;
  rating: 1 | 2 | 3 | 4;
  responseTimeMs: number;
  reviewedAt: number;
  reviewLocalDate: string;
  dueAfter: number;
  attributionSource: AttributionSource;
}

export interface RecentReviewTag {
  tagId: string;
  tagName: string;
  isCurrent: boolean;
}

export interface SessionAnalytics {
  id: string;
  mode: "scheduled" | "extra" | "custom";
  deckNames: string[];
  startedAt: number;
  completedAt: number | null;
  durationMs: number;
  cardCount: number;
  ratedCount: number;
  correctCount: number;
  correctRate: number | null;
  studyTimeMs: number;
  highlighted: boolean;
}

export interface AttributionCoverage {
  capturedReviewCount: number;
  legacyBackfilledReviewCount: number;
}

export interface AnalyticsDeckOption {
  id: string;
  name: string;
  isArchived: boolean;
  hasCurrentCards: boolean;
  hasHistory: boolean;
}

export interface AnalyticsTagOption {
  id: string;
  name: string;
  isCurrent: boolean;
  hasHistory: boolean;
}

export interface AnalyticsDimensionOptions {
  decks: AnalyticsDeckOption[];
  tags: AnalyticsTagOption[];
}

/** Reviewed first attempts, grouped by the captured question type. */
export interface QuestionTypeAnalytics {
  questionType: "basic" | "cloze" | QuestionBody["kind"];
  reviewCount: number;
  selfAssessedCount: number;
  objectiveCount: number;
  correctCount: number;
  incorrectCount: number;
  incompleteCount: number;
  objectiveAccuracy: number | null;
  unassistedObjectiveCount: number;
  unassistedCorrectCount: number;
  unassistedAccuracy: number | null;
  assistanceTrackedCount: number;
  assistedCount: number;
  overrideCount: number;
  remediationCount: number;
  unavailableEvidenceCount: number;
  /** Remembered reviews by the same rule as every other surface. */
  rememberedCount: number;
  rememberedRate: number | null;
}

export interface Analytics {
  scope: AnalyticsScope;
  currentInventory: CurrentInventorySummary;
  history: HistoricalReviewSummary;
  dailyLimits: DailyLimitUsage;
  ratingDistribution: RatingDistribution;
  timeline: TimelineBucket[];
  deckStats: DeckAnalytics[];
  tagStats: TagAnalytics[];
  recentReviews: RecentReview[];
  recentSessions: SessionAnalytics[];
  attributionCoverage: AttributionCoverage;
  dimensionOptions: AnalyticsDimensionOptions;
  questionTypes: QuestionTypeAnalytics[];
}

export async function getAnalytics(input: AnalyticsQuery): Promise<Analytics> {
  return invoke<Analytics>("get_analytics", { input });
}

export async function getRecentPracticeReviews(): Promise<RecentReview[]> {
  return invoke<RecentReview[]>("get_recent_practice_reviews");
}
