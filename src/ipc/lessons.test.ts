import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  cancelSubjectCourseWriting,
  getLesson,
  listSubjectLessons,
  saveLessonProgress,
  writeSubjectCourse,
} from "./lessons";

vi.mock("./transport", () => ({ invoke: vi.fn() }));

const subjectId = "a".repeat(32);
const lessonId = "b".repeat(32);

describe("lesson IPC", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({});
  });

  it("lists a subject's course", async () => {
    await listSubjectLessons(subjectId);
    expect(invoke).toHaveBeenCalledWith("list_subject_lessons", {
      input: { subjectId },
    });
  });

  it("reads the latest revision unless an earlier one is named", async () => {
    await getLesson(lessonId);
    expect(invoke).toHaveBeenLastCalledWith("get_lesson", {
      input: { lessonId, revisionNumber: null },
    });
    await getLesson(lessonId, 1);
    expect(invoke).toHaveBeenLastCalledWith("get_lesson", {
      input: { lessonId, revisionNumber: 1 },
    });
  });

  it("saves whole progress states, so a repeated request is safe", async () => {
    await saveLessonProgress(lessonId, 2, 3, true);
    expect(invoke).toHaveBeenCalledWith("save_lesson_progress", {
      input: { lessonId, revisionNumber: 2, currentStep: 3, finished: true },
    });
  });

  it("writes a course under one operation that can be stopped", async () => {
    const operationId = "c".repeat(32);
    await writeSubjectCourse(subjectId, operationId);
    expect(invoke).toHaveBeenLastCalledWith("write_subject_course", {
      input: { subjectId, operationId },
    });
    await cancelSubjectCourseWriting(operationId);
    expect(invoke).toHaveBeenLastCalledWith("cancel_subject_course_writing", {
      input: { operationId },
    });
    await writeSubjectCourse(subjectId);
    const generated = vi.mocked(invoke).mock.lastCall?.[1] as {
      input: { operationId: string };
    };
    expect(generated.input.operationId).toMatch(/^[0-9a-f]{32}$/);
  });
});
