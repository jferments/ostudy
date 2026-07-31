import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { PracticeScreen } from "./PracticeScreen";

vi.mock("../ipc", () => ({
  completePracticeSession: vi.fn(),
  createSubmissionId: () => "submission-id",
  getAnalytics: vi.fn(),
  getErrorCode: vi.fn(),
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
  markPracticeCardRevealed: vi.fn(),
  markPracticeCardViewed: vi.fn(),
  previewReviewIntervals: vi.fn(),
  startStudy: vi.fn(),
  submitStudyReview: vi.fn(),
  undoLastStudyReview: vi.fn(),
}));

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
  newCardsPerDay: 20,
  reviewsPerDay: 200,
  strugglingCardThreshold: 8,
  autoSuspendStrugglingCards: false,
  buryRelatedCards: true,
};

const decks: ipc.Deck[] = [
  {
    id: "10000000000000000000000000000001",
    name: "Dog facts",
    description: "",
    cardCount: 2,
    dueCount: 1,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  },
];

const analytics: ipc.Analytics = {
  deckCount: 1,
  cardCount: 2,
  dueNow: 1,
  reviewCount: 4,
  correctCount: 3,
  correctRate: 0.75,
  averageResponseTimeMs: 1200,
  strugglingCount: 0,
  suspendedCount: 0,
  ratingDistribution: { wrong: 1, hard: 1, ok: 1, easy: 1 },
  deckStats: [],
  recentDeckPractice: [
    {
      deckId: decks[0].id,
      deckName: "Dog facts",
      sessionId: "20000000000000000000000000000001",
      practicedAt: Date.now() - 60_000,
      reviewCount: 4,
      ratingDistribution: { wrong: 1, hard: 1, ok: 1, easy: 1 },
    },
  ],
  recentReviews: [],
};

describe("PracticeScreen setup", () => {
  beforeEach(() => {
    vi.mocked(ipc.getAnalytics).mockResolvedValue(analytics);
  });

  it("uses a compact Shuffle decks checkbox and shows recent deck confidence", async () => {
    const { container } = render(
      <PracticeScreen
        decks={decks}
        settings={settings}
        launchRequest={null}
        onLaunchConsumed={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    const shuffle = screen.getByRole("checkbox", { name: "Shuffle decks" });
    expect((shuffle as HTMLInputElement).checked).toBe(true);
    expect(shuffle.closest("label")?.classList.contains("compact-checkbox")).toBe(true);
    expect(container.querySelector(".practice-options-card")).toBeNull();

    expect(await screen.findByRole("heading", { name: "Recently practiced" })).toBeTruthy();
    expect(screen.getByText("Wrong 1")).toBeTruthy();
    expect(screen.getByText("Hard 1")).toBeTruthy();
    expect(screen.getByText("OK 1")).toBeTruthy();
    expect(screen.getByText("Easy 1")).toBeTruthy();
  });

  it("selects a recently practiced deck for the next session", async () => {
    render(
      <PracticeScreen
        decks={decks}
        settings={settings}
        launchRequest={null}
        onLaunchConsumed={vi.fn()}
        showToast={vi.fn()}
      />,
    );

    fireEvent.click(await screen.findByRole("button", { name: "Select Dog facts for practice" }));

    await waitFor(() => {
      const deckCheckbox = screen
        .getAllByRole("checkbox")
        .find((checkbox) => checkbox !== screen.getByRole("checkbox", { name: "Shuffle decks" }));
      expect((deckCheckbox as HTMLInputElement | undefined)?.checked).toBe(true);
      expect(screen.getByText("1 selected")).toBeTruthy();
    });
  });
});
