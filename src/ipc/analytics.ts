import { invoke } from "@tauri-apps/api/core";

import type { ReviewOutcome } from "./practice";

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
  tags: string[];
  outcome: ReviewOutcome;
  rating: 1 | 2 | 3 | 4;
  responseTimeMs: number;
  reviewedAt: number;
  reviewLocalDate: string;
  dueAfter: number;
  attributionSource: AttributionSource;
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
}

export async function getAnalytics(input: AnalyticsQuery): Promise<Analytics> {
  return invoke<Analytics>("get_analytics", { input });
}

export async function getRecentPracticeReviews(): Promise<RecentReview[]> {
  return invoke<RecentReview[]>("get_recent_practice_reviews");
}
