import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { HomeSummaryProvider } from "../../controllers/HomeSummaryProvider";
import type { DeckSummary } from "../../ipc/decks";
import type { DocumentSummary } from "../../ipc/documents";
import * as homeIpc from "../../ipc/home";
import * as lessonsIpc from "../../ipc/lessons";
import * as progressIpc from "../../ipc/progress";
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
  createOperationId: () => "operation-1",
}));
vi.mock("../../ipc/home", () => ({ getHomeSummary: vi.fn() }));
vi.mock("../../ipc/subjects", () => ({
  getSubjectLibrary: vi.fn(),
  createSubject: vi.fn(),
  getSubjectFilingReviews: vi.fn(),
  dismissSubjectFilingReview: vi.fn(),
}));
vi.mock("../../ipc/practice", () => ({ getPracticeWorkload: vi.fn() }));
vi.mock("../../ipc/lessons", () => ({ listSubjectLessons: vi.fn() }));
vi.mock("../../ipc/progress", () => ({
  getProgressOverview: vi.fn(),
  getProgressActivity: vi.fn(),
  getProgressSubjects: vi.fn(),
}));

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
  const onOpenLesson = vi.fn();
  const onOpenLibrary = vi.fn();
  const onPracticeSubject = vi.fn();
  const onOpenProgress = vi.fn();
  const onOpenDeck = vi.fn();
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
              onOpenLesson={onOpenLesson}
              onOpenLibrary={onOpenLibrary}
              onPracticeSubject={onPracticeSubject}
              onOpenProgress={onOpenProgress}
              onOpenDeck={onOpenDeck}
              refreshDecks={vi.fn(async () => decks)}
              showToast={vi.fn()}
            />
          </HomeSummaryProvider>
        </OverlayProvider>
      </AppearanceProvider>
    </LayoutModeProvider>,
  );
  return { onStartReview, onOpenSubject, onOpenLesson, onOpenLibrary, onPracticeSubject, onOpenProgress, unmount: view.unmount };
}

const DECKS = [makeDeck()];

/** Chooses an order from the Your subjects menu. */
async function sortBy(label: string): Promise<void> {
  fireEvent.click(screen.getByRole("button", { name: "Sort subjects" }));
  fireEvent.click(await screen.findByRole("menuitemcheckbox", { name: label }));
  await waitFor(() => expect(screen.queryByRole("menu")).toBeNull());
}

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
    vi.mocked(lessonsIpc.listSubjectLessons).mockImplementation(async (subjectId) => ({ subjectId, lessons: [] }));
    vi.mocked(progressIpc.getProgressOverview).mockRejectedValue(new Error("not needed"));
    vi.mocked(progressIpc.getProgressActivity).mockRejectedValue(new Error("not needed"));
    vi.mocked(progressIpc.getProgressSubjects).mockResolvedValue([]);
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
    expect(within(tiles[1] as HTMLElement).getByText("Cards only · 312 cards")).toBeTruthy();
    expect(within(tiles[3] as HTMLElement).getByText("2 sources · 0 cards")).toBeTruthy();
    expect(within(tiles[3] as HTMLElement).queryByText(/due|All done/u)).toBeNull();
    expect(within(tiles[4] as HTMLElement).getByText("All done")).toBeTruthy();
    expect(screen.getByTestId("home-new-subject")).toBeTruthy();
  });

  it("keeps the chosen order for the rest of the session", async () => {
    const first = renderHome();
    await screen.findByTestId("home-today");
    await sortBy("Your order");
    expect(tileNames().slice(0, 4)).toEqual([
      "Cardiac physiology",
      "Pharmacology: Autonomic",
      "Spanish for clinicians",
      "Renal physiology",
    ]);
    await sortBy("Name");
    expect(tileNames().slice(0, 4)).toEqual([
      "Cardiac physiology",
      "Pharmacology: Autonomic",
      "Renal physiology",
      "Spanish for clinicians",
    ]);
    first.unmount();
    renderHome();
    await screen.findByTestId("home-today");
    fireEvent.click(screen.getByRole("button", { name: "Sort subjects" }));
    const menu = await screen.findByRole("menu", { name: "Sort subjects" });
    expect(within(menu).getByRole("menuitemcheckbox", { name: "Name" }).getAttribute("aria-checked")).toBe("true");
    fireEvent.click(within(menu).getByRole("menuitemcheckbox", { name: "Most due first" }));
  });

  it("shows how much of each subject is remembered, as a bar on wide screens and in the line on phones", async () => {
    vi.mocked(progressIpc.getProgressSubjects).mockResolvedValue([
      { subjectId: "cardiac", name: "Cardiac physiology", cardCount: 111, solidCount: 0, dueTodayCount: 23, reviewCount: 40, rememberedCount: 36, correctRate: 0.9, retentionReviewCount: 40, retentionRememberedCount: 36, trueRetention: 0.912, retentionDelta: null, studyTimeMs: 0 },
      { subjectId: "renal", name: "Renal physiology", cardCount: 0, solidCount: 0, dueTodayCount: 0, reviewCount: 0, rememberedCount: 0, correctRate: null, retentionReviewCount: 0, retentionRememberedCount: 0, trueRetention: null, retentionDelta: null, studyTimeMs: 0 },
    ]);
    const wide = renderHome();
    await screen.findByTestId("home-today");
    const cardiacTile = () => screen.getAllByTestId("subject-tile")[0] as HTMLElement;
    expect(await within(cardiacTile()).findByText("91% remembered")).toBeTruthy();
    expect(within(screen.getAllByTestId("subject-tile")[3] as HTMLElement).queryByText(/remembered/u)).toBeNull();
    wide.unmount();
    renderHome({ width: 390 });
    await screen.findByTestId("home-today");
    await waitFor(() => expect(within(cardiacTile()).getByText(/4 sources · 111 cards/u).textContent).toBe("4 sources · 111 cards · 91% remembered"));
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
    expect(onOpenSubject).toHaveBeenLastCalledWith("cardiac");
    fireEvent.click(screen.getByRole("button", { name: "Renal physiology" }));
    expect(onOpenSubject).toHaveBeenLastCalledWith("renal");
    fireEvent.click(screen.getByRole("button", { name: "No subject" }));
    expect(onOpenSubject).toHaveBeenLastCalledWith(null);
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

  it("welcomes an empty library with the mascot, no explanation and three ways to start", async () => {
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(makeHomeSummary());
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(makeSubjectLibrary());
    renderHome();
    const welcome = await screen.findByTestId("home-first-run");
    expect(within(welcome).getByRole("heading", { name: "Your library is empty" })).toBeTruthy();
    expect(welcome.querySelector('img[data-artwork-slot="empty.library"]')).not.toBeNull();
    expect(welcome.querySelector("p")).toBeNull();
    expect(within(welcome).queryByRole("button", { name: "Open Library" })).toBeNull();
    expect(screen.queryByTestId("home-today")).toBeNull();
    fireEvent.click(within(welcome).getByRole("button", { name: "Import a document" }));
    expect(jobs.current.uploadDocument).toHaveBeenCalledOnce();
    fireEvent.click(within(welcome).getByRole("button", { name: "Import a deck" }));
    const importDialog = await screen.findByRole("dialog", { name: "Import deck" });
    fireEvent.click(within(importDialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Import deck" })).toBeNull());
    fireEvent.click(within(welcome).getByRole("button", { name: "Create a subject" }));
    expect(await screen.findByRole("dialog", { name: "New subject" })).toBeTruthy();
  });

  it("does not call a library empty when its decks are all archived", async () => {
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(makeHomeSummary());
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(makeSubjectLibrary());
    const { onOpenLibrary } = renderHome({ decks: [makeDeck({ archivedAt: 1_700_000_000_000 })] });
    const welcome = await screen.findByTestId("home-first-run");
    expect(within(welcome).getByRole("heading", { name: "Nothing to study yet" })).toBeTruthy();
    expect(within(welcome).queryByText(/Your library is empty/u)).toBeNull();
    fireEvent.click(within(welcome).getByRole("button", { name: "Open Library" }));
    expect(onOpenLibrary).toHaveBeenCalledOnce();
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

  it("fits a phone: the strip keeps Start review and the subject chooser, and subjects are one list", async () => {
    const { onStartReview } = renderHome({ width: 390 });
    const strip = await screen.findByTestId("home-today");
    expect(within(strip).getByText("About 9 minutes")).toBeTruthy();
    expect(within(strip).getByRole("button", { name: "Choose subjects to review" })).toBeTruthy();
    fireEvent.click(within(strip).getByRole("button", { name: "Start review" }));
    expect(onStartReview).toHaveBeenCalledOnce();
    expect(screen.getByTestId("home-new-subject")).toBeTruthy();
    expect(screen.getAllByTestId("subject-tile")).toHaveLength(5);
  });

  it.each([1440, 390])("at %s px, says how to review cards due in more decks than one session takes", async (width) => {
    const deckIds = Array.from({ length: 101 }, (_, index) => `deck-${index}`);
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(
      makeHomeSummary({ today: { scheduledReviews: 101, estimatedMinutes: 20, deckIds } }),
    );
    renderHome({ width });
    const strip = await screen.findByTestId("home-today");
    expect(
      within(strip).getByText("Cards are due in more than 100 decks. Choose subjects to review them in parts."),
    ).toBeTruthy();
    expect((within(strip).getByRole("button", { name: "Start review" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("offers to pick up a lesson in progress, most recent first", async () => {
    vi.mocked(lessonsIpc.listSubjectLessons).mockImplementation(async (subjectId) => ({
      subjectId,
      lessons:
        subjectId === "cardiac"
          ? [
              lessonSummary("lesson-old", "cardiac", "Valves", { currentStep: 1, updatedAt: 10 }),
              lessonSummary("lesson-new", "cardiac", "The cardiac cycle", { currentStep: 2, updatedAt: 20 }),
              lessonSummary("lesson-done", "cardiac", "Finished one", { currentStep: 7, updatedAt: 30, finishedAt: 30 }),
            ]
          : [],
    }));
    const { onOpenLesson } = renderHome();
    const panel = await screen.findByTestId("home-continue");
    const rows = within(panel).getAllByRole("listitem");
    expect(rows.map((row) => within(row).getByText(/^(Valves|The cardiac cycle)$/u).textContent)).toEqual([
      "The cardiac cycle",
      "Valves",
    ]);
    expect(within(panel).getByText("Lesson · Cardiac physiology · step 3 of 8")).toBeTruthy();
    expect(within(panel).queryByText("Finished one")).toBeNull();
    fireEvent.click(within(panel).getByRole("button", { name: "Resume The cardiac cycle" }));
    expect(onOpenLesson).toHaveBeenCalledWith("cardiac", "lesson-new");
  });

  it("suggests reviewing a slipping subject and a lesson for a subject with sources and none", async () => {
    vi.mocked(progressIpc.getProgressSubjects).mockResolvedValue([
      subjectProgress("cardiac", 0.62, -0.19),
      subjectProgress("pharm", 0.9, -0.02),
    ]);
    const { onStartReview } = renderHome();
    const panel = await screen.findByTestId("home-suggestions");
    expect(within(panel).getByText("Cardiac physiology is slipping")).toBeTruthy();
    expect(within(panel).getByText("62% remembered this week, down from 81%")).toBeTruthy();
    expect(within(panel).queryByText("Pharmacology: Autonomic is slipping")).toBeNull();
    expect(within(panel).getByText("Pharmacology: Autonomic is ready for a lesson")).toBeTruthy();
    fireEvent.click(within(panel).getByRole("button", { name: "Review 23" }));
    expect(onStartReview).toHaveBeenCalledWith(["deck-cp"]);
  });

  it("shows the week at a glance and opens Progress", async () => {
    vi.mocked(progressIpc.getProgressOverview).mockResolvedValue({
      scope: { deckIds: [], tagIds: [] } as never,
      summary: { reviewCount: 212, newCardCount: 0, rememberedCount: 180, correctRate: 0.85, averageConfidence: null, averageResponseTimeMs: null, studyTimeMs: 6_000_000, retentionReviewCount: 150, retentionRememberedCount: 126, trueRetention: 0.84 },
      previousSummary: null,
      studyDayCount: 6,
      timeline: [],
      desiredRetention: 0.9,
    });
    vi.mocked(progressIpc.getProgressActivity).mockResolvedValue({
      startLocalDate: "2026-04-20",
      todayLocalDate: "2026-10-05",
      days: [{ localDate: "2026-10-05", reviewCount: 30, studyTimeMs: 600_000 }],
      currentStreak: { length: 6, startLocalDate: "2026-09-30", endLocalDate: "2026-10-05" },
      longestStreak: null,
      busiestWeekday: null,
    });
    const { onOpenProgress } = renderHome();
    const panel = await screen.findByTestId("home-week");
    expect(within(panel).getByText("84%")).toBeTruthy();
    expect(within(panel).getByText("212")).toBeTruthy();
    expect(within(panel).getByText("1 h 40")).toBeTruthy();
    expect(within(panel).getByText("6")).toBeTruthy();
    fireEvent.click(within(panel).getByRole("button", { name: "Progress" }));
    expect(onOpenProgress).toHaveBeenCalledOnce();
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

function lessonSummary(
  id: string,
  subjectId: string,
  title: string,
  progress: { currentStep: number; updatedAt: number; finishedAt?: number },
): import("../../ipc/lessons").LessonSummary {
  return {
    id,
    subjectId,
    title,
    revision: 1,
    written: { revisionNumber: 1, origin: "generated", stepCount: 8, checkCount: 2, estimatedMinutes: 12, sources: "current", createdAt: 1 },
    progress: {
      revisionNumber: 1,
      currentStep: progress.currentStep,
      furthestStep: progress.currentStep,
      checks: {},
      startedAt: 1,
      updatedAt: progress.updatedAt,
      finishedAt: progress.finishedAt ?? null,
      revision: 1,
    },
  };
}

function subjectProgress(subjectId: string, trueRetention: number, retentionDelta: number): import("../../ipc/progress").SubjectProgress {
  return {
    subjectId,
    name: subjectId,
    cardCount: 100,
    solidCount: 10,
    dueTodayCount: 5,
    reviewCount: 60,
    rememberedCount: 40,
    correctRate: 0.7,
    retentionReviewCount: 40,
    retentionRememberedCount: Math.round(40 * trueRetention),
    trueRetention,
    retentionDelta,
    studyTimeMs: 0,
  };
}
