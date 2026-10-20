import { createOperationId } from "./common";
import type { QuestionKind } from "./questions";
import { invoke } from "./transport";

/** One teaching block of a lesson step; every field is plain text. */
export type LessonBlock =
  | { kind: "explanation"; heading: string | null; body: string }
  | { kind: "keyPoints"; items: string[] }
  | { kind: "term"; term: string; explanation: string }
  | { kind: "process"; title: string; steps: string[] }
  | {
      kind: "comparison";
      subjectA: string;
      subjectB: string;
      relationship: string;
    }
  | {
      kind: "callout";
      callout: "key_idea" | "warning" | "tip" | "note";
      title: string;
      body: string;
    }
  | {
      kind: "table";
      title: string;
      columns: string[];
      rows: string[][];
      caption: string | null;
    }
  | {
      kind: "workedExample";
      title: string;
      prompt: string;
      steps: string[];
      answer: string;
      takeaway: string | null;
    }
  | { kind: "misconception"; misconception: string; correction: string };

/**
 * How the knowledge a lesson revision explains compares with the library now:
 * `changed` when a unit's content changed since, `unavailable` when one is no
 * longer extracted.
 */
export type LessonSourceStatus = "current" | "changed" | "unavailable";

export interface WrittenLesson {
  revisionNumber: number;
  origin: "authored" | "generated";
  stepCount: number;
  checkCount: number;
  estimatedMinutes: number;
  sources: LessonSourceStatus;
  createdAt: number;
}

/** The learner's latest answer to one step's check. */
export interface AnsweredCheck {
  optionId: string;
  correct: boolean;
}

/** Where the learner stopped. Finishing a lesson is not a review. */
export interface LessonProgress {
  /** May be older than the lesson's latest revision. */
  revisionNumber: number;
  currentStep: number;
  furthestStep: number;
  /** By step key. */
  checks: Record<string, AnsweredCheck>;
  startedAt: number;
  updatedAt: number;
  finishedAt: number | null;
  revision: number;
}

export interface LessonSummary {
  id: string;
  subjectId: string;
  title: string;
  revision: number;
  /** The latest revision; null while the lesson is planned but unwritten. */
  written: WrittenLesson | null;
  progress: LessonProgress | null;
}

export interface SubjectLessons {
  subjectId: string;
  /** In course order. */
  lessons: LessonSummary[];
}

export interface LessonStep {
  key: string;
  title: string;
  blocks: LessonBlock[];
  narration: string;
  slidePoints: string[];
  checkKind: QuestionKind | null;
  /**
   * The check without its answer, when its kind can be answered in a lesson.
   * Which option is right stays native until the learner answers.
   */
  check: LessonCheck | null;
  knowledgeUnitIds: string[];
}

export interface LessonCheck {
  prompt: string;
  options: { id: string; text: string }[];
}

export interface LessonCheckResult {
  correct: boolean;
  correctOptionId: string;
  explanation: string | null;
  progress: LessonProgress;
}

export interface Lesson {
  lesson: LessonSummary;
  revisionNumber: number;
  sources: LessonSourceStatus;
  content: {
    title: string;
    summary: string;
    objectives: string[];
    estimatedMinutes: number;
    steps: LessonStep[];
  };
}

export async function listSubjectLessons(
  subjectId: string,
): Promise<SubjectLessons> {
  return invoke<SubjectLessons>("list_subject_lessons", {
    input: { subjectId },
  });
}

/** Reads the latest revision, or `revisionNumber` when given. */
export async function getLesson(
  lessonId: string,
  revisionNumber?: number,
): Promise<Lesson> {
  return invoke<Lesson>("get_lesson", {
    input: { lessonId, revisionNumber: revisionNumber ?? null },
  });
}

/**
 * Saves where the learner is in one revision. Saving the same state again
 * changes nothing; progress for a newer revision starts over on it.
 */
export async function saveLessonProgress(
  lessonId: string,
  revisionNumber: number,
  currentStep: number,
  finished: boolean,
): Promise<LessonProgress> {
  return invoke<LessonProgress>("save_lesson_progress", {
    input: { lessonId, revisionNumber, currentStep, finished },
  });
}

/**
 * Grades an answer to a step's check and records it in the learner's
 * progress. It never writes a review.
 */
export async function answerLessonCheck(
  lessonId: string,
  revisionNumber: number,
  stepKey: string,
  optionId: string,
): Promise<LessonCheckResult> {
  return invoke<LessonCheckResult>("answer_lesson_check", {
    input: { lessonId, revisionNumber, stepKey, optionId },
  });
}

/**
 * Writes the subject's course with the default document model: one lesson for
 * each top-level topic of its documents' learning plans, saved together when
 * every one is written. A subject that already has lessons, or a document
 * still being processed, is refused. Keep the operation id to stop the run
 * with `cancelSubjectCourseWriting`.
 */
export async function writeSubjectCourse(
  subjectId: string,
  operationId: string = createOperationId(),
): Promise<SubjectLessons> {
  return invoke<SubjectLessons>("write_subject_course", {
    input: { subjectId, operationId },
  });
}

/** Stops a course being written; nothing from the stopped run is saved. */
export async function cancelSubjectCourseWriting(
  operationId: string,
): Promise<void> {
  return invoke<void>("cancel_subject_course_writing", {
    input: { operationId },
  });
}

/** One kept piece of a step's spoken narration, in speaking order. */
export interface NarrationSegment {
  /** A managed audio asset. */
  assetId: string;
  characters: number;
}

export interface LessonStepNarration {
  segments: NarrationSegment[];
  /** How many pieces the voice spoke now; 0 when all were already kept. */
  made: number;
}

/**
 * Speaks a step's saved narration with the lesson voice and keeps the audio,
 * so listening again plays from the library without asking the voice.
 */
export async function narrateLessonStep(
  lessonId: string,
  revisionNumber: number,
  stepKey: string,
): Promise<LessonStepNarration> {
  return invoke<LessonStepNarration>("narrate_lesson_step", {
    input: { lessonId, revisionNumber, stepKey },
  });
}
