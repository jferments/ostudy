import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as studyMaterials from "../../ipc/studyMaterials";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { SubjectStudyActions } from "./SubjectStudyActions";

vi.mock("../../ipc/studyMaterials", () => ({
  beginStudyTargetBundle: vi.fn(),
  composeStudyTargetBundle: vi.fn(),
  cancelStudyTargetBundle: vi.fn(),
  getStudyTargetBundle: vi.fn(),
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
  });

  it("asks what to make before it starts, with both chosen", async () => {
    renderActions();
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));

    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });
    expect(studyMaterials.beginStudyTargetBundle).not.toHaveBeenCalled();
    const guide = within(sheet).getByRole("checkbox", { name: /Study guide/u });
    const cards = within(sheet).getByRole("checkbox", { name: /Flashcards/u });
    expect(guide.getAttribute("aria-checked")).toBe("true");
    expect(cards.getAttribute("aria-checked")).toBe("true");

    fireEvent.click(cards);
    fireEvent.click(within(sheet).getByRole("button", { name: "Make" }));

    await waitFor(() => expect(studyMaterials.beginStudyTargetBundle).toHaveBeenCalledWith(expect.objectContaining({
      target: { kind: "collection", id: subjectId },
      outputPolicy: { flashcards: false, studyGuide: true },
    })));
  });

  it("will not make nothing", async () => {
    renderActions();
    fireEvent.click(await screen.findByRole("button", { name: /Make study materials/u }));
    const sheet = await screen.findByRole("dialog", { name: "Make study materials" });

    fireEvent.click(within(sheet).getByRole("checkbox", { name: /Study guide/u }));
    fireEvent.click(within(sheet).getByRole("checkbox", { name: /Flashcards/u }));

    expect((within(sheet).getByRole("button", { name: "Make" }) as HTMLButtonElement).disabled).toBe(true);
  });
});
