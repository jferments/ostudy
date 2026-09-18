import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  addChoiceOption,
  addGroupingCategory,
  definitionFromDraft,
  moveChoiceOption,
  moveOrderingItem,
  newQuestionDraft,
  questionDraftReady,
  removeChoiceOption,
  removeGroupingCategory,
  type QuestionEditorDraft,
} from "./questionDraft";

type ChoiceDraft = Extract<QuestionEditorDraft, { kind: "single_choice" | "select_all" }>;
type GroupingDraft = Extract<QuestionEditorDraft, { kind: "grouping" }>;
type OrderingDraft = Extract<QuestionEditorDraft, { kind: "ordering" }>;

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

  it("requires every grouping item to target an authored category and preserves that relation", () => {
    const draft = newQuestionDraft("grouping") as GroupingDraft;
    const filled: GroupingDraft = {
      ...draft,
      promptText: "Sort the findings",
      categories: draft.categories.map((category, index) => ({ ...category, text: `Group ${index + 1}` })),
      items: draft.items.map((item, index) => ({
        ...item,
        text: `Finding ${index + 1}`,
        categoryId: draft.categories[index % draft.categories.length].id,
      })),
    };
    expect(questionDraftReady({ ...filled, items: filled.items.map((item) => ({ ...item, categoryId: "" })) })).toBe(false);
    expect(questionDraftReady(filled)).toBe(true);
    const definition = definitionFromDraft(filled);
    expect(definition.question.kind).toBe("grouping");
    if (definition.question.kind === "grouping") {
      expect(definition.question.items.map((item) => item.categoryId)).toEqual(
        filled.items.map((item) => item.categoryId),
      );
    }
  });

  it("clears grouping assignments when their category is removed rather than silently reassigning them", () => {
    const draft = newQuestionDraft("grouping") as GroupingDraft;
    const expanded = addGroupingCategory(draft) as GroupingDraft;
    const removedId = expanded.categories[1].id;
    const assigned: GroupingDraft = {
      ...expanded,
      items: expanded.items.map((item) => ({ ...item, categoryId: removedId })),
    };
    const removed = removeGroupingCategory(assigned, removedId) as GroupingDraft;
    expect(removed.categories.some((category) => category.id === removedId)).toBe(false);
    expect(removed.items.every((item) => item.categoryId === "")).toBe(true);
  });

  it("uses authored ordering row position as the total-order answer while preserving item identity", () => {
    const draft = newQuestionDraft("ordering") as OrderingDraft;
    const firstId = draft.items[0].id;
    const secondId = draft.items[1].id;
    const moved = moveOrderingItem(draft, secondId, -1) as OrderingDraft;
    expect(moved.items.map((item) => item.id)).toEqual([secondId, firstId]);
    const definition = definitionFromDraft(moved);
    expect(definition.question.kind).toBe("ordering");
    if (definition.question.kind === "ordering") {
      expect(definition.question.items.map((item) => item.id)).toEqual([secondId, firstId]);
    }
  });
});
