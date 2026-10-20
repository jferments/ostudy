import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { publishPreparedExport, releasePreparedExport } from "../../ipc/externalFiles";
import { getLessonLecture, narrateLessonStep, prepareLessonLecturePptx, type Lecture } from "../../ipc/lessons";
import { createManagedMediaObjectUrl, revokeManagedMediaObjectUrl } from "../../media/managedPlayback";
import { LectureView } from "./LectureView";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/lessons", () => ({
  getLessonLecture: vi.fn(),
  narrateLessonStep: vi.fn(),
  prepareLessonLecturePptx: vi.fn(),
}));
vi.mock("../../ipc/externalFiles", () => ({ publishPreparedExport: vi.fn(), releasePreparedExport: vi.fn() }));
vi.mock("../../media/managedPlayback", () => ({
  createManagedMediaObjectUrl: vi.fn(),
  revokeManagedMediaObjectUrl: vi.fn(),
}));

const lecture: Lecture = {
  lessonId: "lesson-1",
  revisionNumber: 2,
  title: "Loop of Henle",
  slides: [
    { kind: "title", stepKey: null, title: "Loop of Henle", points: ["Explain the gradient"], notes: "How the loop works." },
    { kind: "step", stepKey: "two-limbs", title: "Two limbs, two jobs", points: ["Water out going down"], notes: "Picture two tubes." },
    { kind: "step", stepKey: "quiet", title: "A quiet step", points: [], notes: "" },
    { kind: "step", stepKey: "multiplier", title: "The multiplier", points: ["Small steps stack"], notes: "Each pump adds a little." },
    { kind: "sources", stepKey: null, title: "Sources", points: ["Renal textbook"], notes: "" },
  ],
};

function open(stepKey: string | null = null) {
  const onClose = vi.fn();
  render(
    <LectureView lessonId="lesson-1" revisionNumber={2} lessonTitle="Loop of Henle" stepKey={stepKey} onClose={onClose} />,
  );
  return onClose;
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
    vi.mocked(narrateLessonStep).mockReset();
    vi.mocked(narrateLessonStep).mockImplementation(async (_lesson, _revision, stepKey) => ({
      segments: [{ assetId: stepKey, characters: 20 }],
      made: 1,
    }));
    vi.mocked(createManagedMediaObjectUrl).mockReset();
    vi.mocked(revokeManagedMediaObjectUrl).mockReset();
    vi.mocked(createManagedMediaObjectUrl).mockImplementation(async (assetId) => `blob:${assetId}`);
    play = vi.fn(() => Promise.resolve());
    vi.spyOn(HTMLMediaElement.prototype, "play").mockImplementation(play as () => Promise<void>);
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => undefined);
  });

  afterEach(() => vi.restoreAllMocks());

  it("opens at the lesson's step with its narration as the notes, and moves between slides", async () => {
    open("multiplier");
    expect(await screen.findByText("Slide 4 of 5")).toBeTruthy();
    expect(getLessonLecture).toHaveBeenCalledWith("lesson-1", 2);
    expect(slide().getByRole("heading", { name: "The multiplier" })).toBeTruthy();
    expect(slide().getByText("Small steps stack")).toBeTruthy();
    expect(screen.getByRole("region", { name: "Speaker notes" }).textContent).toContain("Each pump adds a little.");

    fireEvent.click(screen.getByRole("button", { name: /Next/ }));
    expect(slide().getByRole("heading", { name: "Sources" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Speaker notes" })).toBeNull();
    expect((screen.getByRole("button", { name: /Next/ }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("plays from the first step, skips a step with nothing to say, and stops after the last", async () => {
    open();
    fireEvent.click(await screen.findByRole("button", { name: /Play lecture/ }));
    await screen.findByRole("button", { name: /Stop/ });
    expect(narrateLessonStep).toHaveBeenLastCalledWith("lesson-1", 2, "two-limbs");
    expect(slide().getByRole("heading", { name: "Two limbs, two jobs" })).toBeTruthy();
    expect(audio().src).toBe("blob:two-limbs");

    act(() => audio().dispatchEvent(new Event("ended")));
    expect(slide().getByRole("heading", { name: "The multiplier" })).toBeTruthy();
    await vi.waitFor(() => expect(audio().src).toBe("blob:multiplier"));
    expect(narrateLessonStep).not.toHaveBeenCalledWith("lesson-1", 2, "quiet");

    act(() => audio().dispatchEvent(new Event("ended")));
    expect(screen.getByRole("button", { name: /Play lecture/ })).toBeTruthy();
    expect(slide().getByRole("heading", { name: "The multiplier" })).toBeTruthy();
    expect(narrateLessonStep).toHaveBeenCalledTimes(2);
  });

  it("stops the voice when another slide is chosen", async () => {
    open("two-limbs");
    fireEvent.click(await screen.findByRole("button", { name: /Play lecture/ }));
    await screen.findByRole("button", { name: /Stop/ });
    fireEvent.click(screen.getByRole("button", { name: /Back/ }));
    expect(slide().getByRole("heading", { name: "Loop of Henle" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Play lecture/ })).toBeTruthy();
    act(() => audio().dispatchEvent(new Event("ended")));
    expect(slide().getByRole("heading", { name: "Loop of Henle" })).toBeTruthy();
  });

  it("says why the lecture could not be read aloud and ends it", async () => {
    vi.mocked(narrateLessonStep).mockRejectedValue(new Error("Spoken narration needs an OpenRouter connection."));
    open("two-limbs");
    fireEvent.click(await screen.findByRole("button", { name: /Play lecture/ }));
    expect(
      await screen.findByText(/The lecture could not be read aloud: Spoken narration needs an OpenRouter connection\./),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: /Play lecture/ })).toBeTruthy();
  });

  it("says why a lecture could not open and goes back", async () => {
    vi.mocked(getLessonLecture).mockRejectedValue(new Error("That version of the lesson is no longer in the library."));
    const onClose = open();
    expect(await screen.findByText(/This lecture could not open: That version/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /Loop of Henle/ }));
    expect(onClose).toHaveBeenCalled();
  });

  describe("saving the slides", () => {
    const prepared = {
      preparedExport: {
        id: "export-1",
        purpose: "lecture_pptx" as const,
        suggestedFilename: "Loop of Henle.pptx",
        mediaType: "application/vnd.openxmlformats-officedocument.presentationml.presentation",
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
      fireEvent.click(await screen.findByRole("button", { name: "Save as .pptx" }));
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

      vi.mocked(prepareLessonLecturePptx).mockRejectedValueOnce(new Error("The lesson is no longer available."));
      fireEvent.click(screen.getByRole("button", { name: "Save as .pptx" }));
      expect(
        (await screen.findByRole("alert")).textContent,
      ).toContain("The slides were not saved: The lesson is no longer available.");
    });
  });
});
