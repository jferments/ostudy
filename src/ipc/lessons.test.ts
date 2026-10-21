import { invoke } from "./transport";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  answerLessonCheck,
  cancelSubjectCourseWriting,
  getLesson,
  getLessonLecture,
  prepareLessonLecturePptx,
  getSubjectCourseWritingProgress,
  listSubjectLessons,
  narrateLessonStep,
  getLessonLectureScript,
  writeLessonLectureScript,
  answerLectureQuestion,
  narrateLecturePart,
  saveLessonProgress,
  updateLessonFromSources,
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

  it("asks how far a course being written is by its operation", async () => {
    await getSubjectCourseWritingProgress("c".repeat(32));
    expect(invoke).toHaveBeenLastCalledWith("get_subject_course_writing_progress", {
      input: { operationId: "c".repeat(32) },
    });
  });

  it("reads a lesson as a lecture from the same revision rules", async () => {
    await getLessonLecture(lessonId);
    expect(invoke).toHaveBeenLastCalledWith("get_lesson_lecture", {
      input: { lessonId, revisionNumber: null },
    });
    await getLessonLecture(lessonId, 2);
    expect(invoke).toHaveBeenLastCalledWith("get_lesson_lecture", {
      input: { lessonId, revisionNumber: 2 },
    });
  });

  it("prepares a lecture deck under one operation", async () => {
    const operationId = "e".repeat(32);
    await prepareLessonLecturePptx(lessonId, 2, operationId);
    expect(invoke).toHaveBeenLastCalledWith("prepare_lesson_lecture_pptx", {
      input: { lessonId, revisionNumber: 2, operationId },
    });
  });

  it("saves whole progress states, so a repeated request is safe", async () => {
    await saveLessonProgress(lessonId, 2, 3, true);
    expect(invoke).toHaveBeenCalledWith("save_lesson_progress", {
      input: { lessonId, revisionNumber: 2, currentStep: 3, finished: true },
    });
  });

  it("answers one step's check by its option", async () => {
    const optionId = "d".repeat(32);
    await answerLessonCheck(lessonId, 1, "step-2", optionId);
    expect(invoke).toHaveBeenCalledWith("answer_lesson_check", {
      input: { lessonId, revisionNumber: 1, stepKey: "step-2", optionId },
    });
  });

  it("writes one lesson again from its sources", async () => {
    const operationId = "d".repeat(32);
    await updateLessonFromSources(lessonId, operationId);
    expect(invoke).toHaveBeenLastCalledWith("update_lesson_from_sources", {
      input: { lessonId, operationId },
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

  it("narrates one step of a saved revision", async () => {
    await narrateLessonStep(lessonId, 2, "two-limbs");
    expect(invoke).toHaveBeenLastCalledWith("narrate_lesson_step", {
      input: { lessonId, revisionNumber: 2, stepKey: "two-limbs" },
    });
  });

  it("reads, writes, answers and speaks the teacher's lecture of a revision", async () => {
    const operationId = "f".repeat(32);
    await getLessonLectureScript(lessonId, 2);
    expect(invoke).toHaveBeenLastCalledWith("get_lesson_lecture_script", {
      input: { lessonId, revisionNumber: 2 },
    });
    await writeLessonLectureScript(lessonId, 2, operationId);
    expect(invoke).toHaveBeenLastCalledWith("write_lesson_lecture_script", {
      input: { lessonId, revisionNumber: 2, operationId },
    });
    await answerLectureQuestion(lessonId, 2, "step:two-limbs", operationId);
    expect(invoke).toHaveBeenLastCalledWith("answer_lecture_question", {
      input: {
        lessonId,
        revisionNumber: 2,
        slideKey: "step:two-limbs",
        optionId: operationId,
      },
    });
    await narrateLecturePart(lessonId, 2, "title");
    expect(invoke).toHaveBeenLastCalledWith("narrate_lecture_part", {
      input: { lessonId, revisionNumber: 2, slideKey: "title", replyTo: null },
    });
  });
});
