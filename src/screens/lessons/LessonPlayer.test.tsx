import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as lessonsIpc from "../../ipc/lessons";
import { LessonPlayer } from "./LessonPlayer";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/lessons", () => ({
  getLesson: vi.fn(),
  saveLessonProgress: vi.fn(),
}));
// The lecture has its own tests; here it reports the step it shows and finishes.
vi.mock("./LectureView", () => ({
  LectureView: (props: {
    stepKey: string | null;
    position?: string;
    backLabel: string;
    onStepShown?: (stepKey: string) => void;
    onFinish?: () => void;
    onClose: () => void;
  }) => (
    <div data-testid="lecture-view">
      <p>Opens at {props.stepKey ?? "the title"}</p>
      <p>{props.position}</p>
      <button type="button" onClick={() => props.onStepShown?.("rate")}>Show heart rate</button>
      <button type="button" onClick={() => props.onStepShown?.("volume")}>Show stroke volume</button>
      <button type="button" onClick={props.onFinish}>Finish lesson</button>
      <button type="button" onClick={props.onClose}>{props.backLabel}</button>
    </div>
  ),
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

  it("opens the lecture where the learner stopped and saves each new step it shows", async () => {
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue(makeLesson(makeProgress({ currentStep: 1, furthestStep: 1 })));
    renderPlayer();
    expect(await screen.findByText("Opens at volume")).toBeTruthy();
    expect(screen.getByText("Lesson 2 of 5")).toBeTruthy();

    // Showing the step it opened at saves nothing new.
    fireEvent.click(screen.getByRole("button", { name: "Show stroke volume" }));
    expect(lessonsIpc.saveLessonProgress).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Show heart rate" }));
    expect(lessonsIpc.saveLessonProgress).toHaveBeenCalledWith("lesson-1", 2, 0, false);
  });

  it("opens a new lesson on its cover slide", async () => {
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue(makeLesson(null));
    renderPlayer();
    expect(await screen.findByText("Opens at the title")).toBeTruthy();
  });

  it("opens a finished lesson on its cover and finishing saves then closes", async () => {
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue(
      makeLesson(makeProgress({ currentStep: 1, furthestStep: 1, finishedAt: 5 })),
    );
    const props = renderPlayer();
    expect(await screen.findByText("Opens at the title")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Finish lesson" }));
    await vi.waitFor(() => expect(props.onClose).toHaveBeenCalledOnce());
    expect(lessonsIpc.saveLessonProgress).toHaveBeenLastCalledWith("lesson-1", 2, 1, true);
  });

  it("tells the learner when their place was not saved", async () => {
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue(makeLesson(null));
    vi.mocked(lessonsIpc.saveLessonProgress).mockRejectedValue(new Error("Disk full"));
    const props = renderPlayer();
    fireEvent.click(await screen.findByRole("button", { name: "Show stroke volume" }));
    await vi.waitFor(() =>
      expect(props.showToast).toHaveBeenCalledWith("error", "Your place in this lesson was not saved: Disk full"),
    );
  });

  it("goes back to the subject's lessons", async () => {
    vi.mocked(lessonsIpc.getLesson).mockResolvedValue(makeLesson(null));
    const props = renderPlayer();
    await screen.findByTestId("lecture-view");
    fireEvent.click(screen.getByRole("button", { name: "Cardiac physiology" }));
    expect(props.onClose).toHaveBeenCalledOnce();
  });

  it("says when the lesson cannot open", async () => {
    vi.mocked(lessonsIpc.getLesson).mockRejectedValue(new Error("Lesson is gone"));
    const props = renderPlayer();
    expect(await screen.findByText(/This lesson could not open: Lesson is gone/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Cardiac physiology/u }));
    expect(props.onClose).toHaveBeenCalledOnce();
  });
});
