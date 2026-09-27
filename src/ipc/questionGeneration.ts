import { invoke } from "./transport";

export type QuestionFamily =
  | "recall"
  | "single_choice"
  | "select_all"
  | "true_false"
  | "matching"
  | "grouping"
  | "ordering";

export interface QuestionGenerationPolicy {
  allowedTypes: QuestionFamily[];
  weights: Partial<Record<QuestionFamily, number>>;
  maxCompoundItems: number;
}

export interface QuestionGenerationSnapshot {
  revision: number;
  policy: QuestionGenerationPolicy;
}

export function getQuestionGenerationPolicy(): Promise<QuestionGenerationSnapshot> {
  return invoke<QuestionGenerationSnapshot>("get_question_generation_policy");
}

export function saveQuestionGenerationPolicy(
  input: QuestionGenerationSnapshot,
): Promise<QuestionGenerationSnapshot> {
  return invoke<QuestionGenerationSnapshot>("save_question_generation_policy", { input });
}
