import { invoke } from "@tauri-apps/api/core";
import type { ReviewOutcome } from "./ipc";
export type AnalyticsRange="7d"|"30d"|"90d"|"1y"|"all";
export type TagAnalyticsStatus="building_data"|"needs_attention"|"on_track"|"strong";
export interface AnalyticsQuery{range:AnalyticsRange;deckIds:string[];tagIds:string[];highlightedSessionId?:string}
export interface AnalyticsScope{range:AnalyticsRange;startAt:number|null;endAt:number;bucketGranularity:"day"|"week"|"month";deckIds:string[];tagIds:string[];highlightedSessionId:string|null}
export interface AnalyticsSummary{currentDeckCount:number;sourceNoteCount:number;generatedCardCount:number;studyableCardCount:number;studyableDueCount:number;needsMorePractice:number;cardsPracticed:number;reviewCount:number;correctCount:number;correctRate:number|null;averageConfidence:number|null;averageResponseTimeMs:number|null;totalStudyTimeMs:number;sessionCount:number}
export interface DailyLimitUsage{newLimit:number;reviewLimit:number;newCompleted:number;reviewsCompleted:number;newRemaining:number;reviewsRemaining:number}
export interface RatingDistribution{again:number;hard:number;good:number;easy:number}
export interface TimelineBucket{startAt:number;endAt:number;cardsPracticed:number;reviewCount:number;newCards:number;rememberedCount:number;correctRate:number|null;averageConfidence:number|null;averageResponseTimeMs:number|null;studyTimeMs:number;sessionCount:number;containsHighlightedSession:boolean}
export interface DeckAnalytics{deckId:string;deckName:string;archived:boolean;sourceNoteCount:number;generatedCardCount:number;studyableDueCount:number;needsMorePractice:number;cardsPracticed:number;reviewCount:number;correctCount:number;correctRate:number|null;averageConfidence:number|null;averageResponseTimeMs:number|null;totalStudyTimeMs:number;lastPracticedAt:number|null}
export interface TagAnalytics{tagId:string;tagName:string;sourceNoteCount:number;generatedCardCount:number;studyableDueCount:number;needsMorePractice:number;cardsPracticed:number;reviewCount:number;correctCount:number;correctRate:number|null;averageConfidence:number|null;averageResponseTimeMs:number|null;totalStudyTimeMs:number;lastPracticedAt:number|null;trendDelta:number|null;status:TagAnalyticsStatus}
export interface RecentReview{id:string;deckId:string;deckName:string;cardId:string;frontText:string;tags:string[];outcome:ReviewOutcome;rating:1|2|3|4;responseTimeMs:number;reviewedAt:number;dueAfter:number;attributionSource:"captured"|"legacy_current_state_backfill"}
export interface SessionAnalytics{id:string;mode:"scheduled"|"extra"|"custom";deckNames:string[];startedAt:number;completedAt:number|null;durationMs:number;cardCount:number;ratedCount:number;correctCount:number;correctRate:number|null;studyTimeMs:number;highlighted:boolean}
export interface AnalyticsDimensionOption{id:string;name:string;archived:boolean;historicalOnly:boolean}
export interface AnalyticsDimensionOptions{decks:AnalyticsDimensionOption[];tags:AnalyticsDimensionOption[]}
export interface AttributionCoverage{capturedReviewCount:number;legacyBackfilledReviewCount:number}
export interface Analytics{scope:AnalyticsScope;summary:AnalyticsSummary;dailyLimits:DailyLimitUsage;ratingDistribution:RatingDistribution;timeline:TimelineBucket[];deckStats:DeckAnalytics[];tagStats:TagAnalytics[];recentReviews:RecentReview[];recentSessions:SessionAnalytics[];dimensions:AnalyticsDimensionOptions;attributionCoverage:AttributionCoverage}
export async function getAnalytics(input:AnalyticsQuery):Promise<Analytics>{return invoke<Analytics>("get_analytics",{input})}
export async function getRecentPracticeReviews():Promise<RecentReview[]>{return invoke<RecentReview[]>("get_recent_practice_reviews")}
