import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as lessonsIpc from "../../ipc/lessons";
import { LessonPlayer } from "./LessonPlayer";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/lessons", () => ({
  getLesson: vi.fn(),
  saveLessonProgress: vi.fn(),
  answerLessonCheck: vi.fn(),
  getLessonLecture: vi.fn(),
  getLessonLectureScript: vi.fn(async () => null),
  narrateLessonStep: vi.fn(),
}));

function makeProgress(overrides: Partial<lessonsIpc.LessonProgress> = {}): lessonsIpc.LessonProgress {
  return {
    revisionNumber: 2,
    currentStep: 0,
    furthestStep: 0,
    checks: {},
    startedAt: 1,
    updatedAt: 1,
    finishedAt: null,
    revision: 1,
    ...overrides,
  };
}

function step(key: string, title: string, check: lessonsIpc.LessonCheck | null = null): lessonsIpc.LessonStep {
  return {
    key,
    title,
    blocks: [{ kind: "keyPoints", items: [`${title} point`] }],
    narration: "",
    slidePoints: [],
    checkKind: check ? "singleChoice" : null,
    check,
    knowledgeUnitIds: [],
  } as lessonsIpc.LessonStep;
}

function makeLesson(progress: lessonsIpc.LessonProgress | null): lessonsIpc.Lesson {
  return {
    lesson: {
      id: "lesson-1",
      subjectId: "cardiac",
      title: "Cardiac output",
      revision: 1,
      written: null,
      progress,
    },
    revisionNumber: 2,
    sources: "current",
    content: {
      title: "Cardiac output",
      summary: "",
      objectives: [],
      estimatedMinutes: 8,
      steps: [
        step("rate", "Heart rate"),
        step("volume", "Stroke volume", {
          prompt: "What raises stroke volume?",
          options: [
            { id: "a", text: "More filling" },
            { id: "b", text: "Less filling" },
          ],
          timing: "after",
        }),
      ],
    },
  };
}

function renderPlayer() {
  const props = { onClose: vi.fn(), showToast: vi.fn() };
  render(
    <LessonPlayer lessonId="lesson-1" subjectName="Cardiac physiology" lessonNumber={2} lessonCount={5} {...props} />,
  );
  return props;
}

describe("LessonPlayer", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(lessonsIpc.saveLessonProgress).mockImplementation(async (_id, revisionNumber, currentStep, finished) =>
      makeProgress({ revisionNumber, currentStep, furthestStep: currentStep, finishedAt: finished ? 9 : null }),
    );
  });

  it("resumes where the learner stopped and saves each move", async () => {
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue(makeLesson(makeProgress({ currentStep: 1, furthestStep: 1 })));
    renderPlayer();
    expect(await screen.findByRole("heading", { level: 1, name: "Stroke volume" })).toBeTruthy();
    expect(screen.getByText("Lesson 2 · Step 2 of 2")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: /Back/u }));
    expect(await screen.findByRole("heading", { level: 1, name: "Heart rate" })).toBeTruthy();
    expect(lessonsIpc.saveLessonProgress).toHaveBeenCalledWith("lesson-1", 2, 0, false);
  });

  it("opens a finished lesson at its start and finishing saves then closes", async () => {
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue(
      makeLesson(makeProgress({ currentStep: 1, furthestStep: 1, finishedAt: 5 })),
    );
    const props = renderPlayer();
    expect(await screen.findByRole("heading", { level: 1, name: "Heart rate" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Next/u }));
    fireEvent.click(await screen.findByRole("button", { name: /Finish lesson/u }));
    await vi.waitFor(() => expect(props.onClose).toHaveBeenCalledOnce());
    expect(lessonsIpc.saveLessonProgress).toHaveBeenLastCalledWith("lesson-1", 2, 1, true);
  });

  it("checks an answer natively and explains it", async () => {
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue(makeLesson(makeProgress({ currentStep: 1, furthestStep: 1 })));
    vi.mocked(lessonsIpc.answerLessonCheck).mockResolvedValue({
      correct: false,
      correctOptionId: "a",
      explanation: "Filling stretches the muscle.",
      progress: makeProgress({ currentStep: 1, checks: { volume: { optionId: "b", correct: false } } }),
    });
    renderPlayer();
    const check = await screen.findByTestId("lesson-check");
    const submit = within(check).getByRole("button", { name: "Check answer" });
    expect((submit as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(within(check).getByRole("radio", { name: "Less filling" }));
    fireEvent.click(submit);
    expect(await within(check).findByText("Not quite.")).toBeTruthy();
    expect(within(check).getByText("Filling stretches the muscle.")).toBeTruthy();
    expect(lessonsIpc.answerLessonCheck).toHaveBeenCalledWith("lesson-1", 2, "volume", "b");
  });

  it("asks a prediction ahead of the step's teaching and a check after it", async () => {
    const lesson = makeLesson(makeProgress({ currentStep: 1, furthestStep: 1 }));
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue(lesson);
    renderPlayer();
    let check = await screen.findByTestId("lesson-check");
    expect(within(check).getByText("Quick check")).toBeTruthy();
    const point = screen.getByText("Stroke volume point");
    expect(point.compareDocumentPosition(check) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    cleanup();

    lesson.content.steps[1]!.check!.timing = "before";
    renderPlayer();
    check = await screen.findByTestId("lesson-check");
    expect(within(check).getByText("What do you think?")).toBeTruthy();
    expect(
      screen.getByText("Stroke volume point").compareDocumentPosition(check) & Node.DOCUMENT_POSITION_PRECEDING,
    ).toBeTruthy();
  });

  it("shows an earlier answer's outcome when the step reopens", async () => {
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue(
      makeLesson(makeProgress({ currentStep: 1, checks: { volume: { optionId: "a", correct: true } } })),
    );
    renderPlayer();
    const check = await screen.findByTestId("lesson-check");
    expect(within(check).getByText("Right.")).toBeTruthy();
    expect(within(check).queryByRole("button", { name: "Check answer" })).toBeNull();
  });

  it("tells the learner when their place was not saved", async () => {
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue(makeLesson(null));
    vi.mocked(lessonsIpc.saveLessonProgress).mockRejectedValue(new Error("Disk full"));
    const props = renderPlayer();
    fireEvent.click(await screen.findByRole("button", { name: /Next/u }));
    expect(await screen.findByRole("heading", { level: 1, name: "Stroke volume" })).toBeTruthy();
    await vi.waitFor(() =>
      expect(props.showToast).toHaveBeenCalledWith("error", "Your place in this lesson was not saved: Disk full"),
    );
  });

  it("views the lesson as a lecture at the current step and comes back to it", async () => {
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue(makeLesson(makeProgress({ currentStep: 1, furthestStep: 1 })));
    vi.mocked(lessonsIpc.getLessonLecture).mockResolvedValue({
      lessonId: "lesson-1",
      revisionNumber: 2,
      title: "Cardiac output",
      slides: [
        { key: "title", kind: "title", stepKey: null, title: "Cardiac output", points: [], notes: "" },
        { key: "step:rate", kind: "step", stepKey: "rate", title: "Heart rate", points: [], notes: "" },
        { key: "step:volume", kind: "step", stepKey: "volume", title: "Stroke volume", points: ["Filling"], notes: "" },
      ],
    });
    renderPlayer();
    fireEvent.click(await screen.findByRole("button", { name: /View as lecture/u }));
    expect(await screen.findByText("Slide 3 of 3")).toBeTruthy();
    expect(lessonsIpc.getLessonLecture).toHaveBeenCalledWith("lesson-1", 2);

    const breadcrumb = within(screen.getByRole("navigation", { name: "Breadcrumb" }));
    fireEvent.click(breadcrumb.getByRole("button", { name: /Cardiac output/u }));
    expect(await screen.findByRole("heading", { level: 1, name: "Stroke volume" })).toBeTruthy();
  });

  it("says when the lesson cannot open", async () => {
    vi.mocked(lessonsIpc.getLesson).mockRejectedValue(new Error("Lesson is gone"));
    const props = renderPlayer();
    expect(await screen.findByText(/This lesson could not open: Lesson is gone/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Cardiac physiology/u }));
    expect(props.onClose).toHaveBeenCalledOnce();
  });
});
