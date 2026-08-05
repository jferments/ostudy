import { describe, expect, it } from "vitest";

import type { RecentReview } from "../../analytics-ipc";
import {
  confidenceAverage,
  formatConfidence,
  outcomeForScore,
  summarizeRecentDecks,
} from "./confidence";

function review(
  deckName: string,
  rating: 1 | 2 | 3 | 4,
  reviewedAt: number,
): RecentReview {
  return {
    id: `${deckName}-${rating}-${reviewedAt}`,
    deckId: "d".repeat(32),
    deckName,
    cardId: "a".repeat(32),
    frontText: "Question",
    tags: [],
    outcome:
      rating === 1
        ? "wrong"
        : rating === 2
          ? "not_confident"
          : rating === 3
            ? "somewhat_confident"
            : "very_confident",
    rating,
    responseTimeMs: 1_000,
    reviewedAt,
    dueAfter: reviewedAt + 60_000,
    attributionSource: "captured",
  };
}

describe("practice confidence helpers", () => {
  it("maps the visible 0-3 scale to durable review outcomes", () => {
    expect([0, 1, 2, 3].map((score) => outcomeForScore(score as 0 | 1 | 2 | 3))).toEqual(
      ["wrong", "not_confident", "somewhat_confident", "very_confident"],
    );
  });

  it("groups recent reviews by deck, converts ratings to 0-3, and sorts by recency", () => {
    const summaries = summarizeRecentDecks([
      review("Cardiology", 4, 100),
      review("Cardiology", 2, 300),
      review("Pharmacology", 3, 200),
    ]);

    expect(summaries).toEqual([
      {
        deckName: "Cardiology",
        reviewCount: 2,
        averageConfidence: 2,
        lastReviewedAt: 300,
      },
      {
        deckName: "Pharmacology",
        reviewCount: 1,
        averageConfidence: 2,
        lastReviewedAt: 200,
      },
    ]);
  });

  it("limits the recent deck strip to five decks", () => {
    const summaries = summarizeRecentDecks(
      Array.from({ length: 8 }, (_, index) => review(`Deck ${index}`, 3, index)),
    );

    expect(summaries).toHaveLength(5);
    expect(summaries.map((item) => item.deckName)).toEqual([
      "Deck 7",
      "Deck 6",
      "Deck 5",
      "Deck 4",
      "Deck 3",
    ]);
  });

  it("calculates and formats confidence without dividing by zero", () => {
    expect(confidenceAverage([1, 1, 1, 1], 4)).toBe(1.5);
    expect(confidenceAverage([0, 0, 0, 0], 0)).toBe(0);
    expect(formatConfidence(2.25)).toBe("2.3 / 3");
    expect(formatConfidence(null)).toBe("—");
  });
});
