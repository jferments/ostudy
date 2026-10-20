import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { HomeSummaryProvider } from "../../controllers/HomeSummaryProvider";
import * as homeIpc from "../../ipc/home";
import type { LessonSummary } from "../../ipc/lessons";
import * as lessonsIpc from "../../ipc/lessons";
import type { ProgressOverview, ProgressSummary } from "../../ipc/progress";
import * as progressIpc from "../../ipc/progress";
import * as studyGuidesIpc from "../../ipc/studyGuides";
import * as subjectsIpc from "../../ipc/subjects";
import type { LibrarySubjectScope } from "../../navigation/appDestinations";
import { makeDeck, makeHomeSubjectSummary, makeHomeSummary, makeSubject, makeSubjectLibrary } from "../../test/factories";
import { LibraryOverview } from "./LibraryOverview";

vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));
vi.mock("../../ipc/home", () => ({ getHomeSummary: vi.fn() }));
vi.mock("../../ipc/subjects", () => ({ getSubjectLibrary: vi.fn() }));
vi.mock("../../ipc/lessons", () => ({ listSubjectLessons: vi.fn() }));
vi.mock("../../ipc/studyGuides", () => ({ listStudyGuidePage: vi.fn() }));
vi.mock("../../ipc/progress", () => ({
  getProgressOverview: vi.fn(),
  getProgressForecast: vi.fn(),
  getProgressSubjects: vi.fn(),
}));
const jobs = vi.hoisted(() => ({ current: { documents: [] as Partial<import("../../ipc/documents").DocumentSummary>[] } }));
vi.mock("../../controllers/DocumentJobsProvider", () => ({ useDocumentJobs: () => jobs.current }));

const physiology = makeSubject({ id: "p".repeat(32), name: "Physiology", sortOrder: 0 });
const cardiac = makeDeck({ id: "c".repeat(32), name: "Cardiovascular physiology", generatedCardCount: 11 });
const renal = makeDeck({ id: "r".repeat(32), name: "Renal physiology", generatedCardCount: 5 });
const loose = makeDeck({ id: "l".repeat(32), name: "Loose deck" });
const lectureId = "d".repeat(32);

function summary(trueRetention: number | null, reviewCount = 10): ProgressSummary {
  return {
    reviewCount, newCardCount: 0, rememberedCount: reviewCount, correctRate: null, averageConfidence: null,
    averageResponseTimeMs: null, studyTimeMs: 0, retentionReviewCount: reviewCount, retentionRememberedCount: reviewCount, trueRetention,
  };
}

function overview(now: number | null, before: number | null, timeline: number[] = []): ProgressOverview {
  return {
    scope: {} as ProgressOverview["scope"],
    summary: summary(now),
    previousSummary: before === null ? null : summary(before),
    studyDayCount: timeline.filter((count) => count > 0).length,
    timeline: timeline.map((reviewCount, index) => ({
      ...summary(null, reviewCount),
      startLocalDate: `2026-10-${String(14 + index).padStart(2, "0")}`,
      endLocalDate: `2026-10-${String(14 + index).padStart(2, "0")}`,
    })),
    desiredRetention: 0.9,
  };
}

function lesson(): LessonSummary {
  return {
    id: "1".repeat(32),
    subjectId: physiology.id,
    title: "The cardiac cycle",
    revision: 1,
    written: { revisionNumber: 1, origin: "generated", stepCount: 8, checkCount: 2, estimatedMinutes: 12, sources: "current", createdAt: 1 },
    progress: { revisionNumber: 1, currentStep: 2, furthestStep: 2, checks: {}, startedAt: 1, updatedAt: 5, finishedAt: null, revision: 1 },
  } as LessonSummary;
}

function renderOverview(subject: LibrarySubjectScope = { kind: "subject", subjectId: physiology.id }) {
  const props = {
    onSelectView: vi.fn(),
    onOpenLesson: vi.fn(),
    onOpenDeck: vi.fn(),
    onPracticeDeck: vi.fn(),
    onOpenGuide: vi.fn(),
    onOpenSubject: vi.fn(),
    onOpenProgress: vi.fn(),
  };
  render(
    <HomeSummaryProvider section="overview" activeWorkflow={null} decks={[cardiac, renal, loose]}>
      <LibraryOverview subject={subject} decks={[cardiac, renal, loose]} {...props} />
    </HomeSummaryProvider>,
  );
  return props;
}

describe("LibraryOverview", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(homeIpc.getHomeSummary).mockResolvedValue(makeHomeSummary({
      subjects: [makeHomeSubjectSummary({ subjectId: physiology.id, scheduledReviews: 15, cardCount: 16, deckCount: 2 })],
      noSubject: makeHomeSubjectSummary({ cardCount: 3, deckCount: 1 }),
      today: { estimateMsPerCard: 12_000 },
    }));
    vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(makeSubjectLibrary({
      subjects: [physiology],
      decks: [
        { deckId: cardiac.id, subjectId: physiology.id, sortOrder: 0 },
        { deckId: renal.id, subjectId: physiology.id, sortOrder: 1 },
        { deckId: loose.id, subjectId: null, sortOrder: 0 },
      ],
      documents: [{ documentId: lectureId, subjectId: physiology.id }],
    }));
    vi.mocked(lessonsIpc.listSubjectLessons).mockResolvedValue({ subjectId: physiology.id, lessons: [lesson()] });
    vi.mocked(studyGuidesIpc.listStudyGuidePage).mockResolvedValue({ items: [], totalCount: 0, offset: 0, limit: 24 });
    vi.mocked(progressIpc.getProgressOverview).mockImplementation(async (query) => {
      if (query.deckIds[0] === cardiac.id) return overview(0.91, 0.9);
      if (query.deckIds[0] === renal.id) return overview(0.62, 0.81);
      return query.range === "7d" ? overview(0.84, null, [3, 4, 0, 6, 2, 5, 9]) : overview(0.84, 0.8);
    });
    vi.mocked(progressIpc.getProgressForecast).mockResolvedValue({
      days: [15, 8, 6, 7, 5, 6, 4, 5, 9].map((dueCount, index) => ({ localDate: `2026-10-${20 + index}`, dueCount })),
      overdueCount: 0,
      newAvailableCount: 0,
    });
    vi.mocked(progressIpc.getProgressSubjects).mockResolvedValue([]);
    jobs.current = {
      documents: [{ id: lectureId, title: "Renal lecture 4", lifecycleState: "ready", generatedCardCount: 0, generatedDeckId: null, createdAt: 1 }],
    };
  });

  it("shows a subject's facts, what to do next and how each deck is going", async () => {
    const props = renderOverview();

    const facts = await screen.findByTestId("library-overview-facts");
    await waitFor(() => expect(within(facts).getByText("84%")).toBeTruthy());
    await waitFor(() => expect(within(facts).getByText("15")).toBeTruthy());
    expect(within(facts).getByText("About 3 minutes")).toBeTruthy();
    expect(within(facts).getByText("41")).toBeTruthy();
    expect(within(facts).getByText("6")).toBeTruthy();

    const upNext = await screen.findByTestId("library-overview-up-next");
    await waitFor(() => expect(within(upNext).getByText("Renal physiology is slipping")).toBeTruthy());
    expect(within(upNext).getByText("62% remembered, down from 81%")).toBeTruthy();
    expect(within(upNext).getByText("Resume The cardiac cycle")).toBeTruthy();
    expect(within(upNext).getByText("Renal lecture 4 has no cards yet")).toBeTruthy();
    fireEvent.click(within(upNext).getByRole("button", { name: "Practice" }));
    expect(props.onPracticeDeck).toHaveBeenCalledWith(renal.id);

    const strength = screen.getByTestId("library-overview-strength");
    fireEvent.click(within(strength).getByRole("button", { name: "Cardiovascular physiology, 91% remembered" }));
    expect(props.onOpenDeck).toHaveBeenCalledWith(cardiac.id);
    expect(within(strength).queryByText("Loose deck")).toBeNull();
    expect(within(strength).getByRole("img", { name: /^Reviews: .*Today 9\.$/u })).toBeTruthy();
  });

  it("asks about No subject by its decks", async () => {
    renderOverview({ kind: "none" });
    await waitFor(() => expect(progressIpc.getProgressOverview).toHaveBeenCalledWith(
      expect.objectContaining({ range: "30d", subjectIds: [], deckIds: [loose.id] }),
    ));
  });

  it("shows how each subject is going under Everything", async () => {
    vi.mocked(progressIpc.getProgressSubjects).mockResolvedValue([{
      subjectId: physiology.id, name: "Physiology", cardCount: 16, solidCount: 4, dueTodayCount: 15, reviewCount: 30,
      rememberedCount: 25, correctRate: 0.8, retentionReviewCount: 20, retentionRememberedCount: 15, trueRetention: 0.75,
      retentionDelta: -0.12, studyTimeMs: 0,
    }]);
    const props = renderOverview({ kind: "all" });
    const strength = await screen.findByTestId("library-overview-strength");
    expect(within(strength).getByRole("heading", { name: "How each subject is going" })).toBeTruthy();
    fireEvent.click(await within(strength).findByRole("button", { name: "Physiology, 75% remembered" }));
    expect(props.onOpenSubject).toHaveBeenCalledWith(physiology.id);
    expect(await screen.findByText("Physiology is slipping")).toBeTruthy();
  });
});
