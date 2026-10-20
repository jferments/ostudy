import { useCallback, useEffect, useState, useSyncExternalStore } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { createOperationId, getErrorCode, getErrorMessage } from "../../ipc/common";
import {
  cancelSubjectCourseWriting,
  listSubjectLessons,
  writeSubjectCourse,
  type SubjectLessons,
} from "../../ipc/lessons";

/**
 * A course being written keeps going when its subject page closes, so the
 * runs live outside any one page: reopening the subject shows the run, and
 * its result, instead of offering to start a second one.
 */
interface CourseRun {
  operationId: string;
  stopping: boolean;
  /** Set when the run ends; the page that sees it clears it. */
  outcome: { lessons: SubjectLessons } | { error: string } | { stopped: true } | null;
}

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

function startRun(subjectId: string) {
  if (runs.has(subjectId)) return;
  const operationId = createOperationId();
  publish(subjectId, { operationId, stopping: false, outcome: null });
  void writeSubjectCourse(subjectId, operationId).then(
    (lessons) => settle(subjectId, operationId, { lessons }),
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
  writeError: string | null;
  refresh: () => void;
  write: () => void;
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
    writeError,
    refresh,
    write: () => {
      setWriteError(null);
      startRun(subjectId);
    },
    stop: () => void stopRun(subjectId),
    dismissWriteError: () => setWriteError(null),
  };
}
