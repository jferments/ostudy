import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  checkPracticeQuestionAnswer,
  getPracticeQuestionAttempt,
  savePracticeQuestionAnswer,
  type CheckPracticeAnswerInput,
  type LearnerAnswer,
  type QuestionAttemptTarget,
  type QuestionAttemptView,
  type SavePracticeAnswerInput,
} from "./questionAttempts";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

const target: QuestionAttemptTarget = {
  sessionId: "a".repeat(32),
  cardId: "b".repeat(32),
  instanceId: "c".repeat(32),
};
const rich = (text: string) => ({
  type: "doc",
  content: [{ type: "paragraph", content: [{ type: "text", text }] }],
});
const answer: LearnerAnswer = {
  schemaVersion: 1,
  instanceId: target.instanceId,
  response: { kind: "select_all", selectedOptionIds: [] },
};
const view: QuestionAttemptView = {
  instanceId: target.instanceId,
  revision: 2,
  assisted: false,
  question: {
    kind: "select_all",
    prompt: rich("Select all matching options."),
    options: [
      { id: "e".repeat(32), content: rich("First option") },
      { id: "f".repeat(32), content: rich("Second option") },
    ],
  },
  progress: { phase: "answering", savedAnswer: answer },
};

beforeEach(() => {
  invoke.mockReset();
});

describe("Practice question gateways", () => {
  it("preserves an explicit empty selection and the caller's mutation identity", async () => {
    const input: SavePracticeAnswerInput = {
      target,
      request: { mutationId: "d".repeat(32), expectedRevision: 1, answer },
    };
    invoke.mockResolvedValue(view);
    await expect(savePracticeQuestionAnswer(input)).resolves.toBe(view);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("save_practice_question_answer", { input });
    expect(input.request.answer.response).toEqual({ kind: "select_all", selectedOptionIds: [] });
    expect(input.request.mutationId).toBe("d".repeat(32));
  });

  it("retries a check unchanged and does not append a fallible read to committed success", async () => {
    const input: CheckPracticeAnswerInput = {
      target,
      request: { checkId: "d".repeat(32), expectedRevision: 2, answer, responseTimeMs: 100 },
    };
    const checked: QuestionAttemptView = {
      ...view,
      revision: 3,
      progress: {
        phase: "checked",
        checkId: input.request.checkId,
        firstAnswer: answer,
        responseTimeMs: 100,
        checkedAtMs: 1000,
        evaluation: {
          gradingPolicyVersion: 1,
          verdict: "correct",
          matchedCount: 0,
          requiredCount: 0,
          components: [
            { kind: "choice", itemId: "e".repeat(32), selected: false, expected: false, rationale: null },
            { kind: "choice", itemId: "f".repeat(32), selected: false, expected: false, rationale: null },
          ],
          explanation: null,
        },
      },
    };
    invoke.mockResolvedValue(checked);
    await expect(checkPracticeQuestionAnswer(input)).resolves.toBe(checked);
    await expect(checkPracticeQuestionAnswer(input)).resolves.toBe(checked);
    expect(invoke.mock.calls).toEqual([
      ["check_practice_question_answer", { input }],
      ["check_practice_question_answer", { input }],
    ]);
  });

  it("propagates a recovery-read failure without inventing another mutation", async () => {
    const failure = { code: "question_asset_unavailable", message: "Saved answer is unchanged." };
    invoke.mockRejectedValue(failure);
    await expect(getPracticeQuestionAttempt(target)).rejects.toBe(failure);
    expect(invoke).toHaveBeenCalledExactlyOnceWith("get_practice_question_attempt", { input: target });
  });
});
