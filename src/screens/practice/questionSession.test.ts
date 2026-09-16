import { describe, expect, it } from "vitest";

import type { PracticeCard } from "../../ipc/practice";
import type { ImageOcclusionAttemptView, PracticeQuestion } from "../../ipc/questionAttempts";
import { mergePracticeQuestion, mergeQuestionAttempt } from "./questionSession";

function attempt(instanceId: string, revealed = false): ImageOcclusionAttemptView {
  const document = { type: "doc", content: [{ type: "paragraph" }] };
  return {
    instanceId, revision: revealed ? 2 : 1, assisted: false,
    question: { kind: "image_occlusion", prompt: document, targetRectangles: [{ x: 0, y: 0, width: 1, height: 1 }] },
    progress: revealed
      ? { phase: "revealed", revealId: "9".repeat(32), responseTimeMs: 500, revealedAtMs: 2000, answer: document, explanation: null }
      : { phase: "answering", savedAnswer: null },
  };
}
function card(instanceId: string): PracticeCard {
  return {
    deckName: "Test", dueAt: 1, scheduleState: 0, needsMorePractice: false,
    presentation: { cardId: instanceId, noteId: "2".repeat(32), deckId: "3".repeat(32), direction: "forward",
      frontText: "Identify", createdAt: 1, updatedAt: 1, noteRevision: 1,
      question: { attempt: attempt(instanceId), assets: [] } },
  };
}
function question(value: PracticeCard): PracticeQuestion {
  if (!("question" in value.presentation)) throw new Error("Expected a question");
  return value.presentation.question;
}

describe("captured question acknowledgement merging", () => {
  it("a late reveal updates its instance rather than the new active array position", () => {
    const first = card("a".repeat(32));
    const second = card("b".repeat(32));
    const reordered = [second, first];
    const next = mergeQuestionAttempt(reordered, attempt("a".repeat(32), true));
    expect(next[0]).toBe(second);
    expect(question(next[1]).attempt.progress.phase).toBe("revealed");
    expect(question(first).attempt.progress.phase).toBe("answering");
  });

  it("foreign, duplicate and older acknowledgements leave the existing array unchanged", () => {
    const cards = [card("a".repeat(32))];
    expect(mergeQuestionAttempt(cards, attempt("b".repeat(32), true))).toBe(cards);
    expect(mergeQuestionAttempt(cards, attempt("a".repeat(32)))).toBe(cards);
    const exposed = mergeQuestionAttempt(cards, attempt("a".repeat(32), true));
    expect(mergeQuestionAttempt(exposed, attempt("a".repeat(32)))).toBe(exposed);
  });

  it("same-revision media reconciliation preserves the revealed phase", () => {
    const cards = mergeQuestionAttempt([card("a".repeat(32))], attempt("a".repeat(32), true));
    const media = { id: "4".repeat(32), originalFilename: "answer.png", mimeType: "image/png", byteSize: 1, createdAt: 1 };
    const next = mergePracticeQuestion(cards, { attempt: attempt("a".repeat(32), true), assets: [media] });
    expect(question(next[0]).assets).toEqual([media]);
    expect(question(next[0]).attempt.progress.phase).toBe("revealed");
    const stale = { ...attempt("a".repeat(32)), revision: 2 };
    expect(mergePracticeQuestion(next, { attempt: stale, assets: [] })).toBe(next);
    expect(mergePracticeQuestion(next, { attempt: attempt("a".repeat(32)), assets: [] })).toBe(next);
  });
});
