import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as lessonsIpc from "../../ipc/lessons";
import * as podcastsIpc from "../../ipc/podcasts";
import * as settingsIpc from "../../ipc/settings";
import * as speechIpc from "../../ipc/speech";
import {
  COURSE_PROGRESS_INTERVAL_MS,
  makeLectureAudio,
  resetCourseRunsForTests,
  useSubjectCourse,
} from "./useSubjectCourse";

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
  updateLessonFromSources: vi.fn(),
  getLesson: vi.fn(),
  narrateLessonStep: vi.fn(),
  getLessonLectureScript: vi.fn(),
  writeLessonLectureScript: vi.fn(),
  narrateLecturePart: vi.fn(),
  askLectureQuestion: vi.fn(),
}));
vi.mock("../../ipc/podcasts", () => ({ PODCAST_MINUTES: { default: 15 }, writePodcast: vi.fn() }));
vi.mock("../../ipc/settings", () => ({ getSettings: vi.fn(async () => ({ lectureAudioWithNewLessons: false })) }));
vi.mock("../../ipc/speech", () => ({ getSpeechSettings: vi.fn(async () => ({ profileId: null })) }));

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

  it("writes one lesson again and shows the course it returns, or why it failed", async () => {
    const run = deferred<lessonsIpc.SubjectLessons>();
    vi.mocked(lessonsIpc.updateLessonFromSources).mockReturnValueOnce(run.promise);
    const { result } = renderHook(() => useSubjectCourse("cardiac"));
    await waitFor(() => expect(result.current.course).toEqual(EMPTY));
    act(() => result.current.update("lesson-1"));
    expect(result.current.updating).toBe("lesson-1");
    expect(lessonsIpc.updateLessonFromSources).toHaveBeenCalledWith("lesson-1");
    await act(async () => run.resolve(WRITTEN));
    await waitFor(() => expect(result.current.updating).toBeNull());
    expect(result.current.course).toEqual(WRITTEN);

    vi.mocked(lessonsIpc.updateLessonFromSources).mockRejectedValueOnce(new Error("Part gone"));
    act(() => result.current.update("lesson-1"));
    await waitFor(() => expect(result.current.updateError).toBe("Part gone"));
    expect(result.current.updating).toBeNull();
    expect(result.current.course).toEqual(WRITTEN);
  });

  it("writes a podcast about the first written lessons after the course when asked", async () => {
    const written = (id: string) => ({ id, subjectId: "cardiac", title: id, revision: 1, written: {}, progress: null });
    const course = {
      subjectId: "cardiac",
      lessons: [written("a"), { ...written("planned"), written: null }, written("b"), written("c"), written("d")],
    } as lessonsIpc.SubjectLessons;
    vi.mocked(lessonsIpc.writeSubjectCourse).mockResolvedValue(course);
    const podcast = deferred<never>();
    vi.mocked(podcastsIpc.writePodcast).mockReturnValue(podcast.promise);
    const { result } = renderHook(() => useSubjectCourse("cardiac"));
    act(() => result.current.write({ podcast: true }));
    await waitFor(() => expect(result.current.writingPodcast).toBe(true));
    expect(result.current.writing).toBe(true);
    expect(podcastsIpc.writePodcast).toHaveBeenCalledWith("cardiac", ["a", "b", "c"], 15);
    await act(async () => podcast.reject(new Error("No voice")));
    await waitFor(() => expect(result.current.writing).toBe(false));
    expect(result.current.course).toEqual(course);
    expect(result.current.podcastError).toBe("No voice");
  });

  it("keeps writing until the lessons' lecture audio is made, and says what is missing", async () => {
    const course = {
      subjectId: "cardiac",
      lessons: [{ id: "a", subjectId: "cardiac", title: "a", revision: 1, written: {}, progress: null }],
    } as unknown as lessonsIpc.SubjectLessons;
    vi.mocked(lessonsIpc.writeSubjectCourse).mockResolvedValue(course);
    vi.mocked(speechIpc.getSpeechSettings).mockResolvedValue({ profileId: "voice" } as speechIpc.SpeechSettings);
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue({ revisionNumber: 1, content: { steps: [] } } as unknown as lessonsIpc.Lesson);
    vi.mocked(lessonsIpc.getLessonLectureScript).mockResolvedValue(null);
    const lecture = deferred<lessonsIpc.LectureScript>();
    vi.mocked(lessonsIpc.writeLessonLectureScript).mockReturnValue(lecture.promise);
    const { result } = renderHook(() => useSubjectCourse("cardiac"));
    act(() => result.current.write({ lectureAudio: true }));
    await waitFor(() => expect(result.current.audio).toEqual({ done: 0, total: 1 }));
    expect(result.current.writing).toBe(true);
    await act(async () => lecture.reject(new Error("No model")));
    await waitFor(() => expect(result.current.writing).toBe(false));
    expect(result.current.audio).toBeNull();
    expect(result.current.audioError).toMatch(/not made/u);
    expect(result.current.course).toEqual(course);
  });

  it("rewrites an existing course as a run of its own", async () => {
    vi.mocked(lessonsIpc.writeSubjectCourse).mockResolvedValue(WRITTEN);
    const { result } = renderHook(() => useSubjectCourse("cardiac"));
    await waitFor(() => expect(result.current.course).toEqual(EMPTY));
    act(() => result.current.rewrite());
    expect(result.current.writing).toBe(true);
    expect(lessonsIpc.writeSubjectCourse).toHaveBeenCalledWith("cardiac", "operation-1", true);
    await waitFor(() => expect(result.current.course).toEqual(WRITTEN));
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

describe("makeLectureAudio", () => {
  const course = {
    subjectId: "cardiac",
    lessons: [
      { id: "lesson-1", subjectId: "cardiac", title: "Filling", revision: 1, written: {}, progress: null },
      { id: "lesson-2", subjectId: "cardiac", title: "Planned", revision: 1, written: null, progress: null },
    ],
  } as unknown as lessonsIpc.SubjectLessons;
  const script: lessonsIpc.LectureScript = {
    revisionNumber: 3,
    slides: [
      { slideKey: "title", talk: "Welcome.", question: null },
      {
        slideKey: "a",
        talk: "Filling happens in diastole.",
        question: { timing: "before", prompt: "When?", options: [{ id: "x", text: "Diastole" }, { id: "y", text: "Systole" }] },
      },
      { slideKey: "b", talk: "  ", question: null },
    ],
  };
  const voiceOn = () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue({ lectureAudioWithNewLessons: true } as settingsIpc.AppSettings);
    vi.mocked(speechIpc.getSpeechSettings).mockResolvedValue({ profileId: "voice" } as speechIpc.SpeechSettings);
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue({
      revisionNumber: 3,
      content: {
        steps: [
          { key: "a", narration: "Say this." },
          { key: "b", narration: "  " },
        ],
      },
    } as unknown as lessonsIpc.Lesson);
    vi.mocked(lessonsIpc.getLessonLectureScript).mockResolvedValue(null);
    vi.mocked(lessonsIpc.writeLessonLectureScript).mockResolvedValue(script);
    vi.mocked(lessonsIpc.narrateLessonStep).mockResolvedValue({ segments: [], made: 1 });
    vi.mocked(lessonsIpc.narrateLecturePart).mockResolvedValue({ segments: [], made: 1 });
    vi.mocked(lessonsIpc.askLectureQuestion).mockResolvedValue({ segments: [], made: 1 });
  });

  it("writes each written lesson's lecture and keeps all of its audio", async () => {
    voiceOn();
    const progress = vi.fn();
    expect(await makeLectureAudio(course, undefined, progress)).toBeUndefined();
    expect(lessonsIpc.writeLessonLectureScript).toHaveBeenCalledTimes(1);
    expect(lessonsIpc.writeLessonLectureScript).toHaveBeenCalledWith("lesson-1", 3);
    expect(lessonsIpc.askLectureQuestion).toHaveBeenCalledWith("lesson-1", 3, "a");
    // Each talk, and the teacher's reply to every answer, but not an empty talk.
    expect(vi.mocked(lessonsIpc.narrateLecturePart).mock.calls).toEqual(
      expect.arrayContaining([
        ["lesson-1", 3, "title"],
        ["lesson-1", 3, "a"],
        ["lesson-1", 3, "a", "x"],
        ["lesson-1", 3, "a", "y"],
      ]),
    );
    expect(lessonsIpc.narrateLecturePart).toHaveBeenCalledTimes(4);
    expect(lessonsIpc.narrateLessonStep).not.toHaveBeenCalled();
    expect(progress.mock.calls).toEqual([[{ done: 0, total: 1 }], [null]]);
  });

  it("keeps a lecture already written and says when some audio is missing", async () => {
    voiceOn();
    vi.mocked(lessonsIpc.getLessonLectureScript).mockResolvedValue(script);
    vi.mocked(lessonsIpc.narrateLecturePart).mockRejectedValueOnce(new Error("Voice busy"));
    expect(await makeLectureAudio(course)).toMatch(/not made/u);
    expect(lessonsIpc.writeLessonLectureScript).not.toHaveBeenCalled();
  });

  it("speaks the steps' narration when the lecture cannot be written", async () => {
    voiceOn();
    vi.mocked(lessonsIpc.writeLessonLectureScript).mockRejectedValue(new Error("No model"));
    expect(await makeLectureAudio(course)).toMatch(/not made/u);
    expect(lessonsIpc.narrateLessonStep).toHaveBeenCalledTimes(1);
    expect(lessonsIpc.narrateLessonStep).toHaveBeenCalledWith("lesson-1", 3, "a");
  });

  it("makes nothing when turned off, without a voice, or stopped", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue({ lectureAudioWithNewLessons: false } as settingsIpc.AppSettings);
    vi.mocked(speechIpc.getSpeechSettings).mockResolvedValue({ profileId: "voice" } as speechIpc.SpeechSettings);
    await makeLectureAudio(course);
    vi.mocked(settingsIpc.getSettings).mockResolvedValue({} as settingsIpc.AppSettings);
    vi.mocked(speechIpc.getSpeechSettings).mockResolvedValue({ profileId: null } as speechIpc.SpeechSettings);
    await makeLectureAudio(course);
    voiceOn();
    expect(await makeLectureAudio(course, true, undefined, () => true)).toBeUndefined();
    expect(lessonsIpc.writeLessonLectureScript).not.toHaveBeenCalled();
    expect(lessonsIpc.narrateLecturePart).not.toHaveBeenCalled();
  });

  it("follows a study set's own choice over the setting", async () => {
    vi.mocked(settingsIpc.getSettings).mockResolvedValue({ lectureAudioWithNewLessons: false } as settingsIpc.AppSettings);
    vi.mocked(speechIpc.getSpeechSettings).mockResolvedValue({ profileId: "voice" } as speechIpc.SpeechSettings);
    await makeLectureAudio(course, false);
    expect(lessonsIpc.writeLessonLectureScript).not.toHaveBeenCalled();
    await makeLectureAudio(course, true);
    expect(lessonsIpc.writeLessonLectureScript).toHaveBeenCalledWith("lesson-1", 3);
  });
});
