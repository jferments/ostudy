import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { AnalyticsScreen } from "./AnalyticsScreen";

vi.mock("../ipc", () => ({
  getAnalytics: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));

const analytics: ipc.Analytics = {
  deckCount: 1,
  cardCount: 5,
  dueNow: 2,
  reviewCount: 10,
  correctCount: 8,
  correctRate: 0.8,
  averageResponseTimeMs: 1_500,
  ratingDistribution: { again: 2, hard: 1, good: 3, easy: 4 },
  deckStats: [
    {
      deckId: "a".repeat(32),
      deckName: "Anatomy",
      cardCount: 5,
      dueCount: 2,
      reviewCount: 10,
      correctCount: 8,
      correctRate: 0.8,
      averageResponseTimeMs: 1_500,
    },
  ],
  recentReviews: [
    {
      id: "b".repeat(32),
      deckName: "Anatomy",
      cardId: "c".repeat(32),
      frontText: "Name the chamber.",
      outcome: "very_confident",
      rating: 4,
      responseTimeMs: 1_200,
      reviewedAt: Date.now(),
      dueAfter: Date.now() + 86_400_000,
    },
  ],
};

describe("AnalyticsScreen", () => {
  beforeEach(() => {
    vi.mocked(ipc.getAnalytics).mockResolvedValue(analytics);
  });

  it("presents the learner-facing 0–3 confidence scale", async () => {
    render(<AnalyticsScreen showToast={vi.fn()} />);

    expect(await screen.findByText("Average confidence")).toBeTruthy();
    expect(screen.getByText("1.9 / 3")).toBeTruthy();
    expect(screen.getByText("0 · Wrong")).toBeTruthy();
    expect(screen.getByText("1 · Hard")).toBeTruthy();
    expect(screen.getByText("2 · Medium")).toBeTruthy();
    expect(screen.getByText("3 · Easy")).toBeTruthy();
    expect(screen.getByText("Anatomy · 3 · Easy")).toBeTruthy();
  });
});
