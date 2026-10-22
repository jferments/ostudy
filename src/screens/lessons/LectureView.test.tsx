import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  publishPreparedExport,
  releasePreparedExport,
} from "../../ipc/externalFiles";
import {
  answerLectureQuestion,
  askLectureQuestion,
  answerLessonCheck,
  getLessonLecture,
  getLessonSlideDesigns,
  getLessonLectureScript,
  narrateLecturePart,
  narrateLessonStep,
  prepareLessonLecturePptx,
  writeLessonLectureScript,
  type AnsweredCheck,
  type Lecture,
  type LectureScript,
  type LessonStep,
} from "../../ipc/lessons";
import { getMaterialInk } from "../../ipc/materialInk";
import { getSpeechSettings } from "../../ipc/speech";
import {
  createManagedMediaObjectUrl,
  revokeManagedMediaObjectUrl,
} from "../../media/managedPlayback";
import { layoutWidths, matchesLayoutQuery } from "../../test/viewport";
import { LayoutModeProvider } from "../../ui/layout/LayoutModeProvider";
import { LectureView } from "./LectureView";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) =>
    error instanceof Error ? error.message : "Unexpected error",
}));
vi.mock("../../ipc/lessons", () => ({
  answerLectureQuestion: vi.fn(),
  askLectureQuestion: vi.fn(),
  answerLessonCheck: vi.fn(),
  getLessonLecture: vi.fn(),
  getLessonSlideDesigns: vi.fn(),
  getLessonLectureScript: vi.fn(),
  narrateLecturePart: vi.fn(),
  narrateLessonStep: vi.fn(),
  prepareLessonLecturePptx: vi.fn(),
  writeLessonLectureScript: vi.fn(),
}));
vi.mock("../../ipc/speech", () => ({ getSpeechSettings: vi.fn() }));
vi.mock("../../ipc/materialInk", () => ({ getMaterialInk: vi.fn() }));
vi.mock("../ink/InkEditor", () => ({
  InkEditor: ({ pageKey, pages, onClose }: { pageKey?: string; pages: { key: string }[]; onClose: () => void }) => (
    <div>
      <p>Writing on {pages.length} slides from {pageKey}</p>
      <button type="button" onClick={onClose}>Back to the lecture</button>
    </div>
  ),
}));
vi.mock("./SlideCards", () => ({
  SlideCards: ({ subjectId, step }: { subjectId: string; step: { key: string } }) => (
    <p>Cards for {step.key} in {subjectId}</p>
  ),
}));
vi.mock("../../ipc/externalFiles", () => ({
  publishPreparedExport: vi.fn(),
  releasePreparedExport: vi.fn(),
}));
vi.mock("../../media/managedPlayback", () => ({
  createManagedMediaObjectUrl: vi.fn(),
  revokeManagedMediaObjectUrl: vi.fn(),
  playbackErrorMessage: (failure: unknown) => (failure instanceof Error ? failure.message : String(failure)),
}));

const lecture: Lecture = {
  lessonId: "lesson-1",
  revisionNumber: 2,
  title: "Loop of Henle",
  slides: [
    {
      key: "title",
      kind: "title",
      stepKey: null,
      title: "Loop of Henle",
      points: ["Explain the gradient"],
      notes: "How the loop works.",
    },
    {
      key: "step:two-limbs",
      kind: "step",
      stepKey: "two-limbs",
      title: "Two limbs, two jobs",
      points: ["Water out going down"],
      notes: "Picture two tubes.",
    },
    {
      key: "step:quiet",
      kind: "step",
      stepKey: "quiet",
      title: "A quiet step",
      points: [],
      notes: "",
    },
    {
      key: "step:multiplier",
      kind: "step",
      stepKey: "multiplier",
      title: "The multiplier",
      points: ["Small steps stack"],
      notes: "Each pump adds a little.",
    },
    {
      key: "sources",
      kind: "sources",
      stepKey: null,
      title: "Sources",
      points: ["Renal textbook"],
      notes: "",
    },
  ],
};

const multiplier: LessonStep = {
  key: "multiplier",
  title: "The multiplier",
  blocks: [],
  narration: "Each pump adds a little.",
  slidePoints: ["Small steps stack"],
  checkKind: "single_choice",
  check: {
    prompt: "What builds the gradient?",
    options: [
      { id: "a", text: "Small steps stacking" },
      { id: "b", text: "One big pump" },
    ],
    timing: "after",
  },
  knowledgeUnitIds: [],
};

function open(
  stepKey: string | null = null,
  onAnswered = vi.fn(),
  answered: Record<string, AnsweredCheck> = {},
) {
  const onClose = vi.fn();
  render(
    <LayoutModeProvider>
      <LectureView
        lessonId="lesson-1"
        revisionNumber={2}
        lessonTitle="Loop of Henle"
        subjectId="subject-1"
        stepKey={stepKey}
        steps={[multiplier]}
        answered={answered}
        onAnswered={onAnswered}
        onClose={onClose}
        showToast={vi.fn()}
      />
    </LayoutModeProvider>,
  );
  return onClose;
}

function setViewport(width: number): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: matchesLayoutQuery(query, width),
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

function slide() {
  return within(screen.getByTestId("lecture-slide"));
}

function audio(): HTMLAudioElement {
  return document.querySelector("audio")!;
}

describe("LectureView", () => {
  let play: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    vi.mocked(getLessonLecture).mockReset();
    vi.mocked(getLessonLecture).mockResolvedValue(lecture);
    vi.mocked(getLessonSlideDesigns).mockReset();
    vi.mocked(getLessonSlideDesigns).mockImplementation(async (lessonId, revisionNumber, theme) => ({
      lessonId,
      revisionNumber,
      theme,
      slides: lecture.slides.map((item) => ({ key: item.key, svg: `<svg data-theme="${theme}"/>` })),
    }));
    let made = 0;
    Object.defineProperty(URL, "createObjectURL", {
      configurable: true,
      writable: true,
      value: vi.fn(() => `blob:design-${(made += 1)}`),
    });
    Object.defineProperty(URL, "revokeObjectURL", { configurable: true, writable: true, value: vi.fn() });
    window.localStorage.clear();
    vi.mocked(getMaterialInk).mockReset();
    vi.mocked(getMaterialInk).mockResolvedValue([]);
    vi.mocked(answerLessonCheck).mockReset();
    vi.mocked(getSpeechSettings).mockResolvedValue({
      profileId: "r".repeat(32),
      modelId: "fish-audio/drama-3",
      voice: "",
      revision: 0,
      models: [],
      connections: [],
      voices: [],
    });
    vi.mocked(getLessonLectureScript).mockReset();
    vi.mocked(getLessonLectureScript).mockResolvedValue(null);
    vi.mocked(writeLessonLectureScript).mockReset();
    vi.mocked(answerLectureQuestion).mockReset();
    vi.mocked(narrateLecturePart).mockReset();
    vi.mocked(narrateLecturePart).mockImplementation(
      async (_lesson, _revision, slideKey, replyTo) => ({
        segments: [
          {
            assetId: replyTo ? `reply:${replyTo}` : `talk:${slideKey}`,
            characters: 20,
          },
        ],
        made: 1,
      }),
    );
    vi.mocked(askLectureQuestion).mockReset();
    vi.mocked(askLectureQuestion).mockImplementation(
      async (_lesson, _revision, slideKey) => ({
        segments: [{ assetId: `ask:${slideKey}`, characters: 20 }],
        made: 1,
      }),
    );
    vi.mocked(narrateLessonStep).mockReset();
    vi.mocked(narrateLessonStep).mockImplementation(
      async (_lesson, _revision, stepKey) => ({
        segments: [{ assetId: stepKey, characters: 20 }],
        made: 1,
      }),
    );
    vi.mocked(createManagedMediaObjectUrl).mockReset();
    vi.mocked(revokeManagedMediaObjectUrl).mockReset();
    vi.mocked(createManagedMediaObjectUrl).mockImplementation(
      async (assetId) => `blob:${assetId}`,
    );
    play = vi.fn(() => Promise.resolve());
    vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(
      play as () => Promise<void>,
    );
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(
      () => undefined,
    );
  });

  const originalMatchMedia = window.matchMedia;
  afterEach(() => {
    vi.restoreAllMocks();
    window.matchMedia = originalMatchMedia;
  });

  it("opens at the lesson's step with its narration as the notes, and moves between slides", async () => {
    open("multiplier");
    expect(await screen.findByText("Slide 4 of 5")).toBeTruthy();
    expect(getLessonLecture).toHaveBeenCalledWith("lesson-1", 2);
    expect(
      slide().getByRole("heading", { name: "The multiplier" }),
    ).toBeTruthy();
    expect(slide().getByText("Small steps stack")).toBeTruthy();
    expect(
      screen.getByRole("region", { name: "Speaker notes" }).textContent,
    ).toContain("Each pump adds a little.");

    fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    expect(slide().getByRole("heading", { name: "Sources" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Speaker notes" })).toBeNull();
    expect(
      (screen.getByRole("button", { name: /Next/ }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("plays from the first step, skips a step with nothing to say, and stops after the last", async () => {
    open();
    fireEvent.click(
      await screen.findByRole("button", { name: /Play lecture/ }),
    );
    await screen.findByRole("button", { name: /Stop/ });
    expect(narrateLessonStep).toHaveBeenLastCalledWith(
      "lesson-1",
      2,
      "two-limbs",
    );
    expect(
      slide().getByRole("heading", { name: "Two limbs, two jobs" }),
    ).toBeTruthy();
    expect(audio().src).toBe("blob:two-limbs");

    act(() => audio().dispatchEvent(new Event("ended")));
    expect(
      slide().getByRole("heading", { name: "The multiplier" }),
    ).toBeTruthy();
    await vi.waitFor(() => expect(audio().src).toBe("blob:multiplier"));
    expect(narrateLessonStep).not.toHaveBeenCalledWith("lesson-1", 2, "quiet");

    act(() => audio().dispatchEvent(new Event("ended")));
    expect(screen.getByRole("button", { name: /Play lecture/ })).toBeTruthy();
    expect(
      slide().getByRole("heading", { name: "The multiplier" }),
    ).toBeTruthy();
    expect(narrateLessonStep).toHaveBeenCalledTimes(2);
  });

  it("stops the voice when another slide is chosen", async () => {
    open("two-limbs");
    fireEvent.click(
      await screen.findByRole("button", { name: /Play lecture/ }),
    );
    await screen.findByRole("button", { name: /Stop/ });
    fireEvent.click(screen.getByRole("button", { name: /Back/ }));
    expect(
      slide().getByRole("heading", { name: "Loop of Henle" }),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: /Play lecture/ })).toBeTruthy();
    act(() => audio().dispatchEvent(new Event("ended")));
    expect(
      slide().getByRole("heading", { name: "Loop of Henle" }),
    ).toBeTruthy();
  });

  it("says why the lecture could not be read aloud and ends it", async () => {
    vi.mocked(narrateLessonStep).mockRejectedValue(
      new Error("Spoken narration needs an OpenRouter connection."),
    );
    open("two-limbs");
    fireEvent.click(
      await screen.findByRole("button", { name: /Play lecture/ }),
    );
    expect(
      await screen.findByText(
        /The lecture could not be read aloud: Spoken narration needs an OpenRouter connection\./,
      ),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: /Play lecture/ })).toBeTruthy();
  });

  it("says why a lecture could not open and goes back", async () => {
    vi.mocked(getLessonLecture).mockRejectedValue(
      new Error("That version of the lesson is no longer in the library."),
    );
    const onClose = open();
    expect(
      await screen.findByText(/This lecture could not open: That version/),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Loop of Henle/ }));
    expect(onClose).toHaveBeenCalled();
  });

  describe("saving the slides", () => {
    const prepared = {
      preparedExport: {
        id: "export-1",
        purpose: "lecture_pptx" as const,
        suggestedFilename: "Loop of Henle.pptx",
        mediaType:
          "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        byteSize: 2048,
        sha256: "f".repeat(64),
        expiresAt: 1,
      },
      revisionNumber: 2,
      slideCount: 5,
    };

    beforeEach(() => {
      vi.mocked(prepareLessonLecturePptx).mockReset();
      vi.mocked(publishPreparedExport).mockReset();
      vi.mocked(releasePreparedExport).mockReset();
      vi.mocked(releasePreparedExport).mockResolvedValue(undefined as never);
    });

    it("writes the revision it shows, saves it where the learner picks, and lets the copy go", async () => {
      vi.mocked(prepareLessonLecturePptx).mockResolvedValue(prepared);
      vi.mocked(publishPreparedExport).mockResolvedValue({
        preparedExportId: "export-1",
        displayName: "Loop of Henle.pptx",
        destinationKind: "file",
        completedAt: 1,
      });
      open();
      fireEvent.click(
        await screen.findByRole("button", { name: "Save as .pptx" }),
      );
      expect(await screen.findByText("Saved Loop of Henle.pptx.")).toBeTruthy();
      expect(prepareLessonLecturePptx).toHaveBeenCalledWith("lesson-1", 2);
      expect(publishPreparedExport).toHaveBeenCalledWith("export-1");
      expect(releasePreparedExport).toHaveBeenCalledWith("export-1");
    });

    it("says nothing when the save dialog is closed, and why when writing fails", async () => {
      vi.mocked(prepareLessonLecturePptx).mockResolvedValueOnce(prepared);
      vi.mocked(publishPreparedExport).mockResolvedValueOnce(null);
      open();
      const save = await screen.findByRole("button", { name: "Save as .pptx" });
      fireEvent.click(save);
      await screen.findByRole("button", { name: "Save as .pptx" });
      expect(releasePreparedExport).toHaveBeenCalledWith("export-1");
      expect(screen.queryByRole("status")).toBeNull();

      vi.mocked(prepareLessonLecturePptx).mockRejectedValueOnce(
        new Error("The lesson is no longer available."),
      );
      fireEvent.click(screen.getByRole("button", { name: "Save as .pptx" }));
      expect((await screen.findByRole("alert")).textContent).toContain(
        "The slides were not saved: The lesson is no longer available.",
      );
    });
  });

  it("offers no Play lecture without a lesson voice", async () => {
    vi.mocked(getSpeechSettings).mockResolvedValue({
      profileId: null,
      modelId: "fish-audio/drama-3",
      voice: "",
      revision: 0,
      models: [],
      connections: [],
      voices: [],
    });
    open();
    expect(await screen.findByText("Slide 1 of 5")).toBeTruthy();
    await waitFor(() => expect(getSpeechSettings).toHaveBeenCalled());
    expect(screen.queryByRole("button", { name: /Play lecture/ })).toBeNull();
    expect(screen.getByRole("button", { name: /Save as \.pptx/ })).toBeTruthy();
  });

  describe("the teacher's lecture", () => {
    const script: LectureScript = {
      revisionNumber: 2,
      slides: [
        {
          slideKey: "title",
          talk: "Welcome, everyone. Today is about the kidney's loop.",
          question: null,
        },
        {
          slideKey: "step:two-limbs",
          talk: "Look at the first point: the two limbs do opposite jobs.",
          question: {
            timing: "after",
            prompt: "Which limb lets water out?",
            options: [
              { id: "a".repeat(32), text: "The descending limb" },
              { id: "b".repeat(32), text: "The ascending limb" },
            ],
          },
        },
        {
          slideKey: "step:quiet",
          talk: "This slide is short, so let us move on.",
          question: null,
        },
        {
          slideKey: "step:multiplier",
          talk: "Each pump only adds a little.",
          question: null,
        },
        { slideKey: "sources", talk: "That is the loop.", question: null },
      ],
    };

    it("offers to write it, then shows what the teacher says as the notes", async () => {
      vi.mocked(writeLessonLectureScript).mockResolvedValue(script);
      open("two-limbs");
      expect(
        await screen.findByText(/stops to ask the class a few questions/),
      ).toBeTruthy();
      fireEvent.click(
        screen.getByRole("button", { name: /Write the teacher's lecture/ }),
      );
      expect(writeLessonLectureScript).toHaveBeenCalledWith("lesson-1", 2);
      expect(
        await screen.findByText(
          "Look at the first point: the two limbs do opposite jobs.",
        ),
      ).toBeTruthy();
      expect(
        screen.getByText(/the notes are the teacher's lecture/),
      ).toBeTruthy();
      expect(
        screen.queryByRole("button", { name: /Write the teacher's lecture/ }),
      ).toBeNull();
    });

    it("says why it was not written", async () => {
      vi.mocked(writeLessonLectureScript).mockRejectedValue(
        new Error("Add an AI connection first."),
      );
      open();
      fireEvent.click(
        await screen.findByRole("button", {
          name: /Write the teacher's lecture/,
        }),
      );
      expect(
        await screen.findByText(
          /The teacher's lecture was not written: Add an AI connection first\./,
        ),
      ).toBeTruthy();
    });

    it("talks over every slide, stops for the class's answer, replies and goes on", async () => {
      vi.mocked(getLessonLectureScript).mockResolvedValue(script);
      vi.mocked(answerLectureQuestion).mockResolvedValue({
        correct: false,
        correctOptionId: "a".repeat(32),
        reply: "Not quite. The ascending limb is watertight.",
      });
      open();
      await screen.findByText(/the notes are the teacher's lecture/);
      fireEvent.click(screen.getByRole("button", { name: /Play lecture/ }));
      await vi.waitFor(() => expect(audio().src).toBe("blob:talk:title"));
      expect(narrateLessonStep).not.toHaveBeenCalled();

      act(() => audio().dispatchEvent(new Event("ended")));
      await vi.waitFor(() =>
        expect(audio().src).toBe("blob:talk:step:two-limbs"),
      );
      act(() => audio().dispatchEvent(new Event("ended")));

      const dialog = within(
        await screen.findByRole("dialog", {
          name: "The teacher asks the class",
        }),
      );
      expect(dialog.getByText("Which limb lets water out?")).toBeTruthy();
      fireEvent.click(
        dialog.getByRole("radio", { name: "The ascending limb" }),
      );
      fireEvent.click(dialog.getByRole("button", { name: "Answer" }));
      expect(
        await dialog.findByText("Not quite. The ascending limb is watertight."),
      ).toBeTruthy();
      expect(answerLectureQuestion).toHaveBeenCalledWith(
        "lesson-1",
        2,
        "step:two-limbs",
        "b".repeat(32),
      );
      await vi.waitFor(() =>
        expect(audio().src).toBe(`blob:reply:${"b".repeat(32)}`),
      );
      expect(narrateLecturePart).toHaveBeenLastCalledWith(
        "lesson-1",
        2,
        "step:two-limbs",
        "b".repeat(32),
      );

      // The lecture goes on when the reply ends.
      act(() => audio().dispatchEvent(new Event("ended")));
      await vi.waitFor(() => expect(audio().src).toBe("blob:talk:step:quiet"));
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(
        slide().getByRole("heading", { name: "A quiet step" }),
      ).toBeTruthy();
    });

    it("asks a question before the talk aloud, then talks over the slide and goes on", async () => {
      const asking = structuredClone(script);
      asking.slides[1]!.question!.timing = "before";
      vi.mocked(getLessonLectureScript).mockResolvedValue(asking);
      vi.mocked(answerLectureQuestion).mockResolvedValue({
        correct: true,
        correctOptionId: "a".repeat(32),
        reply: "Right. Let us see why.",
      });
      open("two-limbs");
      await screen.findByText(/the notes are the teacher's lecture/);
      fireEvent.click(screen.getByRole("button", { name: /Play lecture/ }));
      await vi.waitFor(() =>
        expect(audio().src).toBe("blob:ask:step:two-limbs"),
      );
      expect(narrateLecturePart).not.toHaveBeenCalled();
      act(() => audio().dispatchEvent(new Event("ended")));

      const dialog = within(
        await screen.findByRole("dialog", { name: "What do you think?" }),
      );
      fireEvent.click(
        dialog.getByRole("radio", { name: "The descending limb" }),
      );
      fireEvent.click(dialog.getByRole("button", { name: "Answer" }));
      await vi.waitFor(() =>
        expect(audio().src).toBe(`blob:reply:${"a".repeat(32)}`),
      );

      // The slide's talk follows the reply, and the lecture goes on after it
      // without asking again.
      act(() => audio().dispatchEvent(new Event("ended")));
      await vi.waitFor(() =>
        expect(audio().src).toBe("blob:talk:step:two-limbs"),
      );
      expect(screen.queryByRole("dialog")).toBeNull();
      act(() => audio().dispatchEvent(new Event("ended")));
      await vi.waitFor(() => expect(audio().src).toBe("blob:talk:step:quiet"));
      expect(askLectureQuestion).toHaveBeenCalledTimes(1);
    });

    it("lets a question be answered without playing, and says nothing aloud", async () => {
      vi.mocked(getLessonLectureScript).mockResolvedValue(script);
      vi.mocked(answerLectureQuestion).mockResolvedValue({
        correct: true,
        correctOptionId: "a".repeat(32),
        reply: "Right. Water leaves the descending limb.",
      });
      open("two-limbs");
      fireEvent.click(
        await screen.findByRole("button", {
          name: /Answer the teacher's question/,
        }),
      );
      const dialog = within(await screen.findByRole("dialog"));
      fireEvent.click(
        dialog.getByRole("radio", { name: "The descending limb" }),
      );
      fireEvent.click(dialog.getByRole("button", { name: "Answer" }));
      expect(
        await dialog.findByText("Right. Water leaves the descending limb."),
      ).toBeTruthy();
      expect(narrateLecturePart).not.toHaveBeenCalled();
      fireEvent.click(dialog.getByRole("button", { name: "Done" }));
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(
        slide().getByRole("heading", { name: "Two limbs, two jobs" }),
      ).toBeTruthy();
    });
  });
  it("shows each slide designed in the chosen theme and remembers the theme", async () => {
    open("multiplier");
    await vi.waitFor(() => expect(screen.getByTestId("lecture-slide").querySelector("img")).toBeTruthy());
    expect(getLessonSlideDesigns).toHaveBeenLastCalledWith("lesson-1", 2, "navy");
    // The slide's text stays for screen readers beside its picture.
    expect(slide().getByRole("heading", { name: "The multiplier" })).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Slides" }).querySelectorAll("img")).toHaveLength(5);

    fireEvent.click(screen.getByRole("button", { name: "Theme: Navy" }));
    fireEvent.click(await screen.findByRole("menuitemcheckbox", { name: "Plum" }));
    await vi.waitFor(() => expect(getLessonSlideDesigns).toHaveBeenLastCalledWith("lesson-1", 2, "plum"));
    expect(await screen.findByRole("button", { name: "Theme: Plum" })).toBeTruthy();
    expect(window.localStorage.getItem("ostudy.lectureSlideTheme")).toBe("plum");
  });

  it("draws the learner's marks over a slide and opens the slides to write on at the current one", async () => {
    vi.mocked(getMaterialInk).mockResolvedValue([{
      id: "m".repeat(32),
      pageKey: "step:multiplier",
      shape: { kind: "line", x1: 10, y1: 10, x2: 200, y2: 90, color: "#d64545", strokeWidth: 3 },
      revision: 1,
      createdAt: 1,
      updatedAt: 1,
    }]);
    open("multiplier");
    await vi.waitFor(() => expect(screen.getByTestId("lecture-slide").querySelector("[data-ink-overlay]")).toBeTruthy());
    expect(getMaterialInk).toHaveBeenCalledWith({ kind: "lesson", lessonId: "lesson-1" });

    fireEvent.click(screen.getByRole("button", { name: "Write on slides" }));
    expect(await screen.findByText("Writing on 5 slides from step:multiplier")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Back to the lecture" }));
    expect(await screen.findByText("Slide 4 of 5")).toBeTruthy();
    expect(getMaterialInk).toHaveBeenCalledTimes(2);
  });

  it("shows the plain slide when the designed slides cannot be laid out", async () => {
    vi.mocked(getLessonSlideDesigns).mockRejectedValue(new Error("no fonts"));
    open("multiplier");
    expect(await screen.findByText("Slide 4 of 5")).toBeTruthy();
    await vi.waitFor(() => expect(getLessonSlideDesigns).toHaveBeenCalled());
    expect(screen.getByTestId("lecture-slide").querySelector("img")).toBeNull();
    expect(slide().getByText("Small steps stack")).toBeTruthy();
  });

  it("answers a step's check beside its slide", async () => {
    const onAnswered = vi.fn();
    const result = {
      correct: true,
      correctOptionId: "a",
      explanation: "Each pass adds a little.",
      progress: { revisionNumber: 2, currentStep: 1, furthestStep: 1, finishedAt: null, checks: {} },
    };
    vi.mocked(answerLessonCheck).mockResolvedValue(result as never);
    open("multiplier", onAnswered);
    const check = within(await screen.findByTestId("lesson-check"));
    fireEvent.click(check.getByRole("radio", { name: "Small steps stacking" }));
    fireEvent.click(check.getByRole("button", { name: "Check answer" }));
    expect(await check.findByText("Each pass adds a little.")).toBeTruthy();
    expect(answerLessonCheck).toHaveBeenCalledWith("lesson-1", 2, "multiplier", "a");
    expect(onAnswered).toHaveBeenCalledWith(result);

    fireEvent.click(screen.getByRole("button", { name: /Back/ }));
    expect(screen.queryByTestId("lesson-check")).toBeNull();
  });

  it("offers a step's cards beside its slide, for the lesson's subject", async () => {
    open("multiplier");
    expect(await screen.findByText("Cards for multiplier in subject-1")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Back/ }));
    expect(screen.queryByText(/Cards for/)).toBeNull();
  });

  it("on a phone, opens a step's check in a sheet and goes on to the next slide from it", async () => {
    setViewport(layoutWidths.compact);
    const result = {
      correct: true,
      correctOptionId: "a",
      explanation: "Each pass adds a little.",
      progress: { revisionNumber: 2, currentStep: 1, furthestStep: 1, finishedAt: null, checks: {} },
    };
    vi.mocked(answerLessonCheck).mockResolvedValue(result as never);
    open("multiplier", vi.fn(), { multiplier: { optionId: "b", correct: false } });
    expect(await screen.findByText("Answered, not quite")).toBeTruthy();
    expect(screen.queryByTestId("lesson-check")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Check yourself" }));
    const sheet = within(await screen.findByRole("dialog", { name: "Check yourself" }));
    fireEvent.click(sheet.getByRole("radio", { name: "Small steps stacking" }));
    fireEvent.click(sheet.getByRole("button", { name: "Check answer" }));
    expect(await sheet.findByText("Each pass adds a little.")).toBeTruthy();

    fireEvent.click(sheet.getByRole("button", { name: /Next slide/ }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Check yourself" })).toBeNull());
    expect(screen.getByText("Slide 5 of 5")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Check yourself" })).toBeNull();
  });
});
