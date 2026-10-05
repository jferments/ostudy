import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as studyGuides from "../ipc/studyGuides";
import { LayoutModeProvider } from "../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { StudyGuidesScreen } from "./StudyGuidesScreen";
import { matchesLayoutQuery, layoutWidths } from "../test/viewport";

vi.mock("../ipc/studyGuides", async () => {
  const actual = await vi.importActual<typeof import("../ipc/studyGuides")>(
    "../ipc/studyGuides",
  );
  return {
    ...actual,
    listStudyGuidePage: vi.fn(),
    deleteDocumentStudyGuide: vi.fn(),
  };
});

vi.mock("./CollectionStudyGuideDialog", () => ({
  CollectionStudyGuideDialog: ({
    collectionTitle,
    onClose,
  }: {
    collectionTitle: string;
    onClose: () => void;
  }) => (
    <div role="dialog" aria-label={`Collection guide ${collectionTitle}`}>
      <button type="button" onClick={onClose}>Close collection guide</button>
    </div>
  ),
}));

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
        targetKind: "document",
        targetId: "a".repeat(32),
        title: "Distributive shock",
        originalFilename: "shock.pdf",
        sourceFormat: "pdf",
        sourceUnitKind: "page",
        sourceUnitCount: 12,
        sourceCount: 1,
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
    vi.mocked(studyGuides.listStudyGuidePage).mockResolvedValue(page());
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
    expect(studyGuides.listStudyGuidePage).toHaveBeenCalledWith({
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
      expect(studyGuides.listStudyGuidePage).toHaveBeenLastCalledWith({
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
      expect(studyGuides.listStudyGuidePage).toHaveBeenLastCalledWith({
        query: "shock",
        sort: "title_asc",
        offset: 0,
        limit: 40,
      }),
    );

    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(screen.getByRole("dialog", { name: "Guide Distributive shock" })).toBeTruthy();
  });

  it("lists and opens collection study guides from the main Study Guides page", async () => {
    vi.mocked(studyGuides.listStudyGuidePage).mockResolvedValue(
      page({
        items: [{
          guideId: "d".repeat(32),
          targetKind: "collection",
          targetId: "c".repeat(32),
          title: "Week 1 physiology",
          originalFilename: null,
          sourceFormat: null,
          sourceUnitKind: null,
          sourceUnitCount: null,
          sourceCount: 2,
          guideRevision: 1,
          sectionCount: 5,
          noteCount: 0,
          publishedAt: 20,
        }],
      }),
    );

    renderScreen();

    expect(await screen.findByText("Week 1 physiology")).toBeTruthy();
    expect(screen.getByText("Document collection · 2 source documents")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Delete Week 1 physiology study guide" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(screen.getByRole("dialog", { name: "Collection guide Week 1 physiology" })).toBeTruthy();
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
    expect(vi.mocked(studyGuides.listStudyGuidePage).mock.calls.length).toBeGreaterThanOrEqual(2);
  });

  it("opens a newly generated guide request without hydrating every list row first", async () => {
    const onConsumed = vi.fn();
    vi.mocked(studyGuides.listStudyGuidePage).mockResolvedValue(
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
    matches: matchesLayoutQuery(query, layoutWidths[layoutMode]),
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}