import { invoke } from "./transport";

import type { Asset } from "./assets";
import type { RichDocument } from "./common";
import type { NormalizedRect, QuestionItem } from "./questions";
import type { ReviewResult, SubmitReviewInput } from "./practice";

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

/** Self-assessed targets have reveal evidence, never a made-up objective response. */
export interface ImageOcclusionAttemptView {
  instanceId: string;
  revision: number;
  assisted: boolean;
  question: { kind: "image_occlusion"; prompt: RichDocument; targetRectangles: NormalizedRect[] };
  progress:
    | { phase: "answering"; savedAnswer: null }
    | {
        phase: "revealed";
        revealId: string;
        responseTimeMs: number;
        revealedAtMs: number;
        answer: RichDocument;
        explanation: RichDocument | null;
      };
}

export type PracticeAttemptView = QuestionAttemptView | ImageOcclusionAttemptView;

export function isImageOcclusionAttempt(attempt: PracticeAttemptView): attempt is ImageOcclusionAttemptView {
  return attempt.question.kind === "image_occlusion";
}

export interface RevealPracticeQuestionInput {
  target: QuestionAttemptTarget;
  request: { revealId: string; expectedRevision: number; responseTimeMs: number };
}

export interface ReadPracticeOcclusionInput {
  target: QuestionAttemptTarget;
  expectedRevision: number;
  maximumEdge: number;
}

export function revealPracticeQuestionAnswer(input: RevealPracticeQuestionInput): Promise<ImageOcclusionAttemptView> {
  return invoke<ImageOcclusionAttemptView>("reveal_practice_question_answer", { input });
}

/** Only native-authorized PNG bytes cross this route; no source path or phase Boolean. */
export function readPracticeOcclusionImage(input: ReadPracticeOcclusionInput): Promise<ArrayBuffer> {
  return invoke<ArrayBuffer>("read_practice_occlusion_image", { input });
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
  attempt: PracticeAttemptView;
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

export interface QuestionReviewContext {
  instanceId: string;
  attemptRevision: number;
  overrideReason: string | null;
}

export function submitPracticeQuestionReview(input: {
  review: SubmitReviewInput;
  question: QuestionReviewContext;
}): Promise<ReviewResult> {
  return invoke<ReviewResult>("submit_practice_question_review", { input });
}

export interface QuestionRemediationReceipt {
  operationId: string;
  instanceId: string;
  evaluation: QuestionEvaluation;
  createdAt: number;
}

export function remediatePracticeQuestion(input: {
  target: QuestionAttemptTarget;
  operationId: string;
  answer: LearnerAnswer;
}): Promise<QuestionRemediationReceipt> {
  return invoke<QuestionRemediationReceipt>("remediate_practice_question", { input });
}
