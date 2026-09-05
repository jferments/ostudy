import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as studyGuides from "../ipc/studyGuides";
import { LayoutModeProvider } from "../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { StudyGuidesScreen } from "./StudyGuidesScreen";

vi.mock("../ipc/studyGuides", async () => {
  const actual = await vi.importActual<typeof import("../ipc/studyGuides")>(
    "../ipc/studyGuides",
  );
  return { ...actual, listDocumentStudyGuidePage: vi.fn() };
});

// Exercise the production paginated library, not its retired predecessor.
// The reader does not need to mount for this empty-state regression.
vi.mock("./StudyGuideDialog", () => ({ StudyGuideDialog: () => null }));

describe("Study Guides empty library", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(studyGuides.listDocumentStudyGuidePage).mockResolvedValue({
      items: [],
      totalCount: 0,
      offset: 0,
      limit: 40,
    });
  });

  it("directs learners to Documents without loading guide bodies or exposing pagination", async () => {
    render(
      <LayoutModeProvider>
        <OverlayProvider>
          <StudyGuidesScreen showToast={vi.fn()} onPracticeCards={vi.fn()} />
        </OverlayProvider>
      </LayoutModeProvider>,
    );

    expect(await screen.findByText("No study guides yet")).toBeTruthy();
    expect(screen.getByText(/Generate a guide from Documents/u)).toBeTruthy();
    expect(screen.queryByRole("navigation", { name: "Study Guides pages" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Open" })).toBeNull();
    expect(studyGuides.listDocumentStudyGuidePage).toHaveBeenCalledWith({
      query: "",
      sort: "recent",
      offset: 0,
      limit: 40,
    });
  });
});
