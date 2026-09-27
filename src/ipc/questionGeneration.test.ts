import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "./transport";
import { getQuestionGenerationPolicy, saveQuestionGenerationPolicy } from "./questionGeneration";

vi.mock("./transport", () => ({ invoke: vi.fn() }));
const mockedInvoke = vi.mocked(invoke);

beforeEach(() => { mockedInvoke.mockReset(); });

describe("question generation IPC", () => {
  it("reads saved native preferences without starting generation", async () => {
    const saved = { revision: 1, policy: { allowedTypes: ["recall"], weights: { recall: 100 }, maxCompoundItems: 8 } };
    mockedInvoke.mockResolvedValue(saved);
    expect(await getQuestionGenerationPolicy()).toBe(saved);
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
    expect(mockedInvoke).toHaveBeenCalledWith("get_question_generation_policy");
  });

  it("saves the expected revision and lets native conflicts propagate", async () => {
    const failure = new Error("Preferences changed in another window.");
    mockedInvoke.mockRejectedValue(failure);
    const input = {
      revision: 7,
      policy: { allowedTypes: ["matching" as const], weights: { matching: 100 }, maxCompoundItems: 4 },
    };
    await expect(saveQuestionGenerationPolicy(input)).rejects.toBe(failure);
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
    expect(mockedInvoke).toHaveBeenCalledWith("save_question_generation_policy", { input });
  });
});
