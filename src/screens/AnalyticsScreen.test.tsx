import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as analyticsIpc from "../analytics-ipc";
import * as ipc from "../ipc";
import { AnalyticsScreen } from "./AnalyticsScreen";

vi.mock("../analytics-ipc", () => ({
  getAnalytics: vi.fn(),
}));

vi.mock("../ipc", () => ({
  getErrorMessage: (error: unknown) => error instanceof Error ? error.message : "Unexpected error",
  listMatchingCards: vi.fn(),
  listTags: vi.fn(),
}));

vi.mock("./analytics/ProgressChart", () => ({
  ProgressChart: () => <div role="img" aria-label="Learning progress chart" />,
}));

const deck: ipc.Deck = {
  id: "a".repeat(32),
  name: "Cardiology",
  description: "",
  cardCount: 12,
  dueCount: 4,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

const settings: ipc.AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  reduceMotion: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

const analytics: analyticsIpc.Analytics = {
  scope: {
    range: "7d",
    startAt: 1,
    endAt: 2,
    bucketGranularity: "day",
    deckIds: [],
    tagIds: [],
    highlightedSessionId: null,
  },
  summary: {
    deckCount: 1,
    cardCount: 12,
    dueNow: 4,
    needsMorePractice: 2,
    cardsPracticed: 8,
    reviewCount: 10,
    correctCount: 8,
    correctRate: 0.8,
    averageConfidence: 1.9,
    averageResponseTimeMs: 4200,
    totalStudyTimeMs: 120_000,
    sessionCount: 2,
  },
  dailyLimits: {
    newLimit: 20,
    reviewLimit: 200,
    newCompleted: 3,
    reviewsCompleted: 10,
    newRemaining: 17,
    reviewsRemaining: 190,
  },
  ratingDistribution: { again: 2, hard: 2, good: 4, easy: 2 },
  timeline: [
    {
      startAt: 1,
      endAt: 2,
      cardsPracticed: 8,
      reviewCount: 10,
      newCards: 3,
      rememberedCount: 8,
      correctRate: 0.8,
      averageConfidence: 1.9,
      averageResponseTimeMs: 4200,
      studyTimeMs: 120_000,
      sessionCount: 2,
      containsHighlightedSession: true,
    },
  ],
  deckStats: [
    {
      deckId: deck.id,
      deckName: deck.name,
      cardCount: 12,
      dueCount: 4,
      needsMorePractice: 2,
      cardsPracticed: 8,
      reviewCount: 10,
      correctCount: 8,
      correctRate: 0.8,
      averageConfidence: 1.9,
      averageResponseTimeMs: 4200,
      totalStudyTimeMs: 120_000,
      lastPracticedAt: 1,
    },
  ],
  tagStats: [
    {
      tagId: "b".repeat(32),
      tagName: "Arrhythmias",
      cardCount: 6,
      dueCount: 3,
      needsMorePractice: 2,
      cardsPracticed: 4,
      reviewCount: 7,
      correctCount: 4,
      correctRate: 4 / 7,
      averageConfidence: 1.4,
      averageResponseTimeMs: 5000,
      totalStudyTimeMs: 35_000,
      lastPracticedAt: 1,
      trendDelta: -0.2,
      status: "needs_attention",
    },
  ],
  recentReviews: [
    {
      id: "c".repeat(32),
      deckName: deck.name,
      cardId: "d".repeat(32),
      frontText: "What rhythm has no P waves?",
      tags: ["Arrhythmias"],
      outcome: "somewhat_confident",
      rating: 3,
      responseTimeMs: 4000,
      reviewedAt: 1,
      dueAfter: Date.now() + 86_400_000,
    },
  ],
  recentSessions: [
    {
      id: "e".repeat(32),
      mode: "scheduled",
      deckNames: [deck.name],
      startedAt: 1,
      completedAt: 2,
      durationMs: 180_000,
      cardCount: 10,
      ratedCount: 10,
      correctCount: 8,
      correctRate: 0.8,
      studyTimeMs: 120_000,
      highlighted: true,
    },
  ],
};

describe("AnalyticsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(analyticsIpc.getAnalytics).mockResolvedValue(analytics);
    vi.mocked(ipc.listTags).mockResolvedValue([
      { id: analytics.tagStats[0].tagId, name: "Arrhythmias", cardCount: 6 },
    ]);
    vi.mocked(ipc.listMatchingCards).mockResolvedValue([
      { cardId: "f".repeat(32), noteId: "1".repeat(32), deckId: deck.id },
    ]);
  });

  it("starts with the past week and shows an accessible data alternative", async () => {
    render(
      <AnalyticsScreen
        decks={[deck]}
        settings={settings}
        launchRequest={null}
        onLaunchConsumed={vi.fn()}
        onPracticeCards={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    expect(await screen.findByText("Cards practiced")).toBeTruthy();
    await waitFor(() => {
      expect(analyticsIpc.getAnalytics).toHaveBeenCalledWith({
        range: "7d",
        deckIds: [],
        tagIds: [],
        highlightedSessionId: undefined,
      });
    });
    expect(screen.getByRole("button", { name: "7 days" }).getAttribute("aria-pressed")).toBe("true");
    expect(screen.getByText("8")).toBeTruthy();
    expect(screen.getAllByText("80%").length).toBeGreaterThan(0);
    expect(screen.getAllByText("2 min").length).toBeGreaterThan(0);
    expect(screen.getByRole("img", { name: "Learning progress chart" })).toBeTruthy();
    expect(screen.getAllByText("Arrhythmias").length).toBeGreaterThan(0);
    expect(screen.getByText("Needs attention")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "View chart data" }));
    expect(screen.getByRole("table", { name: "Data represented in the learning progress chart" })).toBeTruthy();
  });

  it("changes the range and starts a due-only custom session for a weak tag", async () => {
    const onPracticeCards = vi.fn();
    render(
      <AnalyticsScreen
        decks={[deck]}
        settings={settings}
        launchRequest={null}
        onLaunchConsumed={vi.fn()}
        onPracticeCards={onPracticeCards}
        showToast={vi.fn()}
      />,
    );

    await screen.findAllByText("Arrhythmias");
    fireEvent.click(screen.getByRole("button", { name: "90 days" }));
    await waitFor(() => {
      expect(analyticsIpc.getAnalytics).toHaveBeenLastCalledWith(
        expect.objectContaining({ range: "90d" }),
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Practice due" }));
    await waitFor(() => {
      expect(ipc.listMatchingCards).toHaveBeenCalledWith(
        expect.objectContaining({
          dueFilter: "due",
          statusFilter: "active",
          tagIds: [analytics.tagStats[0].tagId],
        }),
      );
      expect(onPracticeCards).toHaveBeenCalledWith(["f".repeat(32)]);
    });
  });
});
