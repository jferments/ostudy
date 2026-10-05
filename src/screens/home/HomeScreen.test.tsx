import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { HomeSummaryProvider } from "../../controllers/HomeSummaryProvider";
import type { DeckSummary } from "../../ipc/decks";
import type { DocumentSummary } from "../../ipc/documents";
import * as homeIpc from "../../ipc/home";
import * as practiceIpc from "../../ipc/practice";
import * as subjectsIpc from "../../ipc/subjects";
import {
  makeDeck,
  makeHomeSubjectSummary,
  makeHomeSummary,
  makeSubject,
  makeSubjectLibrary,
} from "../../test/factories";
import { matchesLayoutQuery } from "../../test/viewport";
import { AppearanceProvider } from "../../ui/appearance/AppearanceProvider";
import { LayoutModeProvider } from "../../ui/layout/LayoutModeProvider";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { HomeScreen } from "./HomeScreen";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/home", () => ({ getHomeSummary: vi.fn() }));
vi.mock("../../ipc/subjects", () => ({
  getSubjectLibrary: vi.fn(),
  createSubject: vi.fn(),
  getSubjectFilingReviews: vi.fn(),
  dismissSubjectFilingReview: vi.fn(),
}));
vi.mock("../../ipc/practice", () => ({ getPracticeWorkload: vi.fn() }));

const jobs = vi.hoisted(() => ({
  current: {
    documents: [] as Partial<import("../../ipc/documents").DocumentSummary>[],
    cancelingDocumentIds: new Set<string>(),
    guideGeneratingDocumentIds: new Set<string>(),
    generationProgress: {},
    isUploading: false,
    uploadDocument: vi.fn(),
  },
}));
vi.mock("../../controllers/DocumentJobsProvider", () => ({
  useDocumentJobs: () => jobs.current,
}));

const originalMatchMedia = window.matchMedia;

function setViewport(width: number): void {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: matchesLayoutQuery(query, width),
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  }));
}

const cardiac = makeSubject({ id: "cardiac", name: "Cardiac physiology", createdAt: 1 });
const pharmacology = makeSubject({ id: "pharm", name: "Pharmacology: Autonomic", createdAt: 2 });
const spanish = makeSubject({ id: "spanish", name: "Spanish for clinicians", createdAt: 3 });
const renal = makeSubject({ id: "renal", name: "Renal physiology", createdAt: 4 });

function populatedSummary() {
  return makeHomeSummary({
    studyDate: "2026-10-05",
    today: {
      scheduledReviews: 40,
      scheduledNew: 7,
      subjectCount: 3,
      estimatedMinutes: 9,
      deckIds: ["deck-cp", "deck-pa", "deck-sc"],
    },
    subjects: [
      makeHomeSubjectSummary({
        subjectId: "cardiac",
        scheduledReviews: 20,
        scheduledNew: 3,
        cardCount: 111,
        deckCount: 2,
        sourceCount: 4,
        hasOverviewGuide: true,
        dueDeckIds: ["deck-cp"],
      }),
      makeHomeSubjectSummary({
        subjectId: "pharm",
        scheduledReviews: 11,
        cardCount: 214,
        deckCount: 1,
        sourceCount: 3,
        dueDeckIds: ["deck-pa"],
      }),
      makeHomeSubjectSummary({
        subjectId: "spanish",
        scheduledReviews: 9,
        scheduledNew: 4,
        cardCount: 312,
        deckCount: 1,
        dueDeckIds: ["deck-sc"],
      }),
      makeHomeSubjectSummary({ subjectId: "renal", sourceCount: 2 }),
    ],
    noSubject: makeHomeSubjectSummary({ deckCount: 1, cardCount: 12 }),
  });
}

function renderHome({
  width = 1440,
  decorativeArtwork = true,
  decks = DECKS,
}: { width?: number; decorativeArtwork?: boolean; decks?: DeckSummary[] } = {}) {
  setViewport(width);
  const onStartReview = vi.fn();
  const onOpenSubject = vi.fn();
  const onOpenLibrary = vi.fn();
  const view = render(
    <LayoutModeProvider>
      <AppearanceProvider
        settings={{ theme: "light", uiFont: "system", cardFont: "system", uiScale: 100, cardBaseSize: 18, decorativeArtwork }}
      >
        <OverlayProvider>
          <HomeSummaryProvider section="home" activeWorkflow={null} decks={decks}>
            <HomeScreen
              onStartReview={onStartReview}
              onOpenSubject={onOpenSubject}
              onOpenLibrary={onOpenLibrary}
            />
          </HomeSummaryProvider>
        </OverlayProvider>
      </AppearanceProvider>
    </LayoutModeProvider>,
  );
  return { onStartReview, onOpenSubject, onOpenLibrary, unmount: view.unmount };
}

const DECKS = [makeDeck()];

function tileNames(): string[] {
  return screen
    .getAllByTestId("subject-tile")
    .map((tile) => within(tile).getByRole("heading").textContent ?? "");
}

describe("HomeScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    jobs.current = { ...jobs.current, documents: [], isUploading: false, uploadDocument: vi.fn() };
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(populatedSummary());
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(
      makeSubjectLibrary({ subjects: [cardiac, pharmacology, spanish, renal] }),
    );
  });

  afterEach(() => {
    window.matchMedia = originalMatchMedia;
  });

  it("shows today's strip and the subjects, most due first, with No subject last", async () => {
    renderHome();
    const strip = await screen.findByTestId("home-today");
    expect(within(strip).getByRole("heading", { name: "47 cards due today" })).toBeTruthy();
    expect(within(strip).getByText("About 9 minutes across 3 subjects · Monday, October 5")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Home" })).toBeTruthy();
    expect(screen.getByText("4 subjects · 9 sources · 649 cards")).toBeTruthy();

    expect(tileNames()).toEqual([
      "Cardiac physiology",
      "Spanish for clinicians",
      "Pharmacology: Autonomic",
      "Renal physiology",
      "No subject",
    ]);
    const tiles = screen.getAllByTestId("subject-tile");
    expect(within(tiles[0] as HTMLElement).getByText("23 due")).toBeTruthy();
    expect(within(tiles[0] as HTMLElement).getByText("4 sources · 111 cards")).toBeTruthy();
    expect(within(tiles[0] as HTMLElement).getByText("Overview guide ready")).toBeTruthy();
    expect(within(tiles[1] as HTMLElement).getByText("Cards only · 312 cards")).toBeTruthy();
    expect(within(tiles[3] as HTMLElement).getByText("2 sources · 0 cards")).toBeTruthy();
    expect(within(tiles[3] as HTMLElement).queryByText(/due|All done/u)).toBeNull();
    expect(within(tiles[4] as HTMLElement).getByText("All done")).toBeTruthy();
    expect(screen.getByTestId("home-new-subject")).toBeTruthy();
  });

  it("keeps the chosen order for the rest of the session", async () => {
    const first = renderHome();
    await screen.findByTestId("home-today");
    fireEvent.change(screen.getByRole("combobox", { name: "Sort subjects" }), {
      target: { value: "your-order" },
    });
    expect(tileNames().slice(0, 4)).toEqual([
      "Cardiac physiology",
      "Pharmacology: Autonomic",
      "Spanish for clinicians",
      "Renal physiology",
    ]);
    fireEvent.change(screen.getByRole("combobox", { name: "Sort subjects" }), {
      target: { value: "name" },
    });
    expect(tileNames().slice(0, 4)).toEqual([
      "Cardiac physiology",
      "Pharmacology: Autonomic",
      "Renal physiology",
      "Spanish for clinicians",
    ]);
    first.unmount();
    renderHome();
    await screen.findByTestId("home-today");
    expect((screen.getByRole("combobox", { name: "Sort subjects" }) as HTMLSelectElement).value).toBe("name");
    fireEvent.change(screen.getByRole("combobox", { name: "Sort subjects" }), {
      target: { value: "most-due" },
    });
  });

  it("starts exactly what the strip counts", async () => {
    const { onStartReview } = renderHome();
    fireEvent.click(await screen.findByRole("button", { name: "Start review" }));
    expect(onStartReview).toHaveBeenCalledWith(["deck-cp", "deck-pa", "deck-sc"]);
  });

  it("narrows today's review to chosen subjects with a native count", async () => {
    vi.mocked(practiceIpc.getPracticeWorkload).mockImplementation(async (deckIds) => ({
      newLimit: 20,
      reviewLimit: 200,
      newCompleted: 0,
      reviewsCompleted: 0,
      newRemaining: 20,
      reviewsRemaining: 200,
      studyableDueReviews: 0,
      studyableDueNew: 0,
      scheduledReviews: deckIds.length * 10,
      scheduledNew: 0,
      omittedDueReviews: 0,
      omittedDueNew: 0,
      needsMorePractice: 0,
      extraAvailable: 0,
    }));
    const { onStartReview } = renderHome();
    fireEvent.click(await screen.findByRole("button", { name: "Choose subjects to review" }));
    const sheet = await screen.findByRole("dialog", { name: "Choose subjects to review" });
    const boxes = within(sheet).getAllByRole("checkbox");
    expect(boxes).toHaveLength(3);
    expect(await within(sheet).findByText("30 cards to review")).toBeTruthy();
    expect(practiceIpc.getPracticeWorkload).toHaveBeenLastCalledWith(["deck-cp", "deck-pa", "deck-sc"]);

    fireEvent.click(within(sheet).getByRole("checkbox", { name: /Spanish for clinicians/u }));
    expect(await within(sheet).findByText("20 cards to review")).toBeTruthy();
    expect(practiceIpc.getPracticeWorkload).toHaveBeenLastCalledWith(["deck-cp", "deck-pa"]);

    fireEvent.click(within(sheet).getByRole("button", { name: "Select all" }));
    expect(await within(sheet).findByText("30 cards to review")).toBeTruthy();
    fireEvent.click(within(sheet).getByRole("button", { name: "Clear all" }));
    expect(within(sheet).getByText("Choose at least one subject.")).toBeTruthy();
    expect((within(sheet).getByRole("button", { name: "Start review" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(within(sheet).getByRole("checkbox", { name: /Cardiac physiology/u }));
    await within(sheet).findByText("10 cards to review");
    fireEvent.click(within(sheet).getByRole("button", { name: "Start review" }));
    expect(onStartReview).toHaveBeenCalledWith(["deck-cp"]);
  });

  it("opens a subject in the view that shows it", async () => {
    const { onOpenSubject } = renderHome();
    await screen.findByTestId("home-today");
    fireEvent.click(screen.getByRole("button", { name: "Cardiac physiology" }));
    expect(onOpenSubject).toHaveBeenLastCalledWith("cardiac", "decks");
    fireEvent.click(screen.getByRole("button", { name: "Renal physiology" }));
    expect(onOpenSubject).toHaveBeenLastCalledWith("renal", "documents");
    fireEvent.click(screen.getByRole("button", { name: "No subject" }));
    expect(onOpenSubject).toHaveBeenLastCalledWith(null, "decks");
  });

  it("describes each tile's button with its due count and contents", async () => {
    renderHome();
    await screen.findByTestId("home-today");
    const button = screen.getByRole("button", { name: "Cardiac physiology" });
    const description = (button.getAttribute("aria-describedby") ?? "")
      .split(" ")
      .map((id) => document.getElementById(id)?.textContent)
      .join(" | ");
    expect(description).toBe("23 due | 4 sources · 111 cards");
  });

  it("shows the running step of a subject's document", async () => {
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(
      makeSubjectLibrary({
        subjects: [cardiac, pharmacology, spanish, renal],
        documents: [{ documentId: "doc-renal", subjectId: "renal" }],
      }),
    );
    jobs.current = {
      ...jobs.current,
      documents: [
        {
          id: "doc-renal",
          title: "Week 4 notes",
          lifecycleState: "processing",
          generatedDeckId: null,
          sourceUnitCount: 0,
          sourceFormat: "pdf",
        } as Partial<DocumentSummary>,
      ],
    };
    renderHome();
    await screen.findByTestId("home-today");
    const tile = screen.getAllByTestId("subject-tile").find((element) => element.dataset.subjectId === "renal");
    expect(within(tile as HTMLElement).getByText("Preparing")).toBeTruthy();
    expect(within(tile as HTMLElement).getByText("Extracting document content: Week 4 notes")).toBeTruthy();
  });

  it("names a new subject and shows it at once", async () => {
    const created = makeSubject({ id: "bio", name: "Biostatistics", createdAt: 5 });
    vi.mocked(subjectsIpc.createSubject).mockResolvedValue(
      makeSubjectLibrary({ libraryRevision: 2, subjects: [cardiac, pharmacology, spanish, renal, created] }),
    );
    renderHome();
    await screen.findByTestId("home-today");
    fireEvent.click(screen.getByTestId("home-new-subject"));
    const sheet = await screen.findByRole("dialog", { name: "New subject" });
    const create = within(sheet).getByRole("button", { name: "Create subject" }) as HTMLButtonElement;
    expect(create.disabled).toBe(true);
    fireEvent.change(within(sheet).getByRole("textbox", { name: "Name" }), {
      target: { value: "  Biostatistics " },
    });
    fireEvent.click(create);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "New subject" })).toBeNull());
    expect(subjectsIpc.createSubject).toHaveBeenCalledWith({
      name: "Biostatistics",
      expectedLibraryRevision: 1,
    });
    expect(screen.getByRole("button", { name: "Biostatistics" })).toBeTruthy();
    expect(screen.getByText("Biostatistics created.")).toBeTruthy();
  });

  it("keeps the dialog open with the reason when the name is refused", async () => {
    vi.mocked(subjectsIpc.createSubject).mockRejectedValue(new Error("A subject already has that name."));
    renderHome();
    await screen.findByTestId("home-today");
    fireEvent.click(screen.getByTestId("home-new-subject"));
    const sheet = await screen.findByRole("dialog", { name: "New subject" });
    fireEvent.change(within(sheet).getByRole("textbox", { name: "Name" }), {
      target: { value: "Cardiac physiology" },
    });
    fireEvent.click(within(sheet).getByRole("button", { name: "Create subject" }));
    expect(await within(sheet).findByText("A subject already has that name.")).toBeTruthy();
  });

  it("links quietly to Check these and lets an entry be dismissed", async () => {
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(
      makeSubjectLibrary({ subjects: [cardiac], checkTheseCount: 2 }),
    );
    vi.mocked(subjectsIpc.getSubjectFilingReviews).mockResolvedValue([
      { entityKind: "deck", entityId: "d1", reason: "moved_to_document_subject", detail: {}, label: "Heart sounds" },
      {
        entityKind: "document",
        entityId: "doc1",
        reason: "in_several_collections",
        detail: { droppedRangeCount: 2 },
        label: "Lecture 3",
      },
    ]);
    vi.mocked(subjectsIpc.dismissSubjectFilingReview).mockResolvedValue([
      {
        entityKind: "document",
        entityId: "doc1",
        reason: "in_several_collections",
        detail: { droppedRangeCount: 2 },
        label: "Lecture 3",
      },
    ]);
    renderHome();
    const notice = await screen.findByTestId("home-check-these");
    expect(notice.textContent).toContain("2 items had more than one sensible place");
    fireEvent.click(within(notice).getByRole("button", { name: "Check these" }));
    const sheet = await screen.findByRole("dialog", { name: "Check these" });
    expect(await within(sheet).findByText("Heart sounds")).toBeTruthy();
    expect(within(sheet).getByRole("heading", { name: "Decks" })).toBeTruthy();
    expect(
      within(sheet).getByText(
        "This document was in several collections. It stayed in the first one it joined. 2 page ranges set in the others were dropped.",
      ),
    ).toBeTruthy();
    fireEvent.click(within(sheet).getByRole("button", { name: "Dismiss Heart sounds" }));
    await waitFor(() => expect(within(sheet).queryByText("Heart sounds")).toBeNull());
    expect(subjectsIpc.dismissSubjectFilingReview).toHaveBeenCalledWith({ entityKind: "deck", entityId: "d1" });
  });

  it("welcomes an empty library with the mascot and three ways to start", async () => {
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(makeHomeSummary());
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(makeSubjectLibrary());
    const { onOpenLibrary } = renderHome();
    const welcome = await screen.findByTestId("home-first-run");
    expect(within(welcome).getByRole("heading", { name: "Your library is empty" })).toBeTruthy();
    expect(welcome.querySelector('img[data-artwork-slot="empty.library"]')).not.toBeNull();
    expect(screen.queryByTestId("home-today")).toBeNull();
    fireEvent.click(within(welcome).getByRole("button", { name: "Import a document" }));
    expect(jobs.current.uploadDocument).toHaveBeenCalledOnce();
    fireEvent.click(within(welcome).getByRole("button", { name: "Open Library" }));
    expect(onOpenLibrary).toHaveBeenCalledOnce();
    fireEvent.click(within(welcome).getByRole("button", { name: "Create a subject" }));
    expect(await screen.findByRole("dialog", { name: "New subject" })).toBeTruthy();
  });

  it("does not call a library empty when its decks are all archived", async () => {
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(makeHomeSummary());
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(makeSubjectLibrary());
    renderHome({ decks: [makeDeck({ archivedAt: 1_700_000_000_000 })] });
    const welcome = await screen.findByTestId("home-first-run");
    expect(within(welcome).getByRole("heading", { name: "Nothing to study yet" })).toBeTruthy();
    expect(within(welcome).queryByText(/Your library is empty/u)).toBeNull();
    expect(within(welcome).getByText(/Your archived decks are in Library\./u)).toBeTruthy();
    expect(within(welcome).getByRole("button", { name: "Open Library" })).toBeTruthy();
  });

  it("shows a plain icon instead of the mascot when mascot art is off", async () => {
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(makeHomeSummary());
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(makeSubjectLibrary());
    renderHome({ decorativeArtwork: false });
    const welcome = await screen.findByTestId("home-first-run");
    expect(welcome.querySelector("img")).toBeNull();
    expect(welcome.querySelector("svg")).not.toBeNull();
  });

  it("shows unfiled material as No subject instead of the welcome", async () => {
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(
      makeHomeSummary({ noSubject: makeHomeSubjectSummary({ sourceCount: 1 }) }),
    );
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(makeSubjectLibrary());
    renderHome();
    await screen.findByTestId("home-today");
    expect(screen.queryByTestId("home-first-run")).toBeNull();
    expect(tileNames()).toEqual(["No subject"]);
    expect(screen.getByRole("heading", { name: "Nothing due today" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Start review" })).toBeNull();
  });

  it("fits a phone: a slim strip with Start, a New subject button and two columns", async () => {
    const { onStartReview } = renderHome({ width: 390 });
    const strip = await screen.findByTestId("home-today");
    expect(within(strip).getByText("About 9 minutes")).toBeTruthy();
    expect(within(strip).queryByRole("button", { name: "Choose subjects to review" })).toBeNull();
    fireEvent.click(within(strip).getByRole("button", { name: "Start" }));
    expect(onStartReview).toHaveBeenCalledOnce();
    expect(screen.queryByTestId("home-new-subject")).toBeNull();
    expect(screen.getByRole("button", { name: "New subject" })).toBeTruthy();
    expect(screen.getAllByTestId("subject-tile")[0]?.closest("ul")?.className).toContain("tw:grid-cols-2");
  });

  it("says when today's limits are reached instead of showing zero", async () => {
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(
      makeHomeSummary({
        today: { dueReviews: 30, scheduledReviews: 0 },
        subjects: [makeHomeSubjectSummary({ subjectId: "cardiac", dueReviews: 30, cardCount: 30, deckCount: 1 })],
      }),
    );
    renderHome();
    expect(await screen.findByRole("heading", { name: "Done for today" })).toBeTruthy();
    expect(screen.getByText("Today's review limits are reached · Monday, October 5")).toBeTruthy();
  });

  it("offers a retry when Home cannot load", async () => {
    vi.mocked(homeIpc.getHomeSummary)
      .mockRejectedValueOnce(new Error("Library locked"))
      .mockResolvedValue(populatedSummary());
    renderHome();
    expect(await screen.findByText(/Home could not load: Library locked/u)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByTestId("home-today")).toBeTruthy();
  });
});
