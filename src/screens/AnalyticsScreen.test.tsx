import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as analyticsIpc from "../analytics-ipc";
import * as ipc from "../ipc";
import { deferred } from "../test/factories";
import { AnalyticsScreen } from "./AnalyticsScreen";

vi.mock("../analytics-ipc", () => ({
  getAnalytics: vi.fn(),
}));

vi.mock("../ipc", () => ({
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));

vi.mock("./analytics/ProgressChart", () => ({
  ProgressChart: () => <div role="img" aria-label="Learning progress chart" />,
}));

const deck: ipc.DeckSummary = {
  id: "a".repeat(32),
  name: "Cardiology",
  description: "",
  sourceNoteCount: 10,
  generatedCardCount: 12,
  studyableDueCount: 4,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};

const tagId = "b".repeat(32);

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

function analyticsForRange(
  range: analyticsIpc.AnalyticsRange,
  cardsPracticed = 8,
): analyticsIpc.Analytics {
  return {
    scope: {
      range,
      startLocalDate: "2026-07-31",
      endLocalDate: "2026-08-06",
      startAt: 1,
      endAt: 2,
      bucketGranularity: range === "7d" || range === "30d" ? "day" : "week",
      deckIds: [],
      tagIds: [],
      highlightedSessionId: null,
    },
    currentInventory: {
      currentDeckCount: 1,
      sourceNoteCount: 10,
      generatedCardCount: 12,
      studyableCardCount: 12,
      studyableDueCount: 4,
      needsMorePracticeCount: 2,
    },
    history: {
      cardsPracticed,
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
        startLocalDate: "2026-08-01",
        endLocalDate: "2026-08-01",
        startAt: 1,
        endAt: 2,
        cardsPracticed,
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
        isArchived: false,
        sourceNoteCount: 10,
        generatedCardCount: 12,
        studyableCardCount: 12,
        studyableDueCount: 4,
        needsMorePracticeCount: 2,
        cardsPracticed,
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
        tagId,
        tagName: "Arrhythmias",
        isCurrent: true,
        sourceNoteCount: 5,
        generatedCardCount: 6,
        studyableCardCount: 6,
        studyableDueCount: 3,
        needsMorePracticeCount: 2,
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
        capturedDeckName: "Cardiology",
        cardId: "d".repeat(32),
        cardLabel: "Basic",
        questionExcerpt: "What rhythm has no P waves?",
        tags: ["Arrhythmias"],
        outcome: "somewhat_confident",
        rating: 3,
        responseTimeMs: 4000,
        reviewedAt: 1,
        reviewLocalDate: "2026-08-01",
        dueAfter: Date.now() + 86_400_000,
        attributionSource: "captured",
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
    attributionCoverage: {
      capturedReviewCount: 10,
      legacyBackfilledReviewCount: 0,
    },
    dimensionOptions: {
      decks: [
        {
          id: deck.id,
          name: deck.name,
          isArchived: false,
          hasCurrentCards: true,
          hasHistory: true,
        },
      ],
      tags: [
        {
          id: tagId,
          name: "Arrhythmias",
          isCurrent: true,
          hasHistory: true,
        },
      ],
    },
  };
}

function renderAnalytics(onPracticeScope = vi.fn()) {
  render(
    <AnalyticsScreen
      decks={[deck]}
      settings={settings}
      launchRequest={null}
      onLaunchConsumed={vi.fn()}
      onPracticeScope={onPracticeScope}
      showToast={vi.fn()}
    />,
  );
  return onPracticeScope;
}

describe("AnalyticsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(analyticsIpc.getAnalytics).mockResolvedValue(analyticsForRange("7d"));
  });

  it("starts with the past week and shows an accessible data alternative", async () => {
    renderAnalytics();

    expect(await screen.findByText("Cards practiced")).toBeTruthy();
    await waitFor(() => {
      expect(analyticsIpc.getAnalytics).toHaveBeenCalledWith({
        range: "7d",
        deckIds: [],
        tagIds: [],
        highlightedSessionId: undefined,
      });
    });
    expect(
      screen.getByRole("button", { name: "7 days" }).getAttribute("aria-pressed"),
    ).toBe("true");
    expect(screen.getByText("8")).toBeTruthy();
    expect(screen.getAllByText("80%").length).toBeGreaterThan(0);
    expect(screen.getAllByText("2 min").length).toBeGreaterThan(0);
    expect(screen.getByRole("img", { name: "Learning progress chart" })).toBeTruthy();
    expect(screen.getAllByText("Arrhythmias").length).toBeGreaterThan(0);
    expect(screen.getByText("Needs attention")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "View chart data" }));
    expect(
      screen.getByRole("table", {
        name: "Data represented in the learning progress chart",
      }),
    ).toBeTruthy();
  });

  it("changes the range and starts a scoped ready session for a weak tag", async () => {
    const onPracticeScope = renderAnalytics();

    await screen.findAllByText("Arrhythmias");
    fireEvent.click(screen.getByRole("button", { name: "90 days" }));
    await waitFor(() => {
      expect(analyticsIpc.getAnalytics).toHaveBeenLastCalledWith(
        expect.objectContaining({ range: "90d" }),
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Practice ready" }));
    expect(onPracticeScope).toHaveBeenCalledWith([], [tagId]);
  });

  it("discards a slower response for an obsolete range", async () => {
    const first = deferred<analyticsIpc.Analytics>();
    const second = deferred<analyticsIpc.Analytics>();
    vi.mocked(analyticsIpc.getAnalytics)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    renderAnalytics();

    fireEvent.click(screen.getByRole("button", { name: "90 days" }));
    await waitFor(() => expect(analyticsIpc.getAnalytics).toHaveBeenCalledTimes(2));

    second.resolve(analyticsForRange("90d", 90));
    expect(await screen.findByText("90")).toBeTruthy();
    first.resolve(analyticsForRange("7d", 7));
    await Promise.resolve();
    expect(screen.queryByText("7")).toBeNull();
    expect(screen.getByText("90")).toBeTruthy();
  });
});
