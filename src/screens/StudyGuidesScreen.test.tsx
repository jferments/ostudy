import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as studyGuides from "../ipc/studyGuides";
import { LayoutModeProvider } from "../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { StudyGuidesScreen } from "./StudyGuidesScreen";

vi.mock("../ipc/studyGuides", async () => {
  const actual = await vi.importActual<typeof import("../ipc/studyGuides")>(
    "../ipc/studyGuides",
  );
  return {
    ...actual,
    listDocumentStudyGuidePage: vi.fn(),
    deleteDocumentStudyGuide: vi.fn(),
  };
});

vi.mock("./StudyGuideDialog", () => ({
  StudyGuideDialog: ({
    documentTitle,
    onClose,
  }: {
    documentTitle: string;
    onClose: () => void;
  }) => (
    <div role="dialog" aria-label={`Guide ${documentTitle}`}>
      <button type="button" onClick={onClose}>
        Close guide
      </button>
    </div>
  ),
}));

const originalMatchMedia = window.matchMedia;

function page(
  overrides: Partial<studyGuides.StudyGuideLibraryPage> = {},
): studyGuides.StudyGuideLibraryPage {
  return {
    items: [
      {
        guideId: "b".repeat(32),
        documentId: "a".repeat(32),
        documentTitle: "Distributive shock",
        originalFilename: "shock.pdf",
        sourceFormat: "pdf",
        sourceUnitKind: "page",
        sourceUnitCount: 12,
        guideRevision: 3,
        sectionCount: 6,
        noteCount: 2,
        publishedAt: 10,
      },
    ],
    totalCount: 1,
    offset: 0,
    limit: 40,
    ...overrides,
  };
}

function renderScreen(
  overrides: Partial<ComponentProps<typeof StudyGuidesScreen>> = {},
) {
  return render(
    <LayoutModeProvider>
      <OverlayProvider>
        <StudyGuidesScreen
          showToast={vi.fn()}
          onPracticeCards={vi.fn()}
          {...overrides}
        />
      </OverlayProvider>
    </LayoutModeProvider>,
  );
}

describe("StudyGuidesScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setLayoutMode("wide");
    vi.mocked(studyGuides.listDocumentStudyGuidePage).mockResolvedValue(page());
    vi.mocked(studyGuides.deleteDocumentStudyGuide).mockResolvedValue({
      documentId: "a".repeat(32),
      guideId: "b".repeat(32),
      deletedGuideRevision: 3,
      removedRevisionCount: 2,
      removedOverrideCount: 1,
      removedCheckpointCount: 1,
      removedExportCount: 1,
      removedJobCount: 1,
      replayed: false,
    });
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
  });

  it("loads a bounded projection, searches, sorts, and opens through screen state", async () => {
    renderScreen();

    expect(await screen.findByText("Distributive shock")).toBeTruthy();
    expect(screen.getByText("6 sections")).toBeTruthy();
    expect(screen.getByText("2 notes")).toBeTruthy();
    expect(screen.queryByText("Revision 3")).toBeNull();
    expect(studyGuides.listDocumentStudyGuidePage).toHaveBeenCalledWith({
      query: "",
      sort: "recent",
      offset: 0,
      limit: 40,
    });

    fireEvent.change(screen.getByRole("searchbox", { name: "Search guides" }), {
      target: { value: "shock" },
    });
    fireEvent.submit(screen.getByRole("search"));
    await waitFor(() =>
      expect(studyGuides.listDocumentStudyGuidePage).toHaveBeenLastCalledWith({
        query: "shock",
        sort: "recent",
        offset: 0,
        limit: 40,
      }),
    );

    fireEvent.change(screen.getByRole("combobox", { name: "Sort" }), {
      target: { value: "title_asc" },
    });
    await waitFor(() =>
      expect(studyGuides.listDocumentStudyGuidePage).toHaveBeenLastCalledWith({
        query: "shock",
        sort: "title_asc",
        offset: 0,
        limit: 40,
      }),
    );

    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(screen.getByRole("dialog", { name: "Guide Distributive shock" })).toBeTruthy();
  });

  it("deletes through explicit preservation copy and refreshes the projection", async () => {
    const showToast = vi.fn();
    renderScreen({ showToast });
    await screen.findByText("Distributive shock");

    fireEvent.click(
      screen.getByRole("button", {
        name: "Delete Distributive shock study guide",
      }),
    );
    const confirmation = await screen.findByRole("alertdialog", {
      name: "Delete “Distributive shock” study guide?",
    });
    expect(confirmation.textContent).toContain("linked flashcards");
    expect(confirmation.textContent).toContain("stable guide/topic notes");
    fireEvent.click(screen.getByRole("button", { name: "Delete study guide" }));

    await waitFor(() =>
      expect(studyGuides.deleteDocumentStudyGuide).toHaveBeenCalledWith(
        "a".repeat(32),
        "b".repeat(32),
        3,
        expect.stringMatching(/^[0-9a-f]{32}$/u),
      ),
    );
    expect(showToast).toHaveBeenCalledWith(
      "success",
      expect.stringContaining("stable notes were preserved"),
    );
    expect(vi.mocked(studyGuides.listDocumentStudyGuidePage).mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it("opens a newly generated guide request without hydrating every list row first", async () => {
    const onConsumed = vi.fn();
    vi.mocked(studyGuides.listDocumentStudyGuidePage).mockResolvedValue(
      page({ items: [], totalCount: 0 }),
    );
    renderScreen({
      openRequest: {
        requestId: 17,
        documentId: "a".repeat(32),
        documentTitle: "Cardiac output",
      },
      onOpenRequestConsumed: onConsumed,
    });

    expect(await screen.findByRole("dialog", { name: "Guide Cardiac output" })).toBeTruthy();
    expect(onConsumed).toHaveBeenCalledWith(17);
  });
});

function setLayoutMode(layoutMode: "wide" | "medium" | "compact"): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches:
      (query === "(max-width: 760px)" && layoutMode === "compact") ||
      (query === "(max-width: 1180px)" && layoutMode !== "wide"),
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}