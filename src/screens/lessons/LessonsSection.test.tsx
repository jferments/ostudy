import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { LessonSummary } from "../../ipc/lessons";
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
      ],
    },
    loading: false,
    readError: null,
    writing: false,
    stopping: false,
    progress: null,
    writeError: null,
    updating: null,
    updateError: null,
    refresh: vi.fn(),
    update: vi.fn(),
    write: vi.fn(),
    stop: vi.fn(),
    dismissWriteError: vi.fn(),
    ...overrides,
  };
}

describe("LessonsSection", () => {
  it("offers to update only lessons written with AI whose sources changed or went", () => {
    const value = course();
    render(<LessonsSection headingId="h" course={value} hasSources onOpenLesson={vi.fn()} />);
    const rows = screen.getAllByTestId("subject-lesson");
    expect(within(rows[0]!).queryByRole("button", { name: /Update/ })).toBeNull();
    expect(within(rows[1]!).getByText("Source changed · 8 min")).toBeTruthy();
    expect(within(rows[2]!).queryByRole("button", { name: /Update/ })).toBeNull();
    expect(within(rows[3]!).getByText("Source removed · 8 min")).toBeTruthy();

    fireEvent.click(within(rows[1]!).getByRole("button", { name: "Update Ejection from its sources" }));
    expect(value.update).toHaveBeenCalledWith("two");
  });

  it("shows the lesson being updated and why an update failed", () => {
    render(
      <LessonsSection
        headingId="h"
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
});
