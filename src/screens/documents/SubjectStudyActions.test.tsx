import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as lessons from "../../ipc/lessons";
import * as studyMaterials from "../../ipc/studyMaterials";
import { resetCourseRunsForTests } from "../lessons/useSubjectCourse";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { SubjectStudyActions } from "./SubjectStudyActions";

vi.mock("../../ipc/studyMaterials", () => ({
  beginStudyTargetBundle: vi.fn(),
  composeStudyTargetBundle: vi.fn(),
  cancelStudyTargetBundle: vi.fn(),
  getStudyTargetBundle: vi.fn(),
}));

vi.mock("../../ipc/lessons", () => ({
  listSubjectLessons: vi.fn(),
  writeSubjectCourse: vi.fn(),
  getSubjectCourseWritingProgress: vi.fn(),
  cancelSubjectCourseWriting: vi.fn(),
}));

const subjectId = "c".repeat(32);

function renderActions() {
  return render(
    <OverlayProvider>
      <SubjectStudyActions subjectId={subjectId} refreshKey={1} hasDocuments />
    </OverlayProvider>,
  );
}

describe("SubjectStudyActions", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(studyMaterials.getStudyTargetBundle).mockResolvedValue(null);
    vi.mocked(studyMaterials.beginStudyTargetBundle).mockRejectedValue(new Error("stop here"));
    vi.mocked(lessons.listSubjectLessons).mockResolvedValue({ subjectId, lessons: [] });
    vi.mocked(lessons.writeSubjectCourse).mockReturnValue(new Promise(() => undefined));
    vi.mocked(lessons.getSubjectCourseWritingProgress).mockResolvedValue({ planned: null, written: 0 });
    resetCourseRunsForTests();
  });

  it("asks what to make, then makes flashcards, a study guide and lessons together", async () => {
    renderActions();
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));

    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });
    expect(studyMaterials.beginStudyTargetBundle).not.toHaveBeenCalled();
    for (const name of ["Flashcards", "Study guide", "Lessons"]) {
      expect(within(sheet).getByRole("checkbox", { name }).getAttribute("aria-checked")).toBe("true");
    }

    fireEvent.click(within(sheet).getByRole("button", { name: "Make" }));

    await waitFor(() => expect(studyMaterials.beginStudyTargetBundle).toHaveBeenCalledWith(expect.objectContaining({
      target: { kind: "collection", id: subjectId },
      outputPolicy: { flashcards: true, studyGuide: true },
    })));
    expect(lessons.writeSubjectCourse).toHaveBeenCalledWith(subjectId, expect.any(String), false);
    expect(await screen.findByText("Writing lessons")).toBeTruthy();
  });

  it("makes only what is ticked", async () => {
    renderActions();
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));
    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });

    fireEvent.click(within(sheet).getByRole("checkbox", { name: "Flashcards" }));
    fireEvent.click(within(sheet).getByRole("checkbox", { name: "Study guide" }));
    fireEvent.click(within(sheet).getByRole("button", { name: "Make" }));

    await waitFor(() => expect(lessons.writeSubjectCourse).toHaveBeenCalled());
    expect(studyMaterials.beginStudyTargetBundle).not.toHaveBeenCalled();
  });

  it("offers only what is not made yet", async () => {
    vi.mocked(lessons.listSubjectLessons).mockResolvedValue({
      subjectId,
      lessons: [{ id: "l".repeat(32) } as lessons.SubjectLessons["lessons"][number]],
    });
    renderActions();
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));
    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });

    const made = within(sheet).getByRole("checkbox", { name: /^Lessons/u });
    expect(made.getAttribute("aria-checked")).toBe("false");
    expect(made.getAttribute("aria-disabled") ?? String((made as HTMLInputElement).disabled)).toBe("true");
    expect(within(sheet).getByText("Made")).toBeTruthy();
  });

  it("will not make nothing", async () => {
    renderActions();
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));
    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });

    for (const name of ["Flashcards", "Study guide", "Lessons"]) {
      fireEvent.click(within(sheet).getByRole("checkbox", { name }));
    }

    expect((within(sheet).getByRole("button", { name: "Make" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
