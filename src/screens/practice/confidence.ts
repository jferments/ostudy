import type { RecentReview, ReviewOutcome } from "../../ipc";

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

export function confidenceAverage(counts: ConfidenceCounts, total: number): number {
  if (total === 0) return 0;
  return counts.reduce((sum, count, score) => sum + count * score, 0) / total;
}

export function formatConfidence(value: number | null): string {
  return value === null ? "—" : `${value.toFixed(1)} / 3`;
}
