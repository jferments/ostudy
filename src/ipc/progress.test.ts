import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  getProgressActivity,
  getProgressCardStrength,
  getProgressForecast,
  getProgressOverview,
  getProgressQuestionTypes,
  getProgressSessions,
  getProgressSubjects,
  getReviewLog,
  type ProgressQuery,
} from "./progress";

vi.mock("./transport", () => ({
  invoke: vi.fn(),
}));

const input: ProgressQuery = {
  range: "30d",
  subjectIds: ["a".repeat(32)],
  deckIds: [],
  tagIds: ["b".repeat(32)],
};

describe("progress IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({});
  });

  it("sends the shared scope to each section's own command", async () => {
    await getProgressOverview(input);
    await getProgressForecast(input);
    await getProgressCardStrength(input);
    await getProgressSubjects(input);
    await getProgressQuestionTypes(input);
    await getProgressSessions(input);
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["get_progress_overview", { input }],
      ["get_progress_forecast", { input }],
      ["get_progress_card_strength", { input }],
      ["get_progress_subjects", { input }],
      ["get_progress_question_types", { input }],
      ["get_progress_sessions", { input }],
    ]);
  });

  it("reads the library-wide calendar without a scope", async () => {
    await getProgressActivity();
    expect(invoke).toHaveBeenCalledWith("get_progress_activity");
  });

  it("pages the review log with the previous page's cursor", async () => {
    const before = { reviewedAt: 2, createdAt: 3, id: "c".repeat(32) };
    await getReviewLog(input);
    await getReviewLog(input, before);
    expect(vi.mocked(invoke).mock.calls).toEqual([
      ["get_review_log", { input, before: null }],
      ["get_review_log", { input, before }],
    ]);
  });
});
