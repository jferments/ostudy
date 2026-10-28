import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { LessonSummary } from "../../ipc/lessons";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { LessonsSection } from "./LessonsSection";
import type { SubjectCourse } from "./useSubjectCourse";

function lesson(
  id: string,
  title: string,
  sources: "current" | "changed" | "unavailable",
  origin = "generated",
): LessonSummary {
  return {
    id,
    subjectId: "cardiac",
    title,
    revision: 1,
    written: {
      revisionNumber: 1,
      origin,
      stepCount: 4,
      checkCount: 2,
      estimatedMinutes: 8,
      sources,
      inStudyPlan: id !== "five",
      createdAt: 1,
    },
    progress: null,
  } as LessonSummary;
}

function course(overrides: Partial<SubjectCourse> = {}): SubjectCourse {
  return {
    course: {
      subjectId: "cardiac",
      lessons: [
        lesson("one", "Filling", "current"),
        lesson("two", "Ejection", "changed"),
        lesson("three", "Valves", "changed", "authored"),
        lesson("four", "Pressure", "unavailable"),
        lesson("five", "Old topic", "changed"),
      ],
    },
    loading: false,
    readError: null,
    writing: false,
    stopping: false,
    progress: null,
    writeError: null,
    writingPodcast: false,
    podcastError: null,
    audio: null,
    audioError: null,
    updating: null,
    updateError: null,
    refresh: vi.fn(),
    update: vi.fn(),
    write: vi.fn(),
    rewrite: vi.fn(),
    stop: vi.fn(),
    dismissWriteError: vi.fn(),
    ...overrides,
  };
}

describe("LessonsSection", () => {
  it("offers to update only lessons written with AI whose sources changed or went", () => {
    const value = course();
    render(<LessonsSection headingId="h" subjectId="s" course={value} hasSources onOpenLesson={vi.fn()} />);
    const rows = screen.getAllByTestId("subject-lesson");
    expect(within(rows[0]!).queryByRole("button", { name: /Update/ })).toBeNull();
    expect(within(rows[1]!).getByText("Source changed · 8 min")).toBeTruthy();
    expect(within(rows[2]!).queryByRole("button", { name: /Update/ })).toBeNull();
    expect(within(rows[3]!).getByText("Source removed · 8 min")).toBeTruthy();
    // A lesson whose part left the study plan cannot be updated from it.
    expect(within(rows[4]!).getByText("No longer in the study plan")).toBeTruthy();
    expect(within(rows[4]!).queryByRole("button", { name: /Update/ })).toBeNull();

    fireEvent.click(within(rows[1]!).getByRole("button", { name: "Update Ejection from its sources" }));
    expect(value.update).toHaveBeenCalledWith("two");
  });

  it("shows the lesson being updated and why an update failed", () => {
    render(
      <LessonsSection
        headingId="h"
        subjectId="s"
        course={course({ updating: "two", updateError: "The part is gone." })}
        hasSources
        onOpenLesson={vi.fn()}
      />,
    );
    expect(screen.getByText("Updating…")).toBeTruthy();
    expect(screen.getByText("The lesson was not updated: The part is gone.")).toBeTruthy();
    for (const button of screen.getAllByRole("button", { name: /from its sources/ })) {
      expect((button as HTMLButtonElement).disabled).toBe(true);
    }
  });

  it("rewrites the course once the learner confirms the cost", async () => {
    const value = course();
    render(
      <OverlayProvider>
        <LessonsSection headingId="h" subjectId="s" course={value} hasSources onOpenLesson={vi.fn()} />
      </OverlayProvider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Rewrite course" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText(/costs about as much as writing the course did/)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "Rewrite course" }));
    await waitFor(() => expect(value.rewrite).toHaveBeenCalled());
  });
});
