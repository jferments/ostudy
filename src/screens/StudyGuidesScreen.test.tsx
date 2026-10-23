import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import * as aiProfiles from "../ipc/aiProfiles";
import { listLatestDocumentStudyGuideJobs } from "../ipc/documentArtifactProgress";
import { publishPreparedExport, releasePreparedExport } from "../ipc/externalFiles";
import { getSettings, type AppSettings } from "../ipc/settings";
import * as studyGuides from "../ipc/studyGuides";
import { requiredTestValue as required } from "../test/factories";
import { LayoutModeProvider } from "../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../ui/overlays/OverlayProvider";
import { LibraryFrame } from "./library/LibraryFrame";
import { StudyGuidesScreen } from "./StudyGuidesScreen";
import { TutorContext } from "./tutor/TutorContext";
import type { TutorController } from "./tutor/useTutorController";
import { matchesLayoutQuery, layoutWidths } from "../test/viewport";

vi.mock("../ipc/studyGuides", async () => {
  const actual = await vi.importActual<typeof import("../ipc/studyGuides")>(
    "../ipc/studyGuides",
  );
  return {
    ...actual,
    listStudyGuidePage: vi.fn(),
    deleteDocumentStudyGuide: vi.fn(),
    generateDocumentStudyGuide: vi.fn(),
    cancelDocumentStudyGuideGeneration: vi.fn(),
    prepareDocumentStudyGuidePdf: vi.fn(),
  };
});

vi.mock("../ipc/aiProfiles", () => ({ getAiTaskDefaults: vi.fn(), listAiProviderProfiles: vi.fn() }));
vi.mock("../ipc/documentArtifactProgress", () => ({ listLatestDocumentStudyGuideJobs: vi.fn() }));
vi.mock("../ipc/settings", () => ({ getSettings: vi.fn() }));
vi.mock("../ipc/externalFiles", async () => {
  const actual = await vi.importActual<typeof import("../ipc/externalFiles")>("../ipc/externalFiles");
  return { ...actual, publishPreparedExport: vi.fn(), releasePreparedExport: vi.fn() };
});

vi.mock("../ipc/studyGuideReading", () => ({}));

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
    source,
    onClose,
  }: {
    documentTitle: string;
    source: { filename: string | null; format: string | null; unitKind: string | null; unitCount: number | null } | null;
    onClose: () => void;
  }) => (
    <div role="dialog" aria-label={`Guide ${documentTitle}`}>
      <p>{source ? `Source ${source.filename} ${source.format} ${source.unitCount} ${source.unitKind}` : "No source details"}</p>
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
        readSectionCount: 0,
        lastOpenedAt: null,
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
  tutor: TutorController | null = null,
) {
  return render(
    <LayoutModeProvider>
      <OverlayProvider>
        <TutorContext.Provider value={tutor}>
          <StudyGuidesScreen
            showToast={vi.fn()}
            onPracticeCards={vi.fn()}
            {...overrides}
          />
        </TutorContext.Provider>
      </OverlayProvider>
    </LayoutModeProvider>,
  );
}

describe("StudyGuidesScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setLayoutMode("wide");
    vi.mocked(studyGuides.listStudyGuidePage).mockResolvedValue(page());
    vi.mocked(listLatestDocumentStudyGuideJobs).mockResolvedValue([]);
    vi.mocked(aiProfiles.getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: "openrouter" });
    vi.mocked(aiProfiles.listAiProviderProfiles).mockResolvedValue([
      { id: "openrouter" } as aiProfiles.AiProviderProfileSummary,
    ]);
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

  it("shows how far each document guide is read", async () => {
    const base = required(page().items[0]);
    vi.mocked(studyGuides.listStudyGuidePage).mockResolvedValue(page({
      items: [
        base,
        { ...base, guideId: "c".repeat(32), targetId: "d".repeat(32), title: "Renal physiology", readSectionCount: 2, sectionCount: 4, lastOpenedAt: 5 },
        { ...base, guideId: "e".repeat(32), targetId: "f".repeat(32), title: "Cardiac cycle", readSectionCount: 3, sectionCount: 3, lastOpenedAt: 6 },
      ],
      totalCount: 3,
    }));
    renderScreen();

    const cards = await screen.findAllByTestId("guide-card");
    expect(cards.map((card) => within(card).getByRole("progressbar").getAttribute("aria-valuetext"))).toEqual([
      "Not started", "2 of 4 sections read", "Finished",
    ]);
  });

  it("lists guides without a separate Continue reading band", async () => {
    renderScreen();

    expect(await screen.findAllByTestId("guide-card")).not.toHaveLength(0);
    expect(screen.queryByText("Continue reading")).toBeNull();
    expect(screen.queryByRole("button", { name: "Resume" })).toBeNull();
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
      subject: { kind: "all" },
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
        subject: { kind: "all" },
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
        subject: { kind: "all" },
      }),
    );

    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    const reader = screen.getByRole("dialog", { name: "Guide Distributive shock" });
    // The reader names the source file from the row instead of loading it again.
    expect(reader.textContent).toContain("Source shock.pdf pdf 12 page");
  });

  it("gives an open guide Library's whole page, then brings the tabs back", async () => {
    render(
      <LayoutModeProvider>
        <OverlayProvider>
          <LibraryFrame view="study-guides" onSelectView={vi.fn()} subject={{ kind: "all" }} onChooseSubject={vi.fn()} onStartReview={vi.fn()} onPracticeSubject={vi.fn()} showToast={vi.fn()}>
            <StudyGuidesScreen showToast={vi.fn()} onPracticeCards={vi.fn()} />
          </LibraryFrame>
        </OverlayProvider>
      </LayoutModeProvider>,
    );
    expect(await screen.findByText("Distributive shock")).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Library pages" })).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(screen.getByRole("dialog", { name: "Guide Distributive shock" })).toBeTruthy();
    // The reader's sticky top bar starts at the edges, right under the app header.
    expect(screen.queryByRole("navigation", { name: "Library pages" })).toBeNull();
    expect(screen.getByTestId("library").className).toContain("tw:p-0");

    fireEvent.click(screen.getByRole("button", { name: "Close guide" }));
    expect(await screen.findByText("Distributive shock")).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Library pages" })).toBeTruthy();
    expect(screen.getByRole("navigation", { name: "Library pages" }).closest("div.tw\\:px-5")).toBeTruthy();
  });

  it("lists and opens subject study guides from the main Study Guides page", async () => {
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
          readSectionCount: 0,
          lastOpenedAt: null,
          publishedAt: 20,
        }],
      }),
    );

    renderScreen();

    expect(await screen.findByText("Week 1 physiology")).toBeTruthy();
    expect(screen.getByText("Collection · 2 documents")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "More actions for Week 1 physiology" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(screen.getByRole("dialog", { name: "Collection guide Week 1 physiology" })).toBeTruthy();
  });

  it("deletes through explicit preservation copy and refreshes the projection", async () => {
    const showToast = vi.fn();
    renderScreen({ showToast });
    await screen.findByText("Distributive shock");

    const menu = await openGuideActions("Distributive shock");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Delete study guide" }));
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

  it("asks Tutor about a guide from its card", async () => {
    const openContext = vi.fn();
    renderScreen({}, { busy: false, pending: null, openContext } as unknown as TutorController);
    await screen.findByText("Distributive shock");

    const menu = await openGuideActions("Distributive shock");
    expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual([
      "Ask Tutor", "Export PDF", "Remake the study guide", "Open the source", "Delete study guide",
    ]);
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Ask Tutor" }));
    expect(openContext).toHaveBeenCalledWith({
      title: "Distributive shock",
      scope: { kind: "guide", id: "a".repeat(32) },
      focus: { kind: "guide", documentId: "a".repeat(32), guideId: "b".repeat(32), topicId: null, contentHash: null },
      mode: "explain",
    });
  });

  it("exports a guide's PDF from its card with the Settings choices", async () => {
    const showToast = vi.fn();
    vi.mocked(getSettings).mockResolvedValue({ guidePdfTheme: "plum", guidePdfIncludeNotes: true } as AppSettings);
    vi.mocked(studyGuides.prepareDocumentStudyGuidePdf).mockResolvedValue({
      preparedExport: { id: "e".repeat(32) },
      guideRevision: 3, sectionCount: 6, annotationCount: 2, mathSourceFallback: false,
    } as studyGuides.PreparedStudyGuidePdf);
    vi.mocked(publishPreparedExport).mockResolvedValue({} as Awaited<ReturnType<typeof publishPreparedExport>>);
    vi.mocked(releasePreparedExport).mockResolvedValue(undefined);
    renderScreen({ showToast });
    await screen.findByText("Distributive shock");

    const menu = await openGuideActions("Distributive shock");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Export PDF" }));

    await waitFor(() => expect(showToast).toHaveBeenCalledWith("success", "Exported study guide with 6 sections and 2 notes."));
    expect(studyGuides.prepareDocumentStudyGuidePdf).toHaveBeenCalledWith("a".repeat(32), true, "plum");
    expect(releasePreparedExport).toHaveBeenCalledWith("e".repeat(32));
  });

  it("remakes a guide from its card, shows its bar, and refreshes once it is saved", async () => {
    const showToast = vi.fn();
    let finish: (guide: studyGuides.DocumentStudyGuide) => void = () => undefined;
    vi.mocked(studyGuides.generateDocumentStudyGuide).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    renderScreen({ showToast });
    await screen.findByText("Distributive shock");

    let menu = await openGuideActions("Distributive shock");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Remake the study guide" }));

    expect(studyGuides.generateDocumentStudyGuide).toHaveBeenCalledWith("a".repeat(32));
    expect(await screen.findByText("Remaking · Preparing")).toBeTruthy();
    menu = await openGuideActions("Distributive shock");
    expect(within(menu).getByRole("menuitem", { name: "Stop remaking" })).toBeTruthy();
    expect(within(menu).getByRole("menuitem", { name: "Delete study guide" }).getAttribute("aria-disabled")).toBe("true");
    const listsBefore = vi.mocked(studyGuides.listStudyGuidePage).mock.calls.length;

    await act(async () => { finish({ revision: 4 } as studyGuides.DocumentStudyGuide); });

    expect(showToast).toHaveBeenCalledWith("success", "Study guide revision 4 made.");
    await waitFor(() => expect(vi.mocked(studyGuides.listStudyGuidePage).mock.calls.length).toBeGreaterThan(listsBefore));
    expect(screen.queryByText("Remaking · Preparing")).toBeNull();
  });

  it("opens AI setup from Remake while no model is chosen for guides", async () => {
    const onSetUpAi = vi.fn();
    vi.mocked(aiProfiles.getAiTaskDefaults).mockResolvedValue({ flashcardGenerationProfileId: null });
    renderScreen({ onSetUpAi });
    await screen.findByText("Distributive shock");
    await waitFor(() => expect(aiProfiles.listAiProviderProfiles).toHaveBeenCalled());
    await act(async () => { await Promise.resolve(); });

    const menu = await openGuideActions("Distributive shock");
    fireEvent.click(within(menu).getByRole("menuitem", { name: "Remake the study guide" }));

    await waitFor(() => expect(onSetUpAi).toHaveBeenCalledTimes(1));
    expect(studyGuides.generateDocumentStudyGuide).not.toHaveBeenCalled();
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

    expect((await screen.findByRole("dialog", { name: "Guide Cardiac output" })).textContent).toContain("No source details");
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

async function openGuideActions(title: string): Promise<HTMLElement> {
  const label = `More actions for ${title}`;
  const trigger = screen.getByRole("button", { name: label });
  // Base UI wires a menu trigger in a passive effect, so a click before it runs
  // is dropped; clicking until the menu reports open never toggles it shut.
  for (let attempt = 0; attempt < 3 && trigger.getAttribute("aria-expanded") !== "true"; attempt += 1) {
    await act(async () => {
      fireEvent.click(trigger);
    });
  }
  await waitFor(() => {
    expect(trigger.getAttribute("aria-expanded")).toBe("true");
  });
  return screen.findByRole("menu", { name: label });
}
