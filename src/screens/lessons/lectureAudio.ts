import {
  askLectureQuestion,
  getLesson,
  getLessonLectureScript,
  narrateLecturePart,
  narrateLessonStep,
  writeLessonLectureScript,
  type LectureScript,
  type LessonStepNarration,
} from "../../ipc/lessons";

/** How many pieces of one lesson's lecture are spoken at once. */
const PARTS_AT_ONCE = 3;

/**
 * Everything Play lecture can say for one revision: each slide's talk, a
 * question asked before the talk, and the teacher's reply to every answer.
 */
export function lectureParts(
  lessonId: string,
  revisionNumber: number,
  script: LectureScript,
): Array<() => Promise<LessonStepNarration>> {
  return script.slides.flatMap((slide) => [
    ...(slide.question?.timing === "before" ? [() => askLectureQuestion(lessonId, revisionNumber, slide.slideKey)] : []),
    ...(slide.talk.trim() ? [() => narrateLecturePart(lessonId, revisionNumber, slide.slideKey)] : []),
    ...(slide.question?.options ?? []).map(
      (option) => () => narrateLecturePart(lessonId, revisionNumber, slide.slideKey, option.id),
    ),
  ]);
}

/** Runs each task, a few at a time, until all are done or `stopped` says so. */
async function runAll(tasks: Array<() => Promise<unknown>>, stopped: () => boolean): Promise<number> {
  let next = 0;
  let failed = 0;
  const worker = async () => {
    while (next < tasks.length && !stopped()) {
      const task = tasks[next++]!;
      await task().catch(() => {
        failed += 1;
      });
    }
  };
  await Promise.all(Array.from({ length: Math.min(PARTS_AT_ONCE, tasks.length) }, worker));
  return failed;
}

/** Speaks and keeps every part of a written lecture; returns how many failed. */
export function keepLectureAudio(lessonId: string, revisionNumber: number, script: LectureScript): Promise<number> {
  return runAll(lectureParts(lessonId, revisionNumber, script), () => false);
}

/**
 * Makes and keeps all of one lesson's lecture audio, writing the teacher's
 * lecture first when it has none, so Play lecture only plays what is kept.
 * When the lecture cannot be written, each step's narration is spoken
 * instead. Returns how many pieces could not be made; those are spoken when
 * played.
 */
export async function makeLessonLectureAudio(
  lessonId: string,
  stopped: () => boolean = () => false,
): Promise<number> {
  const lesson = await getLesson(lessonId);
  const revision = lesson.revisionNumber;
  let script = await getLessonLectureScript(lessonId, revision).catch(() => null);
  if (!script && !stopped()) script = await writeLessonLectureScript(lessonId, revision).catch(() => null);
  if (stopped()) return 0;
  if (script) return runAll(lectureParts(lessonId, revision, script), stopped);
  const steps = lesson.content.steps.filter((step) => step.narration.trim());
  const failed = await runAll(steps.map((step) => () => narrateLessonStep(lessonId, revision, step.key)), stopped);
  // Without its lecture the lesson still plays, but the teacher's talk is missing.
  return failed + 1;
}
