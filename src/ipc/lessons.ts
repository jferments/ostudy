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

/** Where the learner stopped. Finishing a lesson is not a review. */
export interface LessonProgress {
  /** May be older than the lesson's latest revision. */
  revisionNumber: number;
  currentStep: number;
  furthestStep: number;
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
  /** A check's kind; the question itself stays native until it is answered. */
  checkKind: QuestionKind | null;
  knowledgeUnitIds: string[];
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
