import { useCallback, useEffect, useMemo, useState, useSyncExternalStore } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { createOperationId, getErrorCode, getErrorMessage } from "../../ipc/common";
import { getSettings } from "../../ipc/settings";
import { getSpeechSettings } from "../../ipc/speech";
import {
  cancelSubjectCourseWriting,
  getSubjectCourseWritingProgress,
  listSubjectLessons,
  updateLessonFromSources,
  writeSubjectCourse,
  type CourseWritingProgress,
  type SubjectLessons,
} from "../../ipc/lessons";
import { PODCAST_MINUTES, writePodcast } from "../../ipc/podcasts";
import { makeLessonLectureAudio } from "./lectureAudio";

/** How often a running course is asked how far it is. */
export const COURSE_PROGRESS_INTERVAL_MS = 1500;

/**
 * A course being written keeps going when its subject page closes, so the
 * runs live outside any one page: reopening the subject shows the run, and
 * its result, instead of offering to start a second one.
 */
interface CourseRun {
  operationId: string;
  stopping: boolean;
  progress: CourseWritingProgress | null;
  /** True while the podcast asked for with the course is being written. */
  podcast: boolean;
  /** True when a podcast was asked for with the course. */
  wantsPodcast?: boolean;
  /** How many lessons' lecture audio is made, while it is being made. */
  audio: LectureAudioProgress | null;
  /** Set when the run ends; the page that sees it clears it. */
  outcome: { lessons: SubjectLessons; podcastError?: string; audioError?: string } | { error: string } | { stopped: true } | null;
}

export interface LectureAudioProgress {
  done: number;
  total: number;
}

/** How many of a new course's first lessons its podcast talks through. */
export const COURSE_PODCAST_LESSONS = 3;

const runs = new Map<string, CourseRun>();
const listeners = new Set<() => void>();

function publish(subjectId: string, run: CourseRun | null) {
  if (run) runs.set(subjectId, run);
  else runs.delete(subjectId);
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/** What a new course is followed by. */
export interface CourseExtras {
  /** A podcast about the first lessons. */
  podcast?: boolean;
  /** Lecture audio for every lesson, whatever the setting says; false makes none. */
  lectureAudio?: boolean;
}

function startRun(subjectId: string, rewrite: boolean, { podcast = false, lectureAudio }: CourseExtras = {}) {
  if (runs.has(subjectId)) return;
  const operationId = createOperationId();
  publish(subjectId, { operationId, stopping: false, progress: null, podcast: false, wantsPodcast: podcast, audio: null, outcome: null });
  watchProgress(subjectId, operationId);
  const current = () => {
    const run = runs.get(subjectId);
    return run?.operationId === operationId && !run.outcome ? run : null;
  };
  const update = (change: Partial<CourseRun>) => {
    const run = current();
    if (run) publish(subjectId, { ...run, ...change });
  };
  void writeSubjectCourse(subjectId, operationId, rewrite).then(
    async (lessons) => {
      if (current()?.stopping !== false) return settle(subjectId, operationId, { lessons });
      const written = lessons.lessons.filter((lesson) => lesson.written).slice(0, COURSE_PODCAST_LESSONS);
      // The lessons are saved whatever happens to their audio or the podcast,
      // which are made side by side.
      const writingPodcast = async () => {
        if (!podcast || written.length === 0) return undefined;
        update({ podcast: true });
        return writePodcast(subjectId, written.map((lesson) => lesson.id), PODCAST_MINUTES.default).then(
          () => undefined,
          (error: unknown) => getErrorMessage(error),
        );
      };
      const [audioError, podcastError] = await Promise.all([
        makeLectureAudio(lessons, lectureAudio, (audio) => update({ audio }), () => current()?.stopping !== false),
        writingPodcast(),
      ]);
      settle(subjectId, operationId, {
        lessons,
        ...(podcastError === undefined ? {} : { podcastError }),
        ...(audioError === undefined ? {} : { audioError }),
      });
    },
    (error: unknown) =>
      settle(
        subjectId,
        operationId,
        getErrorCode(error) === "lesson_writing_canceled"
          ? { stopped: true }
          : { error: getErrorMessage(error) },
      ),
  );
}

/**
 * Writes each new lesson's teacher's lecture and speaks and keeps all of it
 * when the learner asked for lecture audio with new lessons and a lesson
 * voice is set up, so Play lecture plays at once without waiting for the
 * voice. Audio is optional: the lessons are already saved, and a piece that
 * fails is spoken when played. Returns why some audio is missing, if it is.
 */
export async function makeLectureAudio(
  course: SubjectLessons,
  wanted?: boolean,
  onProgress?: (progress: LectureAudioProgress | null) => void,
  stopped: () => boolean = () => false,
): Promise<string | undefined> {
  if (wanted === false) return undefined;
  try {
    const [settings, speech] = await Promise.all([getSettings(), getSpeechSettings()]);
    if (speech.profileId === null || (wanted === undefined && settings.lectureAudioWithNewLessons === false)) return undefined;
  } catch {
    return undefined;
  }
  const written = course.lessons.filter((lesson) => lesson.written);
  let missing = 0;
  for (const [done, lesson] of written.entries()) {
    if (stopped()) break;
    onProgress?.({ done, total: written.length });
    missing += await makeLessonLectureAudio(lesson.id, stopped).catch(() => 1);
  }
  onProgress?.(null);
  return missing > 0 && !stopped()
    ? "Some of the lecture audio was not made; it is made when that part plays."
    : undefined;
}

/**
 * Asks how far the run is until it ends. Progress is advisory: a failed ask
 * is tried again, and the run's own result is what ends it.
 */
function watchProgress(subjectId: string, operationId: string) {
  const current = () => {
    const run = runs.get(subjectId);
    return run?.operationId === operationId && !run.outcome ? run : null;
  };
  const ask = () => {
    if (!current()) return;
    getSubjectCourseWritingProgress(operationId).then(
      (progress) => {
        const run = current();
        if (run) publish(subjectId, { ...run, progress });
      },
      () => undefined,
    ).finally(() => {
      if (current()) setTimeout(ask, COURSE_PROGRESS_INTERVAL_MS);
    });
  };
  setTimeout(ask, COURSE_PROGRESS_INTERVAL_MS);
}

function settle(subjectId: string, operationId: string, outcome: NonNullable<CourseRun["outcome"]>) {
  const run = runs.get(subjectId);
  if (run?.operationId !== operationId) return;
  publish(subjectId, { ...run, outcome });
}

async function stopRun(subjectId: string) {
  const run = runs.get(subjectId);
  if (!run || run.outcome || run.stopping) return;
  publish(subjectId, { ...run, stopping: true });
  try {
    await cancelSubjectCourseWriting(run.operationId);
  } catch {
    // The run already ended; its own result arrives through `settle`.
  }
}

/** Forgets every run; tests start from nothing. */
export function resetCourseRunsForTests() {
  runs.clear();
  for (const listener of listeners) listener();
}

export interface SubjectCourse {
  course: SubjectLessons | null;
  loading: boolean;
  readError: string | null;
  writing: boolean;
  stopping: boolean;
  /** How far the running course is, once it has said. */
  progress: CourseWritingProgress | null;
  writeError: string | null;
  /** True while the podcast asked for with the course is being written. */
  writingPodcast: boolean;
  /** Why the podcast asked for with the course was not written. */
  podcastError: string | null;
  /** How far the lecture audio is, while it is being made. */
  audio: LectureAudioProgress | null;
  /** Why some of the lecture audio was not made. */
  audioError: string | null;
  /** The lesson being written again from its sources. */
  updating: string | null;
  updateError: string | null;
  refresh: () => void;
  update: (lessonId: string) => void;
  /** Writes the course, then what `extras` asks for. */
  write: (extras?: CourseExtras) => void;
  /** Writes the whole course again; each lesson keeps its place. */
  rewrite: () => void;
  stop: () => void;
  dismissWriteError: () => void;
}

/** How far a subject's course being written is, for pages that show it without managing it. */
export interface CourseRunProgress {
  progress: CourseWritingProgress | null;
  audio: LectureAudioProgress | null;
  /** A podcast was asked for and is not written yet. */
  podcastPending: boolean;
  /** The podcast is being written now. */
  writingPodcast: boolean;
}

/** The subject's course being written, if one is. */
export function useCourseRun(subjectId: string): CourseRunProgress | null {
  const run = useSyncExternalStore(subscribe, () => runs.get(subjectId) ?? null);
  return useMemo(() => (run && run.outcome === null
    ? { progress: run.progress, audio: run.audio, podcastPending: run.wantsPodcast === true, writingPodcast: run.podcast }
    : null), [run]);
}

/** Starts writing a subject's new course from outside its page, as a New study set does. */
export function startSubjectCourse(subjectId: string, extras?: CourseExtras): void {
  startRun(subjectId, false, extras);
}

function writingSubjectsKey(): string {
  return [...runs].filter(([, run]) => run.outcome === null).map(([subjectId]) => subjectId).sort().join("\n");
}

/** The subjects whose course, with its podcast and lecture audio, is being written now. */
export function useSubjectsWritingCourses(): ReadonlySet<string> {
  const key = useSyncExternalStore(subscribe, writingSubjectsKey);
  return useMemo(() => new Set(key ? key.split("\n") : []), [key]);
}

/** A subject's course, and writing it with AI. */
export function useSubjectCourse(subjectId: string): SubjectCourse {
  const lane = useRequestLane();
  const [course, setCourse] = useState<SubjectLessons | null>(null);
  const [loading, setLoading] = useState(true);
  const [readError, setReadError] = useState<string | null>(null);
  const [writeError, setWriteError] = useState<string | null>(null);
  const [podcastError, setPodcastError] = useState<string | null>(null);
  const [audioError, setAudioError] = useState<string | null>(null);
  const [updating, setUpdating] = useState<string | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);
  const run = useSyncExternalStore(subscribe, () => runs.get(subjectId) ?? null);

  const refresh = useCallback(() => {
    setLoading(true);
    void latestResult(lane, () => listSubjectLessons(subjectId)).then(
      (value) => {
        if (value === undefined) return;
        setCourse(value);
        setReadError(null);
        setLoading(false);
      },
      (error: unknown) => {
        setReadError(getErrorMessage(error));
        setLoading(false);
      },
    );
  }, [lane, subjectId]);

  useEffect(() => {
    setCourse(null);
    refresh();
  }, [refresh]);

  // A finished run hands its result to the page and is forgotten.
  const outcome = run?.outcome ?? null;
  useEffect(() => {
    if (!outcome) return;
    publish(subjectId, null);
    if ("lessons" in outcome) {
      lane.invalidate();
      setCourse(outcome.lessons);
      setReadError(null);
      setLoading(false);
      setPodcastError(outcome.podcastError ?? null);
      setAudioError(outcome.audioError ?? null);
    } else if ("error" in outcome) {
      setWriteError(outcome.error);
    }
  }, [lane, outcome, subjectId]);

  return {
    course,
    loading,
    readError,
    writing: run !== null && run.outcome === null,
    stopping: run?.stopping ?? false,
    progress: run && !run.outcome ? run.progress : null,
    writeError,
    writingPodcast: run !== null && run.outcome === null && run.podcast,
    podcastError,
    audio: run && !run.outcome ? run.audio : null,
    audioError,
    updating,
    updateError,
    refresh,
    update: (lessonId: string) => {
      setUpdating(lessonId);
      setUpdateError(null);
      updateLessonFromSources(lessonId)
        .then(
          (value) => {
            lane.invalidate();
            setCourse(value);
            setReadError(null);
          },
          (error: unknown) => setUpdateError(getErrorMessage(error)),
        )
        .finally(() => setUpdating(null));
    },
    write: (extras) => {
      setWriteError(null);
      setPodcastError(null);
      setAudioError(null);
      startRun(subjectId, false, extras);
    },
    rewrite: () => {
      setWriteError(null);
      startRun(subjectId, true);
    },
    stop: () => void stopRun(subjectId),
    dismissWriteError: () => setWriteError(null),
  };
}
