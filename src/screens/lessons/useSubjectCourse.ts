import { useCallback, useEffect, useState, useSyncExternalStore } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { createOperationId, getErrorCode, getErrorMessage } from "../../ipc/common";
import { getSettings } from "../../ipc/settings";
import { getSpeechSettings } from "../../ipc/speech";
import {
  cancelSubjectCourseWriting,
  getLesson,
  getSubjectCourseWritingProgress,
  listSubjectLessons,
  narrateLessonStep,
  updateLessonFromSources,
  writeSubjectCourse,
  type CourseWritingProgress,
  type SubjectLessons,
} from "../../ipc/lessons";
import { PODCAST_MINUTES, writePodcast } from "../../ipc/podcasts";

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
  /** Set when the run ends; the page that sees it clears it. */
  outcome: { lessons: SubjectLessons; podcastError?: string } | { error: string } | { stopped: true } | null;
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
  publish(subjectId, { operationId, stopping: false, progress: null, podcast: false, outcome: null });
  watchProgress(subjectId, operationId);
  void writeSubjectCourse(subjectId, operationId, rewrite).then(
    async (lessons) => {
      void makeLectureAudio(lessons, lectureAudio);
      const written = lessons.lessons.filter((lesson) => lesson.written).slice(0, COURSE_PODCAST_LESSONS);
      const run = runs.get(subjectId);
      if (!podcast || written.length === 0 || run?.operationId !== operationId || run.stopping) {
        return settle(subjectId, operationId, { lessons });
      }
      publish(subjectId, { ...run, podcast: true });
      // The lessons are saved whatever happens to the podcast.
      const podcastError = await writePodcast(subjectId, written.map((lesson) => lesson.id), PODCAST_MINUTES.default).then(
        () => undefined,
        (error: unknown) => getErrorMessage(error),
      );
      settle(subjectId, operationId, podcastError === undefined ? { lessons } : { lessons, podcastError });
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
 * Speaks and keeps each new lesson's narration when the learner asked for
 * lecture audio with new lessons and a lesson voice is set up, so the
 * lecture plays without waiting for the voice. Audio is optional: the
 * lessons are already saved, and a step that fails is spoken when played.
 */
export async function makeLectureAudio(course: SubjectLessons, wanted?: boolean): Promise<void> {
  if (wanted === false) return;
  try {
    const [settings, speech] = await Promise.all([getSettings(), getSpeechSettings()]);
    if (speech.profileId === null || (wanted === undefined && settings.lectureAudioWithNewLessons === false)) return;
  } catch {
    return;
  }
  for (const summary of course.lessons) {
    if (!summary.written) continue;
    try {
      const lesson = await getLesson(summary.id);
      for (const step of lesson.content.steps) {
        if (!step.narration.trim()) continue;
        await narrateLessonStep(summary.id, lesson.revisionNumber, step.key).catch(() => undefined);
      }
    } catch {
      // The lesson could not be read; its audio is made when it is played.
    }
  }
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

/** A subject's course, and writing it with AI. */
export function useSubjectCourse(subjectId: string): SubjectCourse {
  const lane = useRequestLane();
  const [course, setCourse] = useState<SubjectLessons | null>(null);
  const [loading, setLoading] = useState(true);
  const [readError, setReadError] = useState<string | null>(null);
  const [writeError, setWriteError] = useState<string | null>(null);
  const [podcastError, setPodcastError] = useState<string | null>(null);
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
