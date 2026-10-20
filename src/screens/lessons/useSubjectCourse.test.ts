import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as lessonsIpc from "../../ipc/lessons";
import { COURSE_PROGRESS_INTERVAL_MS, resetCourseRunsForTests, useSubjectCourse } from "./useSubjectCourse";

vi.mock("../../ipc/common", () => ({
  createOperationId: () => "operation-1",
  getErrorCode: (error: unknown) => (error as { code?: string }).code ?? null,
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/lessons", () => ({
  listSubjectLessons: vi.fn(),
  writeSubjectCourse: vi.fn(),
  cancelSubjectCourseWriting: vi.fn(),
  getSubjectCourseWritingProgress: vi.fn(),
}));

const EMPTY: lessonsIpc.SubjectLessons = { subjectId: "cardiac", lessons: [] };
const WRITTEN = {
  subjectId: "cardiac",
  lessons: [{ id: "lesson-1", subjectId: "cardiac", title: "Filling", revision: 1, written: null, progress: null }],
} as lessonsIpc.SubjectLessons;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((ok, fail) => {
    resolve = ok;
    reject = fail;
  });
  return { promise, resolve, reject };
}

describe("useSubjectCourse", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetCourseRunsForTests();
    vi.mocked(lessonsIpc.listSubjectLessons).mockResolvedValue(EMPTY);
    vi.mocked(lessonsIpc.getSubjectCourseWritingProgress).mockResolvedValue({ planned: null, written: 0 });
  });

  it("keeps a run going when the page closes and shows its result on return", async () => {
    const run = deferred<lessonsIpc.SubjectLessons>();
    vi.mocked(lessonsIpc.writeSubjectCourse).mockReturnValue(run.promise);
    const first = renderHook(() => useSubjectCourse("cardiac"));
    await waitFor(() => expect(first.result.current.course).toEqual(EMPTY));
    act(() => first.result.current.write());
    expect(first.result.current.writing).toBe(true);
    first.unmount();

    const second = renderHook(() => useSubjectCourse("cardiac"));
    expect(second.result.current.writing).toBe(true);
    act(() => second.result.current.write());
    expect(lessonsIpc.writeSubjectCourse).toHaveBeenCalledOnce();

    await act(async () => run.resolve(WRITTEN));
    await waitFor(() => expect(second.result.current.writing).toBe(false));
    expect(second.result.current.course).toEqual(WRITTEN);
  });

  it("treats a canceled run as stopped, not failed", async () => {
    const run = deferred<lessonsIpc.SubjectLessons>();
    vi.mocked(lessonsIpc.writeSubjectCourse).mockReturnValue(run.promise);
    vi.mocked(lessonsIpc.cancelSubjectCourseWriting).mockResolvedValue(undefined as never);
    const { result } = renderHook(() => useSubjectCourse("cardiac"));
    act(() => result.current.write());
    act(() => result.current.stop());
    expect(result.current.stopping).toBe(true);
    expect(lessonsIpc.cancelSubjectCourseWriting).toHaveBeenCalledWith("operation-1");

    await act(async () => run.reject({ code: "lesson_writing_canceled" }));
    await waitFor(() => expect(result.current.writing).toBe(false));
    expect(result.current.writeError).toBeNull();
  });

  it("reports a failed run", async () => {
    vi.mocked(lessonsIpc.writeSubjectCourse).mockRejectedValue(new Error("No AI provider"));
    const { result } = renderHook(() => useSubjectCourse("cardiac"));
    act(() => result.current.write());
    await waitFor(() => expect(result.current.writeError).toBe("No AI provider"));
    expect(result.current.writing).toBe(false);
  });

  it("shows how many lessons are written while a run goes, and stops asking when it ends", async () => {
    vi.useFakeTimers();
    try {
      const run = deferred<lessonsIpc.SubjectLessons>();
      vi.mocked(lessonsIpc.writeSubjectCourse).mockReturnValue(run.promise);
      vi.mocked(lessonsIpc.getSubjectCourseWritingProgress)
        .mockRejectedValueOnce(new Error("busy"))
        .mockResolvedValue({ planned: 4, written: 1 });
      const { result } = renderHook(() => useSubjectCourse("cardiac"));
      act(() => result.current.write());
      expect(result.current.progress).toBeNull();

      // A failed ask is tried again on the next tick.
      await act(() => vi.advanceTimersByTimeAsync(COURSE_PROGRESS_INTERVAL_MS));
      expect(result.current.progress).toBeNull();
      await act(() => vi.advanceTimersByTimeAsync(COURSE_PROGRESS_INTERVAL_MS));
      expect(result.current.progress).toEqual({ planned: 4, written: 1 });
      expect(lessonsIpc.getSubjectCourseWritingProgress).toHaveBeenCalledWith("operation-1");

      await act(async () => run.resolve(WRITTEN));
      expect(result.current.progress).toBeNull();
      const asked = vi.mocked(lessonsIpc.getSubjectCourseWritingProgress).mock.calls.length;
      await act(() => vi.advanceTimersByTimeAsync(COURSE_PROGRESS_INTERVAL_MS * 3));
      expect(lessonsIpc.getSubjectCourseWritingProgress).toHaveBeenCalledTimes(asked);
    } finally {
      vi.useRealTimers();
    }
  });
});
