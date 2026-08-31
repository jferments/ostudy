import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getAnalytics,
  getRecentPracticeReviews,
  type RecentReview,
} from "./analytics";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("analytics IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("sends the complete scoped analytics query", async () => {
    const input = {
      range: "90d" as const,
      deckIds: ["a".repeat(32), "b".repeat(32)],
      tagIds: ["c".repeat(32)],
      highlightedSessionId: "d".repeat(32),
    };
    vi.mocked(invoke).mockResolvedValue({ scope: input });

    await getAnalytics(input);

    expect(invoke).toHaveBeenCalledWith("get_analytics", { input });
  });

  it("uses the lightweight recent-review command for Practice", async () => {
    const reviews: RecentReview[] = [
      {
        id: "a".repeat(32),
        deckName: "Medication",
        capturedDeckName: "Medication",
        cardId: "c".repeat(32),
        cardLabel: "Basic",
        questionExcerpt: "Question",
        tags: [
          {
            tagId: "b".repeat(32),
            tagName: "Medication\u001fsafety",
            isCurrent: false,
          },
        ],
        outcome: "somewhat_confident",
        rating: 3,
        responseTimeMs: 1_000,
        reviewedAt: 2,
        reviewLocalDate: "2026-08-30",
        dueAfter: 3,
        attributionSource: "captured",
      },
    ];
    vi.mocked(invoke).mockResolvedValue(reviews);

    await expect(getRecentPracticeReviews()).resolves.toEqual(reviews);
    expect(invoke).toHaveBeenCalledWith("get_recent_practice_reviews");
  });

  it("preserves backend failures for the shared error translator", async () => {
    const error = { code: "invalid_analytics_range", message: "Invalid range." };
    vi.mocked(invoke).mockRejectedValue(error);

    await expect(
      getAnalytics({ range: "30d", deckIds: [], tagIds: [] }),
    ).rejects.toBe(error);
  });
});
