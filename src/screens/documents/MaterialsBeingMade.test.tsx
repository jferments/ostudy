import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as lessons from "../../ipc/lessons";
import { COURSE_PROGRESS_INTERVAL_MS, resetCourseRunsForTests, startSubjectCourse } from "../lessons/useSubjectCourse";
import { SubjectMaterialBeingMade } from "./MaterialsBeingMade";
import { queueStudySet, resetStudySetsForTests, takeStudySet } from "./pendingStudySets";
import { resetStudyTargetBundleRuns } from "./useStudyTargetBundleController";

vi.mock("../../ipc/lessons", () => ({
  listSubjectLessons: vi.fn(),
  writeSubjectCourse: vi.fn(),
  getSubjectCourseWritingProgress: vi.fn(),
  cancelSubjectCourseWriting: vi.fn(),
}));
vi.mock("../../ipc/studyMaterials", () => ({ getStudyTargetBundle: vi.fn() }));

const subjectId = "c".repeat(32);

describe("SubjectMaterialBeingMade", () => {
  afterEach(() => { vi.useRealTimers(); });
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(lessons.writeSubjectCourse).mockReturnValue(new Promise(() => undefined));
    vi.mocked(lessons.getSubjectCourseWritingProgress).mockResolvedValue({ planned: 8, written: 3 });
    resetCourseRunsForTests();
    resetStudySetsForTests();
    resetStudyTargetBundleRuns();
  });

  it("shows nothing when nothing is on its way", () => {
    render(<SubjectMaterialBeingMade subjectId={subjectId} kind="podcast" />);
    expect(screen.queryByTestId("material-being-made")).toBeNull();
  });

  it("follows a podcast from its documents being read to its lessons being written", async () => {
    vi.useFakeTimers();
    queueStudySet(subjectId, { flashcards: true, studyGuide: true, lessons: true, podcast: true, documentIds: ["d".repeat(32)] });
    render(<SubjectMaterialBeingMade subjectId={subjectId} kind="podcast" />);
    expect(screen.getByText("Podcast being made…")).toBeTruthy();
    expect(screen.getByText("Reading the document first")).toBeTruthy();

    act(() => {
      takeStudySet(subjectId);
      startSubjectCourse(subjectId, { podcast: true });
    });
    expect(screen.getByText("Waiting for the lessons to be planned")).toBeTruthy();
    await act(async () => { await vi.advanceTimersByTimeAsync(COURSE_PROGRESS_INTERVAL_MS); });
    expect(screen.getByText("Waiting for the lessons: 3 of 8 written")).toBeTruthy();
  });

  it("leaves out a podcast nobody asked for", () => {
    queueStudySet(subjectId, { flashcards: true, studyGuide: true, lessons: true, podcast: false, documentIds: ["d".repeat(32)] });
    render(<SubjectMaterialBeingMade subjectId={subjectId} kind="podcast" />);
    expect(screen.queryByTestId("material-being-made")).toBeNull();
  });
});
