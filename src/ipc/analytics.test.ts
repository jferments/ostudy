import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { getAnalytics, getRecentPracticeReviews } from "./analytics";

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
    vi.mocked(invoke).mockResolvedValue([]);

    await expect(getRecentPracticeReviews()).resolves.toEqual([]);
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
