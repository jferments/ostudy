import { invoke } from "@tauri-apps/api/core";

import type { Asset } from "./assets";
import type { RichDocument } from "./common";
import type { QuestionItem } from "./questions";

/** Public study data is deliberately distinct from authored definitions/keys. */
export type PublicQuestion =
  | { kind: "single_choice"; prompt: RichDocument; options: QuestionItem[] }
  | { kind: "select_all"; prompt: RichDocument; options: QuestionItem[] }
  | { kind: "true_false"; statement: RichDocument }
  | { kind: "matching"; prompt: RichDocument; items: QuestionItem[]; targets: QuestionItem[] }
  | { kind: "grouping"; prompt: RichDocument; items: QuestionItem[]; categories: QuestionItem[] }
  | { kind: "ordering"; prompt: RichDocument; items: QuestionItem[] };

export type QuestionResponse =
  | { kind: "single_choice"; selectedOptionId: string | null }
  | { kind: "select_all"; selectedOptionIds: string[] }
  | { kind: "true_false"; selected: boolean | null }
  | { kind: "matching"; assignments: { itemId: string; targetId: string }[] }
  | { kind: "grouping"; assignments: { itemId: string; targetId: string }[] }
  | { kind: "ordering"; orderedItemIds: string[] };

export interface LearnerAnswer {
  schemaVersion: 1;
  instanceId: string;
  response: QuestionResponse;
}

export type ComponentFeedback =
  | { kind: "choice"; itemId: string; selected: boolean; expected: boolean; rationale: RichDocument | null }
  | { kind: "boolean"; selected: boolean | null; expected: boolean; correctedStatement: RichDocument | null }
  | { kind: "placement"; itemId: string; selectedTargetId: string | null; expectedTargetId: string; rationale: RichDocument | null }
  | { kind: "position"; itemId: string; selectedPosition: number | null; expectedPosition: number };

export interface QuestionEvaluation {
  gradingPolicyVersion: 1;
  verdict: "correct" | "incorrect" | "incomplete";
  matchedCount: number;
  requiredCount: number;
  components: ComponentFeedback[];
  explanation: RichDocument | null;
}

export interface QuestionAttemptView {
  instanceId: string;
  revision: number;
  assisted: boolean;
  question: PublicQuestion;
  progress:
    | { phase: "answering"; savedAnswer: LearnerAnswer | null }
    | {
        phase: "checked";
        checkId: string;
        firstAnswer: LearnerAnswer;
        responseTimeMs: number;
        checkedAtMs: number;
        evaluation: QuestionEvaluation;
      };
}

export interface QuestionAttemptTarget {
  sessionId: string;
  cardId: string;
  instanceId: string;
}

export interface SavePracticeAnswerInput {
  target: QuestionAttemptTarget;
  request: { mutationId: string; expectedRevision: number; answer: LearnerAnswer };
}

export interface CheckPracticeAnswerInput {
  target: QuestionAttemptTarget;
  request: {
    checkId: string;
    expectedRevision: number;
    answer: LearnerAnswer;
    responseTimeMs: number;
  };
}

export interface PracticeQuestion {
  attempt: QuestionAttemptView;
  assets: Asset[];
}

export function getPracticeQuestionAttempt(input: QuestionAttemptTarget): Promise<PracticeQuestion> {
  return invoke<PracticeQuestion>("get_practice_question_attempt", { input });
}

/** A successful response means committed. Asset reconciliation is a separate read. */
export function savePracticeQuestionAnswer(input: SavePracticeAnswerInput): Promise<QuestionAttemptView> {
  return invoke<QuestionAttemptView>("save_practice_question_answer", { input });
}

export function checkPracticeQuestionAnswer(input: CheckPracticeAnswerInput): Promise<QuestionAttemptView> {
  return invoke<QuestionAttemptView>("check_practice_question_answer", { input });
}
