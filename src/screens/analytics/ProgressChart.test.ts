import { Chart as ChartJS } from "chart.js";
import { describe, expect, it } from "vitest";

import type { TimelineBucket } from "../../ipc/analytics";
import "./ProgressChart";
import { buildDifficultyTimelineSeries } from "./difficultySeries";

function bucket(overrides: Partial<TimelineBucket> = {}): TimelineBucket {
  return {
    startLocalDate: "2026-08-11",
    endLocalDate: "2026-08-11",
    startAt: 1,
    endAt: 2,
    cardsPracticed: 8,
    reviewCount: 10,
    newCards: 2,
    rememberedCount: 8,
    wrongCount: 2,
    hardCount: 1,
    mediumCount: 4,
    easyCount: 3,
    correctRate: 0.8,
    averageConfidence: 1.8,
    averageResponseTimeMs: 4000,
    studyTimeMs: 40_000,
    sessionCount: 1,
    containsHighlightedSession: false,
    ...overrides,
  };
}

describe("ProgressChart", () => {
  it("registers the line controller used by the progress chart", () => {
    expect(() => ChartJS.registry.getController("line")).not.toThrow();
  });

  it("keeps total reviews and all four practice difficulty levels as separate series", () => {
    const series = buildDifficultyTimelineSeries([
      bucket(),
      bucket({
        reviewCount: 7,
        wrongCount: 1,
        hardCount: 2,
        mediumCount: 3,
        easyCount: 1,
      }),
    ]);

    expect(series.reviewed).toEqual([10, 7]);
    expect(series.wrong).toEqual([2, 1]);
    expect(series.hard).toEqual([1, 2]);
    expect(series.medium).toEqual([4, 3]);
    expect(series.easy).toEqual([3, 1]);
  });
});
