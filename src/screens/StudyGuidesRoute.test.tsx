import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { LayoutModeProvider } from "../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import * as studyGuides from "../ipc/studyGuides";
import { StudyGuidesRoute } from "./StudyGuidesRoute";

vi.mock("../ipc/studyGuides", async () => {
  const actual = await vi.importActual<typeof import("../ipc/studyGuides")>(
    "../ipc/studyGuides",
  );
  return {
    ...actual,
    listDocumentStudyGuidePage: vi.fn().mockResolvedValue({
      items: [],
      totalCount: 0,
      offset: 0,
      limit: 40,
    }),
    deleteDocumentStudyGuide: vi.fn(),
  };
});

vi.mock("./StudyGuideDialog", () => ({
  StudyGuideDialog: () => null,
}));

describe("StudyGuidesRoute", () => {
  it("renders the first-class lazy route workspace", async () => {
    render(
      <OverlayProvider>
        <LayoutModeProvider>
          <StudyGuidesRoute showToast={vi.fn()} onPracticeCards={vi.fn()} />
        </LayoutModeProvider>
      </OverlayProvider>,
    );

    expect(await screen.findByRole("heading", { name: "Study Guides" })).toBeTruthy();
    expect(screen.getByRole("search")).toBeTruthy();
    expect(studyGuides.listDocumentStudyGuidePage).toHaveBeenCalledWith({
      query: "",
      sort: "recent",
      offset: 0,
      limit: 40,
    });
  });
});
