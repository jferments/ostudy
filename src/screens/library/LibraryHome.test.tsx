import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { HomeSummaryProvider } from "../../controllers/HomeSummaryProvider";
import * as annotationsIpc from "../../ipc/documentAnnotations";
import * as homeIpc from "../../ipc/home";
import * as subjectsIpc from "../../ipc/subjects";
import { makeHomeSubjectSummary, makeHomeSummary, makeSubject, makeSubjectLibrary } from "../../test/factories";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { LibraryHome } from "./LibraryHome";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/home", () => ({ getHomeSummary: vi.fn() }));
vi.mock("../../ipc/subjects", () => ({
  getSubjectLibrary: vi.fn(),
  createSubject: vi.fn(),
  renameSubject: vi.fn(),
  moveSubject: vi.fn(),
  deleteSubject: vi.fn(),
}));
vi.mock("../../ipc/documentAnnotations", () => ({ renderDocumentPageTile: vi.fn() }));
vi.mock("../../controllers/DocumentJobsProvider", () => ({ useDocumentJobs: () => null }));

const physiology = makeSubject({ id: "p".repeat(32), name: "Physiology", sortOrder: 0 });
const spanish = makeSubject({ id: "s".repeat(32), name: "Medical Spanish", sortOrder: 1 });
const library = makeSubjectLibrary({ libraryRevision: 2, subjects: [physiology, spanish] });

function renderHome() {
  const onOpenSubject = vi.fn();
  const onStartReview = vi.fn();
  const onOpenAllCards = vi.fn();
  render(
    <OverlayProvider>
      <HomeSummaryProvider section="overview" activeWorkflow={null} decks={[]}>
        <LibraryHome onOpenSubject={onOpenSubject} onChooseSubject={vi.fn()} onOpenAllCards={onOpenAllCards} onStartReview={onStartReview} showToast={vi.fn()} />
      </HomeSummaryProvider>
    </OverlayProvider>,
  );
  return { onOpenSubject, onStartReview, onOpenAllCards };
}

function tile(name: string): HTMLElement {
  const heading = screen.getByRole("heading", { level: 2, name });
  return heading.closest("article")!;
}

describe("LibraryHome", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(library);
    vi.mocked(annotationsIpc.renderDocumentPageTile).mockResolvedValue(new ArrayBuffer(4));
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(makeHomeSummary({
      subjects: [
        makeHomeSubjectSummary({
          subjectId: physiology.id,
          cardCount: 322,
          deckCount: 1,
          sourceCount: 1,
          guideCount: 1,
          lessonCount: 16,
          podcastCount: 1,
          dueReviews: 19,
          scheduledReviews: 19,
          dueDeckIds: ["d".repeat(32)],
          cover: { documentId: "x".repeat(32), pageNumber: 4, pageWidth: 612, pageHeight: 792, left: 72, top: 100, width: 300, height: 200 },
        }),
        makeHomeSubjectSummary({ subjectId: spanish.id, cardCount: 120, deckCount: 1 }),
      ],
      noSubject: makeHomeSubjectSummary({ cardCount: 5, deckCount: 1 }),
    }));
  });

  it("is titled Library and shows every subject as a tile, No subject last", async () => {
    renderHome();
    expect(await screen.findByRole("heading", { level: 1, name: "Library" })).toBeTruthy();
    const names = within(screen.getByTestId("library-subject-tiles"))
      .getAllByRole("heading", { level: 2 })
      .map((heading) => heading.textContent);
    expect(names).toEqual(["Physiology", "Medical Spanish", "No subject"]);
    expect(screen.queryByRole("navigation", { name: "Subject pages" })).toBeNull();
  });

  it("offers a button for each kind of material a subject holds", async () => {
    const { onOpenSubject } = renderHome();
    await screen.findByRole("heading", { level: 2, name: "Physiology" });
    const full = tile("Physiology");
    expect(within(full).getByText("322 cards · 1 guide · 16 lessons · 1 podcast · 1 document")).toBeTruthy();
    expect(within(full).getAllByRole("button").map((button) => button.getAttribute("aria-label")).filter(Boolean)).toEqual([
      "Physiology actions",
      "Physiology: Flashcards",
      "Physiology: Guide",
      "Physiology: Lessons",
      "Physiology: Podcast",
      "Physiology: Documents",
    ]);
    fireEvent.click(within(full).getByRole("button", { name: "Physiology: Lessons" }));
    expect(onOpenSubject).toHaveBeenCalledWith(physiology.id, "lessons");

    const cardsOnly = tile("Medical Spanish");
    expect(within(cardsOnly).queryByRole("button", { name: "Medical Spanish: Lessons" })).toBeNull();
    fireEvent.click(within(cardsOnly).getByRole("button", { name: "Medical Spanish" }));
    expect(onOpenSubject).toHaveBeenCalledWith(spanish.id, "decks");

    fireEvent.click(within(tile("No subject")).getByRole("button", { name: "No subject: Documents" }));
    expect(onOpenSubject).toHaveBeenCalledWith(null, "documents");
  });

  it("draws a subject's figure on its tile and its due count", async () => {
    renderHome();
    await screen.findByRole("heading", { level: 2, name: "Physiology" });
    // Library keeps a drawn figure while the app runs, so it is drawn once.
    await waitFor(() => expect(tile("Physiology").querySelector("img")).toBeTruthy());
    expect(vi.mocked(annotationsIpc.renderDocumentPageTile).mock.calls.length).toBeLessThanOrEqual(1);
    expect(within(tile("Physiology")).getAllByText("19 due").length).toBeGreaterThan(0);
    // A subject without a figure shows its initials instead.
    expect(within(tile("Medical Spanish")).getByText("MS")).toBeTruthy();
  });

  it("reviews everything due from the header", async () => {
    const { onStartReview } = renderHome();
    fireEvent.click(await screen.findByRole("button", { name: /Review 19 due/u }));
    expect(onStartReview).toHaveBeenCalledWith(["d".repeat(32)]);
  });

  it("opens every card in the Workbench from the header", async () => {
    const { onOpenAllCards } = renderHome();
    fireEvent.click(await screen.findByRole("button", { name: "All cards" }));
    expect(onOpenAllCards).toHaveBeenCalledTimes(1);
  });
});
