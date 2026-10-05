import { invoke } from "./transport";

import type {
  AnalyticsDimensionOptions,
  AnalyticsRange,
  AnalyticsScope,
  QuestionTypeAnalytics,
  RecentReview,
  SessionAnalytics,
  TagAnalytics,
} from "./analytics";

/** The scope every scoped Progress section shares. Each non-empty filter narrows it further. */
export interface ProgressQuery {
  range: AnalyticsRange;
  subjectIds: string[];
  deckIds: string[];
  tagIds: string[];
  customStartLocalDate?: string;
  customEndLocalDate?: string;
}

/** Review totals for a period of current cards, with their rates. */
export interface ProgressSummary {
  reviewCount: number;
  newCardCount: number;
  rememberedCount: number;
  correctRate: number | null;
  averageConfidence: number | null;
  averageResponseTimeMs: number | null;
  studyTimeMs: number;
  retentionReviewCount: number;
  retentionRememberedCount: number;
  trueRetention: number | null;
}

export interface ProgressBucket extends ProgressSummary {
  startLocalDate: string;
  endLocalDate: string;
}

export interface ProgressOverview {
  scope: AnalyticsScope;
  summary: ProgressSummary;
  /** The equal-length period just before; null for all time. */
  previousSummary: ProgressSummary | null;
  /** Study dates in the range with at least one review. */
  studyDayCount: number;
  /** Buckets are cut back from today, so only the first can be partial. All time has one per month. */
  timeline: ProgressBucket[];
  desiredRetention: number;
}

export interface ForecastDay {
  localDate: string;
  dueCount: number;
}

export interface ProgressForecast {
  /** Reviews due on each of 14 study days from today; today includes overdue. */
  days: ForecastDay[];
  overdueCount: number;
  newAvailableCount: number;
}

export interface CardStrength {
  newCount: number;
  learningCount: number;
  gettingThereCount: number;
  solidCount: number;
  solidStabilityDays: number;
  /** Mean predicted recall right now over studied cards. */
  estimatedRecall: number | null;
  estimatedRecallCardCount: number;
}

export interface ActivityDay {
  localDate: string;
  reviewCount: number;
  studyTimeMs: number;
}

export interface Streak {
  length: number;
  startLocalDate: string;
  endLocalDate: string;
}

/** The whole library's study calendar over the last 26 weeks. */
export interface ProgressActivity {
  startLocalDate: string;
  todayLocalDate: string;
  /** Study days with at least one review, oldest first. */
  days: ActivityDay[];
  currentStreak: Streak | null;
  longestStreak: Streak | null;
  /** Monday is 0. */
  busiestWeekday: number | null;
}

export interface SubjectProgress {
  /** null is the decks filed under no subject. */
  subjectId: string | null;
  name: string | null;
  cardCount: number;
  solidCount: number;
  dueTodayCount: number;
  reviewCount: number;
  rememberedCount: number;
  correctRate: number | null;
  retentionReviewCount: number;
  retentionRememberedCount: number;
  trueRetention: number | null;
  retentionDelta: number | null;
  studyTimeMs: number;
}

export interface ReviewLogCursor {
  reviewedAt: number;
  createdAt: number;
  id: string;
}

export interface ReviewLogPage {
  reviews: RecentReview[];
  /** Pass back as `before` for the next page; null on the last page. */
  next: ReviewLogCursor | null;
}

export async function getProgressOverview(input: ProgressQuery): Promise<ProgressOverview> {
  return invoke<ProgressOverview>("get_progress_overview", { input });
}

export async function getProgressForecast(input: ProgressQuery): Promise<ProgressForecast> {
  return invoke<ProgressForecast>("get_progress_forecast", { input });
}

export async function getProgressCardStrength(input: ProgressQuery): Promise<CardStrength> {
  return invoke<CardStrength>("get_progress_card_strength", { input });
}

export async function getProgressActivity(): Promise<ProgressActivity> {
  return invoke<ProgressActivity>("get_progress_activity");
}

export async function getProgressSubjects(input: ProgressQuery): Promise<SubjectProgress[]> {
  return invoke<SubjectProgress[]>("get_progress_subjects", { input });
}

export async function getProgressQuestionTypes(input: ProgressQuery): Promise<QuestionTypeAnalytics[]> {
  return invoke<QuestionTypeAnalytics[]>("get_progress_question_types", { input });
}

export async function getProgressSessions(input: ProgressQuery): Promise<SessionAnalytics[]> {
  return invoke<SessionAnalytics[]>("get_progress_sessions", { input });
}

/** Every tag in scope, the ones that need attention first. */
export async function getProgressTopics(input: ProgressQuery): Promise<TagAnalytics[]> {
  return invoke<TagAnalytics[]>("get_progress_topics", { input });
}

export async function getProgressFilterOptions(): Promise<AnalyticsDimensionOptions> {
  return invoke<AnalyticsDimensionOptions>("get_progress_filter_options");
}

export async function getReviewLog(input: ProgressQuery, before: ReviewLogCursor | null = null): Promise<ReviewLogPage> {
  return invoke<ReviewLogPage>("get_review_log", { input, before });
}
