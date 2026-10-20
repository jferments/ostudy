import type { RecentReview } from "../../ipc/analytics";
import {
  type ReviewOutcome,
} from "../../ipc/practice";

export type ConfidenceScore = 0 | 1 | 2 | 3;
export type ConfidenceCounts = [number, number, number, number];

export interface RecentDeckSummary {
  deckName: string;
  reviewCount: number;
  averageConfidence: number;
  lastReviewedAt: number;
}

export function outcomeForScore(score: ConfidenceScore): ReviewOutcome {
  switch (score) {
    case 0:
      return "wrong";
    case 1:
      return "not_confident";
    case 2:
      return "somewhat_confident";
    case 3:
      return "very_confident";
  }
}

const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;

/** The next interval a rating would schedule, in the short form shown under it: "1 min", "3 h",
 * "4 days", "2 mo", "1 yr". */
export function formatNextInterval(intervalMs: number): string {
  if (intervalMs < HOUR_MS) return `${Math.max(1, Math.round(intervalMs / MINUTE_MS))} min`;
  if (intervalMs < DAY_MS) return `${Math.round(intervalMs / HOUR_MS)} h`;
  const days = Math.round(intervalMs / DAY_MS);
  if (days < 30) return days === 1 ? "1 day" : `${days} days`;
  if (days < 365) return `${Math.round(days / 30)} mo`;
  const years = Math.round((days / 365) * 10) / 10;
  return `${years} yr`;
}

export function summarizeRecentDecks(reviews: RecentReview[]): RecentDeckSummary[] {
  const groups = new Map<string, { count: number; total: number; latest: number }>();
  for (const review of reviews) {
    const current = groups.get(review.deckName) ?? { count: 0, total: 0, latest: 0 };
    current.count += 1;
    current.total += review.rating - 1;
    current.latest = Math.max(current.latest, review.reviewedAt);
    groups.set(review.deckName, current);
  }

  return [...groups.entries()]
    .map(([deckName, group]) => ({
      deckName,
      reviewCount: group.count,
      averageConfidence: group.total / group.count,
      lastReviewedAt: group.latest,
    }))
    .sort((left, right) => right.lastReviewedAt - left.lastReviewedAt)
    .slice(0, 5);
}

export function formatConfidence(value: number | null): string {
  return value === null ? "—" : `${value.toFixed(1)} / 3`;
}
