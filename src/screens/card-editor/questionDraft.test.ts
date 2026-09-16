import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  addChoiceOption,
  definitionFromDraft,
  moveChoiceOption,
  newQuestionDraft,
  questionDraftReady,
  removeChoiceOption,
  type QuestionEditorDraft,
} from "./questionDraft";

type ChoiceDraft = Extract<QuestionEditorDraft, { kind: "single_choice" | "select_all" }>;

let nextId = 0;

beforeEach(() => {
  nextId = 0;
  vi.spyOn(crypto, "randomUUID").mockImplementation(() => {
    nextId += 1;
    return `${nextId.toString(16).padStart(8, "0")}-0000-0000-0000-000000000000` as `${string}-${string}-${string}-${string}-${string}`;
  });
});

describe("interactive question drafts", () => {
  it("keeps option identity and the answer key attached while rows move", () => {
    const original = newQuestionDraft("single_choice") as ChoiceDraft;
    const expanded = addChoiceOption(original) as ChoiceDraft;
    const keyed: ChoiceDraft = { ...expanded, correctOptionIds: [expanded.options[1].id] };
    const moved = moveChoiceOption(keyed, expanded.options[1].id, -1) as ChoiceDraft;
    expect(moved.options[0].id).toBe(expanded.options[1].id);
    expect(moved.correctOptionIds).toEqual([expanded.options[1].id]);
    const removed = removeChoiceOption(moved, expanded.options[2].id) as ChoiceDraft;
    expect(removed.correctOptionIds).toEqual([expanded.options[1].id]);
  });

  it("distinguishes an unfinished select-all key from an intentionally empty key", () => {
    const draft = newQuestionDraft("select_all") as ChoiceDraft;
    const filled: ChoiceDraft = {
      ...draft,
      promptText: "Select all that apply",
      options: draft.options.map((option, index) => ({ ...option, text: `Option ${index + 1}` })),
    };
    expect(questionDraftReady(filled)).toBe(false);
    const confirmed: ChoiceDraft = { ...filled, emptySetConfirmed: true };
    expect(questionDraftReady(confirmed)).toBe(true);
    const definition = definitionFromDraft(confirmed);
    expect(definition.question.kind).toBe("select_all");
    if (definition.question.kind === "select_all") expect(definition.question.correctOptionIds).toEqual([]);
  });

  it("does not treat the default false-looking state as an authored false answer", () => {
    const draft = newQuestionDraft("true_false") as Extract<QuestionEditorDraft, { kind: "true_false" }>;
    expect(draft.correct).toBeNull();
    expect(questionDraftReady({ ...draft, statementText: "The statement" })).toBe(false);
    expect(questionDraftReady({ ...draft, statementText: "The statement", correct: false })).toBe(true);
  });

  it("creates independent stable identities for matching left and right items", () => {
    const draft = newQuestionDraft("matching") as Extract<QuestionEditorDraft, { kind: "matching" }>;
    expect(draft.pairs).toHaveLength(2);
    expect(new Set(draft.pairs.flatMap((pair) => [pair.left.id, pair.right.id])).size).toBe(4);
    expect(draft.pairs.every((pair) => pair.id === pair.left.id)).toBe(true);
  });
});
