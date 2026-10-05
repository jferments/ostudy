import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { HomeSummaryProvider } from "../../controllers/HomeSummaryProvider";
import * as homeIpc from "../../ipc/home";
import * as subjectsIpc from "../../ipc/subjects";
import {
  makeDeck,
  makeHomeSubjectSummary,
  makeHomeSummary,
  makeSubject,
  makeSubjectLibrary,
} from "../../test/factories";
import { ReviewScreen } from "./ReviewScreen";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/home", () => ({ getHomeSummary: vi.fn() }));
vi.mock("../../ipc/subjects", () => ({ getSubjectLibrary: vi.fn() }));
vi.mock("../../controllers/DocumentJobsProvider", () => ({ useDocumentJobs: () => null }));

const DECKS = [makeDeck()];

function renderReview() {
  const onStartReview = vi.fn();
  const onChooseDecks = vi.fn();
  render(
    <HomeSummaryProvider section="review" activeWorkflow={null} decks={DECKS}>
      <ReviewScreen onStartReview={onStartReview} onChooseDecks={onChooseDecks} />
    </HomeSummaryProvider>,
  );
  return { onStartReview, onChooseDecks };
}

describe("ReviewScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(
      makeSubjectLibrary({
        subjects: [
          makeSubject({ id: "cardiac", name: "Cardiac physiology", createdAt: 1 }),
          makeSubject({ id: "bio", name: "Biostatistics", createdAt: 2 }),
          makeSubject({ id: "pharm", name: "Pharmacology", createdAt: 3 }),
        ],
      }),
    );
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(
      makeHomeSummary({
        studyDate: "2026-10-05",
        today: {
          scheduledReviews: 30,
          scheduledNew: 4,
          subjectCount: 3,
          estimatedMinutes: 7,
          deckIds: ["deck-cp", "deck-ph", "deck-loose"],
        },
        subjects: [
          makeHomeSubjectSummary({ subjectId: "cardiac", scheduledReviews: 9, dueDeckIds: ["deck-cp"] }),
          makeHomeSubjectSummary({ subjectId: "bio", cardCount: 64, deckCount: 1 }),
          makeHomeSubjectSummary({
            subjectId: "pharm",
            scheduledReviews: 20,
            scheduledNew: 1,
            dueDeckIds: ["deck-ph"],
          }),
        ],
        noSubject: makeHomeSubjectSummary({
          deckCount: 1,
          scheduledNew: 3,
          dueDeckIds: ["deck-loose"],
        }),
      }),
    );
  });

  it("shows today's total with Start review for everything", async () => {
    const { onStartReview } = renderReview();
    const today = await screen.findByTestId("review-today");
    expect(screen.getByRole("heading", { level: 1, name: "Review" })).toBeTruthy();
    expect(screen.getByText("Monday, October 5")).toBeTruthy();
    expect(within(today).getByRole("heading", { name: "34 cards due today" })).toBeTruthy();
    expect(within(today).getByText("About 7 minutes across 3 subjects")).toBeTruthy();
    fireEvent.click(within(today).getByRole("button", { name: "Start review" }));
    expect(onStartReview).toHaveBeenCalledWith(["deck-cp", "deck-ph", "deck-loose"]);
  });

  it("lists only subjects with cards due, most first, each with its own Start", async () => {
    const { onStartReview } = renderReview();
    const list = await screen.findByTestId("review-subjects");
    const rows = within(list).getAllByTestId("review-subject");
    expect(rows.map((row) => row.textContent)).toEqual([
      "PHPharmacology21 dueStart",
      "CPCardiac physiology9 dueStart",
      "No subject3 dueStart",
    ]);
    fireEvent.click(within(list).getByRole("button", { name: "Start Cardiac physiology" }));
    expect(onStartReview).toHaveBeenCalledWith(["deck-cp"]);
    fireEvent.click(within(list).getByRole("button", { name: "Start No subject" }));
    expect(onStartReview).toHaveBeenLastCalledWith(["deck-loose"]);
    expect(screen.getByText(/Today's limits are shared/u)).toBeTruthy();
  });

  it("leads to custom practice in Library", async () => {
    const { onChooseDecks } = renderReview();
    fireEvent.click(await screen.findByRole("button", { name: "Choose decks" }));
    expect(onChooseDecks).toHaveBeenCalledOnce();
  });

  it("rests calmly when nothing is due", async () => {
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(makeHomeSummary());
    renderReview();
    expect(await screen.findByRole("heading", { name: "Nothing due today" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Start review" })).toBeNull();
    expect(screen.queryByTestId("review-subjects")).toBeNull();
    expect(screen.getByRole("button", { name: "Choose decks" })).toBeTruthy();
  });

  it("says when today's limits are reached", async () => {
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(
      makeHomeSummary({ today: { dueReviews: 12, scheduledReviews: 0 } }),
    );
    renderReview();
    expect(await screen.findByRole("heading", { name: "Done for today" })).toBeTruthy();
    expect(screen.getByText(/Today's review limits are reached/u)).toBeTruthy();
  });

  it("asks for one subject at a time when cards are due in more decks than one session takes", async () => {
    const deckIds = Array.from({ length: 101 }, (_, index) => `deck-${index}`);
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(
      makeHomeSummary({ today: { scheduledReviews: 101, deckIds, subjectCount: 1 } }),
    );
    renderReview();
    const today = await screen.findByTestId("review-today");
    expect((within(today).getByRole("button", { name: "Start review" }) as HTMLButtonElement).disabled).toBe(true);
    expect(within(today).getByText(/more than 100 decks/u)).toBeTruthy();
  });
});
