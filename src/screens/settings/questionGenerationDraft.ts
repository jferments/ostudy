import type { QuestionFamily, QuestionGenerationPolicy } from "../../ipc/questionGeneration";

export const GENERATION_FAMILIES: ReadonlyArray<{ value: QuestionFamily; label: string }> = [
  { value: "recall", label: "Basic and fill in the blank" },
  { value: "single_choice", label: "Single best answer" },
  { value: "select_all", label: "Select all that apply" },
  { value: "true_false", label: "True or false" },
  { value: "matching", label: "Matching" },
  { value: "grouping", label: "Grouping" },
  { value: "ordering", label: "Ordering" },
];

export interface QuestionGenerationDraft {
  allowedTypes: QuestionFamily[];
  weights: Record<QuestionFamily, string>;
  maxCompoundItems: string;
}

export function recommendedQuestionMix(): QuestionGenerationPolicy {
  return {
    allowedTypes: GENERATION_FAMILIES.map(({ value }) => value),
    weights: { recall: 50, single_choice: 10, select_all: 10, true_false: 2, matching: 15, grouping: 10, ordering: 3 },
    maxCompoundItems: 8,
  };
}

export function toQuestionGenerationDraft(policy: QuestionGenerationPolicy): QuestionGenerationDraft {
  return {
    allowedTypes: GENERATION_FAMILIES.filter(({ value }) => policy.allowedTypes.includes(value)).map(({ value }) => value),
    weights: Object.fromEntries(GENERATION_FAMILIES.map(({ value }) => [value, String(policy.weights[value] ?? 0)])) as Record<QuestionFamily, string>,
    maxCompoundItems: String(policy.maxCompoundItems),
  };
}

export function parseQuestionGenerationDraft(draft: QuestionGenerationDraft): QuestionGenerationPolicy | null {
  const maxCompoundItems = integer(draft.maxCompoundItems, 2, 16);
  if (maxCompoundItems === null || draft.allowedTypes.length === 0) return null;
  const weights: Partial<Record<QuestionFamily, number>> = {};
  for (const { value } of GENERATION_FAMILIES) {
    const weight = integer(draft.weights[value], 0, 1_000);
    if (weight === null) return null;
    weights[value] = weight;
  }
  if (!draft.allowedTypes.some((value) => (weights[value] ?? 0) > 0)) return null;
  return { allowedTypes: [...draft.allowedTypes], weights, maxCompoundItems };
}

function integer(value: string, minimum: number, maximum: number): number | null {
  if (!/^\d+$/u.test(value)) return null;
  const number = Number(value);
  return Number.isSafeInteger(number) && number >= minimum && number <= maximum ? number : null;
}
