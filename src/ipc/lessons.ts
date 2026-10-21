import { createOperationId } from "./common";
import type { PreparedExport } from "./externalFiles";
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

export type LectureSlideKind = "title" | "step" | "sources";

export interface LectureSlide {
  /** `title`, `step:<step key>` or `sources`; a teacher's lecture is keyed by it. */
  key: string;
  kind: LectureSlideKind;
  /** The step a step slide shows; its narration can be spoken. */
  stepKey: string | null;
  title: string;
  points: string[];
  /** Speaker notes: a step's narration, or the lesson's summary. */
  notes: string;
}

/** A lesson laid out as slides, compiled natively from one saved revision. */
export interface Lecture {
  lessonId: string;
  revisionNumber: number;
  title: string;
  slides: LectureSlide[];
}

/** The lesson as a lecture: the latest revision, or `revisionNumber` when given. */
export async function getLessonLecture(
  lessonId: string,
  revisionNumber?: number,
): Promise<Lecture> {
  return invoke<Lecture>("get_lesson_lecture", {
    input: { lessonId, revisionNumber: revisionNumber ?? null },
  });
}

export interface PreparedLecturePptx {
  preparedExport: PreparedExport;
  /** The revision the deck was written from. */
  revisionNumber: number;
  slideCount: number;
}

/**
 * Writes the lesson's lecture as an editable PowerPoint deck, ready to
 * publish. Repeating `operationId` returns the deck already prepared.
 */
export async function prepareLessonLecturePptx(
  lessonId: string,
  revisionNumber?: number,
  operationId = createOperationId(),
): Promise<PreparedLecturePptx> {
  return invoke<PreparedLecturePptx>("prepare_lesson_lecture_pptx", {
    input: { lessonId, revisionNumber: revisionNumber ?? null, operationId },
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
 * Writes a lesson written with AI again from the part of the subject it
 * teaches, as its sources are now, and keeps it as the lesson's next
 * revision. Returns the subject's lessons.
 */
export async function updateLessonFromSources(
  lessonId: string,
  operationId: string = createOperationId(),
): Promise<SubjectLessons> {
  return invoke<SubjectLessons>("update_lesson_from_sources", {
    input: { lessonId, operationId },
  });
}

/**
 * Writes the subject's course with the default document model: one lesson for
 * each top-level topic of its documents' learning plans, saved together when
 * every one is written. A subject that already has lessons is refused unless
 * `rewrite`: then each lesson becomes the next revision of the lesson teaching
 * the same part, or a new lesson. A document still being processed is
 * refused. Keep the operation id to stop the run with
 * `cancelSubjectCourseWriting`.
 */
export async function writeSubjectCourse(
  subjectId: string,
  operationId: string = createOperationId(),
  rewrite = false,
): Promise<SubjectLessons> {
  return invoke<SubjectLessons>("write_subject_course", {
    input: { subjectId, operationId, rewrite },
  });
}

/** How far a course being written is; the course is saved only when complete. */
export interface CourseWritingProgress {
  /** The lessons the course will have; null while its parts are planned. */
  planned: number | null;
  written: number;
}

export async function getSubjectCourseWritingProgress(
  operationId: string,
): Promise<CourseWritingProgress> {
  return invoke<CourseWritingProgress>("get_subject_course_writing_progress", {
    input: { operationId },
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

/** A question the teacher asks the class; its answer stays native until answered. */
export interface LectureQuestion {
  prompt: string;
  options: { id: string; text: string }[];
}

export interface LectureScriptSlide {
  slideKey: string;
  /** What the teacher says over the slide. */
  talk: string;
  /** Asked after the talk, on a few step slides. */
  question: LectureQuestion | null;
}

/** The teacher's lecture kept for one saved revision. */
export interface LectureScript {
  revisionNumber: number;
  slides: LectureScriptSlide[];
}

export interface LectureAnswer {
  correct: boolean;
  correctOptionId: string;
  /** What the teacher says back. */
  reply: string;
}

/** The teacher's lecture kept for a revision, or null when none is written. */
export async function getLessonLectureScript(
  lessonId: string,
  revisionNumber: number,
): Promise<LectureScript | null> {
  return invoke<LectureScript | null>("get_lesson_lecture_script", {
    input: { lessonId, revisionNumber },
  });
}

/**
 * Writes the teacher's lecture for a revision with the default document model
 * and keeps it, replacing an earlier one.
 */
export async function writeLessonLectureScript(
  lessonId: string,
  revisionNumber: number,
  operationId: string = createOperationId(),
): Promise<LectureScript> {
  return invoke<LectureScript>("write_lesson_lecture_script", {
    input: { lessonId, revisionNumber, operationId },
  });
}

/** Grades an answer to the teacher's question. Nothing is saved. */
export async function answerLectureQuestion(
  lessonId: string,
  revisionNumber: number,
  slideKey: string,
  optionId: string,
): Promise<LectureAnswer> {
  return invoke<LectureAnswer>("answer_lecture_question", {
    input: { lessonId, revisionNumber, slideKey, optionId },
  });
}

/**
 * Speaks a slide's talk, or with `replyTo` the teacher's reply to that
 * answer, with the lesson voice and keeps the audio.
 */
export async function narrateLecturePart(
  lessonId: string,
  revisionNumber: number,
  slideKey: string,
  replyTo: string | null = null,
): Promise<LessonStepNarration> {
  return invoke<LessonStepNarration>("narrate_lecture_part", {
    input: { lessonId, revisionNumber, slideKey, replyTo },
  });
}
