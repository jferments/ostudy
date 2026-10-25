import { beforeEach, describe, expect, it, vi } from "vitest";

import { getHomeSummary, getQuestionOfTheDay } from "./home";
import { invoke } from "./transport";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

describe("home IPC", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("reads the Home summary through one typed native command", async () => {
    const summary = { studyDate: "2026-10-05" };
    vi.mocked(invoke).mockResolvedValue(summary);

    await expect(getHomeSummary()).resolves.toBe(summary);
    expect(invoke).toHaveBeenCalledWith("get_home_summary");
  });

  it("reads the question of the day through its own native command", async () => {
    vi.mocked(invoke).mockResolvedValue(null);

    await expect(getQuestionOfTheDay()).resolves.toBeNull();
    expect(invoke).toHaveBeenCalledWith("get_question_of_the_day");
  });
});
