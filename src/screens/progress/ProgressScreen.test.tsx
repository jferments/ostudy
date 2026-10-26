import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AnalyticsLaunchRequest } from "../../App";
import type { RecentReview, TagAnalytics } from "../../ipc/analytics";
import * as progressIpc from "../../ipc/progress";
import type { ProgressActivity, ProgressBucket, ProgressOverview, ProgressSummary, ReviewLogPage } from "../../ipc/progress";
import * as subjectsIpc from "../../ipc/subjects";
import type { SubjectLibrary } from "../../ipc/subjects";
import { deferred, makeDeck } from "../../test/factories";
import { OverlayProvider } from "../../ui/overlays/OverlayProvider";
import { ProgressScreen } from "./ProgressScreen";

vi.mock("../../ipc/progress", () => ({
  getProgressOverview: vi.fn(),
  getProgressForecast: vi.fn(),
  getProgressCardStrength: vi.fn(),
  getProgressActivity: vi.fn(),
  getProgressSubjects: vi.fn(),
  getProgressQuestionTypes: vi.fn(),
  getProgressSessions: vi.fn(),
  getProgressTopics: vi.fn(),
  getProgressFilterOptions: vi.fn(),
  getReviewLog: vi.fn(),
}));
vi.mock("../../ipc/subjects", () => ({ getSubjectLibrary: vi.fn() }));
vi.mock("../../ipc/common", () => ({
  getErrorMessage: (error: unknown) => (error instanceof Error ? error.message : "Unexpected error"),
}));

const TODAY = "2026-10-05";
const SUBJECT = "5".repeat(32);
const TAG = "7".repeat(32);

function summary(overrides: Partial<ProgressSummary> = {}): ProgressSummary {
  return {
    reviewCount: 100, newCardCount: 10, rememberedCount: 89, correctRate: 0.89, averageConfidence: 2,
    averageResponseTimeMs: 4_000, studyTimeMs: 3_600_000, retentionReviewCount: 60, retentionRememberedCount: 54,
    trueRetention: 0.9, ...overrides,
  };
}

function bucket(localDate: string, reviewCount: number, rememberedCount: number): ProgressBucket {
  return { ...summary({ reviewCount, rememberedCount, correctRate: reviewCount ? rememberedCount / reviewCount : null }), startLocalDate: localDate, endLocalDate: localDate };
}

function overview(overrides: Partial<ProgressOverview> = {}): ProgressOverview {
  return {
    scope: {
      range: "30d", startLocalDate: "2026-09-06", endLocalDate: TODAY, startAt: 1, endAt: 2, bucketGranularity: "day",
      todayLocalDate: TODAY, deckIds: [], tagIds: [], highlightedSessionId: null,
    },
    summary: summary(),
    previousSummary: summary({ correctRate: 0.86 }),
    studyDayCount: 24,
    timeline: [bucket("2026-10-03", 40, 36), bucket("2026-10-04", 0, 0), bucket(TODAY, 60, 53)],
    desiredRetention: 0.9,
    ...overrides,
  };
}

function topic(overrides: Partial<TagAnalytics> = {}): TagAnalytics {
  return {
    tagId: TAG, tagName: "Muscarinic antagonists", isCurrent: true, sourceNoteCount: 10, generatedCardCount: 20,
    studyableCardCount: 20, studyableDueCount: 14, needsMorePracticeCount: 3, cardsPracticed: 18, reviewCount: 40,
    correctCount: 24, correctRate: 0.61, averageConfidence: 1.2, averageResponseTimeMs: 5_000, totalStudyTimeMs: 600_000,
    lastPracticedAt: 1, retentionReviewCount: 20, retentionRememberedCount: 12, trueRetention: 0.6, trendDelta: -0.08,
    status: "needs_attention", ...overrides,
  };
}

function activity(overrides: Partial<ProgressActivity> = {}): ProgressActivity {
  return {
    startLocalDate: "2026-04-06", todayLocalDate: TODAY,
    days: [{ localDate: "2026-10-04", reviewCount: 30, studyTimeMs: 600_000 }],
    currentStreak: { length: 12, startLocalDate: "2026-09-23", endLocalDate: "2026-10-04" },
    longestStreak: { length: 19, startLocalDate: "2026-07-02", endLocalDate: "2026-07-20" },
    busiestWeekday: 3,
    ...overrides,
  };
}

function review(overrides: Partial<RecentReview> = {}): RecentReview {
  return {
    id: "r".repeat(32), deckName: "Pharmacology", capturedDeckName: "Pharmacology", cardId: "c".repeat(32), cardLabel: "Card",
    questionExcerpt: "Which receptor does atropine block?", tags: [{ tagId: TAG, tagName: "Muscarinic antagonists", isCurrent: true }],
    outcome: "somewhat_confident", rating: 3, responseTimeMs: 4_200, reviewedAt: 1_800_000_000_000, reviewLocalDate: TODAY,
    dueAfter: 1_800_100_000_000, attributionSource: "captured", ...overrides,
  };
}

const library: SubjectLibrary = {
  subjects: [{ id: SUBJECT, name: "Pharmacology", sortOrder: 0, createdAt: 1, updatedAt: 1, revision: 1 }],
  decks: [{ deckId: "d".repeat(32), subjectId: SUBJECT, sortOrder: 0 }],
} as unknown as SubjectLibrary;

const firstPage: ReviewLogPage = { reviews: [review()], next: { reviewedAt: 1, createdAt: 1, id: "n".repeat(32) } };

function mockAll() {
  vi.mocked(progressIpc.getProgressOverview).mockResolvedValue(overview());
  vi.mocked(progressIpc.getProgressForecast).mockResolvedValue({
    days: Array.from({ length: 14 }, (_, index) => ({ localDate: `2026-10-${String(5 + index).padStart(2, "0")}`, dueCount: index === 2 ? 96 : 20 })),
    overdueCount: 3, newAvailableCount: 5,
  });
  vi.mocked(progressIpc.getProgressCardStrength).mockResolvedValue({
    newCount: 120, learningCount: 64, gettingThereCount: 219, solidCount: 512, solidStabilityDays: 21,
    estimatedRecall: 0.93, estimatedRecallCardCount: 795,
  });
  vi.mocked(progressIpc.getProgressActivity).mockResolvedValue(activity());
  vi.mocked(progressIpc.getProgressSubjects).mockResolvedValue([{
    subjectId: SUBJECT, name: "Pharmacology", cardCount: 214, solidCount: 98, dueTodayCount: 11, reviewCount: 300,
    rememberedCount: 240, correctRate: 0.8, retentionReviewCount: 100, retentionRememberedCount: 81, trueRetention: 0.81,
    retentionDelta: -0.05, studyTimeMs: 5_760_000,
  }]);
  vi.mocked(progressIpc.getProgressQuestionTypes).mockResolvedValue([]);
  vi.mocked(progressIpc.getProgressSessions).mockResolvedValue([]);
  vi.mocked(progressIpc.getProgressTopics).mockResolvedValue([topic(), topic({ tagId: "8".repeat(32), tagName: "Heart sounds", status: "on_track", correctRate: 0.76, trendDelta: 0.04, studyableDueCount: 5 })]);
  vi.mocked(progressIpc.getProgressFilterOptions).mockResolvedValue({ decks: [], tags: [{ id: TAG, name: "Muscarinic antagonists", isCurrent: true, hasHistory: true }] });
  vi.mocked(progressIpc.getReviewLog).mockResolvedValue(firstPage);
  vi.mocked(subjectsIpc.getSubjectLibrary).mockResolvedValue(library);
}

function renderScreen(props: Partial<Parameters<typeof ProgressScreen>[0]> = {}) {
  const callbacks = { onLaunchConsumed: vi.fn(), onPracticeScope: vi.fn(), onSearchTag: vi.fn() };
  const view = render(
    <OverlayProvider>
      <ProgressScreen decks={[makeDeck({ id: "d".repeat(32), studyableDueCount: 11 }), makeDeck({ studyableDueCount: 0 })]} launchRequest={null} {...callbacks} {...props} />
    </OverlayProvider>,
  );
  return { ...view, ...callbacks };
}

describe("ProgressScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockAll();
  });

  it("puts the scheduled review in the header, over the decks with cards due, with no summary paragraph", async () => {
    const { onPracticeScope } = renderScreen();
    await screen.findByRole("region", { name: "Learning summary" });
    expect(screen.queryByRole("region", { name: "Summary" })).toBeNull();
    const [review] = await screen.findAllByRole("button", { name: /Review 20 due/u });
    fireEvent.click(review!);
    expect(onPracticeScope).toHaveBeenCalledWith(["d".repeat(32)], []);
  });

  it("explains the page, spaced repetition and FSRS with Start review before anything was studied", async () => {
    vi.mocked(progressIpc.getProgressActivity).mockResolvedValue(activity({ currentStreak: null, longestStreak: null, days: [] }));
    const { onPracticeScope } = renderScreen();
    expect(await screen.findByText("Your progress shows up here once you review")).toBeTruthy();
    expect(screen.getByRole("list", { name: "What Progress shows" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "How spaced repetition works" })).toBeTruthy();
    expect(screen.getByText("FSRS")).toBeTruthy();
    expect(screen.getByRole("figure", { name: "A preview of Progress after three weeks of study" })).toBeTruthy();
    expect(screen.queryByRole("region", { name: "Learning summary" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Start review" }));
    expect(onPracticeScope).toHaveBeenCalledWith(["d".repeat(32)], []);
  });

  it("shows the tiles, forecast peak, strength and activity facts", async () => {
    renderScreen();
    const tiles = await screen.findByRole("region", { name: "Learning summary" });
    await within(tiles).findByText("89%");
    expect(within(tiles).getByText("3 pts")).toBeTruthy();
    expect(within(tiles).getByText("1 hr")).toBeTruthy();
    expect(within(tiles).getByText("12-day streak")).toBeTruthy();
    expect(within(tiles).getByText("this month")).toBeTruthy();
    expect(await within(tiles).findByText("512")).toBeTruthy();
    expect(within(tiles).getByRole("button", { name: "What solid means" })).toBeTruthy();
    expect(screen.queryByText(/Each bar is one/u)).toBeNull();
    expect(screen.queryByText("Cards due each day, next two weeks")).toBeNull();
    expect(await screen.findByText("93%")).toBeTruthy();
    expect(await screen.findByText("12 days, since September 23")).toBeTruthy();
    expect(screen.getByText("19 days, July 2 to July 20")).toBeTruthy();
    expect(screen.getByText("Thursday")).toBeTruthy();
  });

  it("reloads the scoped sections, not the library-wide calendar, when the range changes", async () => {
    renderScreen();
    await screen.findByText("89%");
    await waitFor(() => expect(progressIpc.getProgressForecast).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getAllByRole("radio", { name: "90 days" })[0]!);
    await waitFor(() => expect(progressIpc.getProgressOverview).toHaveBeenLastCalledWith({ range: "90d", subjectIds: [], deckIds: [], tagIds: [] }));
    expect(progressIpc.getProgressForecast).toHaveBeenCalledTimes(2);
    expect(progressIpc.getProgressActivity).toHaveBeenCalledTimes(1);
    expect(subjectsIpc.getSubjectLibrary).toHaveBeenCalledTimes(1);
  });

  it("keeps the newest overview when an older one resolves last", async () => {
    const older = deferred<ProgressOverview>();
    const newer = deferred<ProgressOverview>();
    vi.mocked(progressIpc.getProgressOverview).mockReturnValueOnce(older.promise).mockReturnValueOnce(newer.promise);
    renderScreen();
    fireEvent.click(screen.getAllByRole("radio", { name: "7 days" })[0]!);
    await act(async () => newer.resolve(overview({ summary: summary({ correctRate: 0.5, rememberedCount: 50 }) })));
    await act(async () => older.resolve(overview({ summary: summary({ correctRate: 0.99, rememberedCount: 99 }) })));
    const tiles = screen.getByRole("region", { name: "Learning summary" });
    expect(within(tiles).getByText("50%")).toBeTruthy();
    expect(within(tiles).queryByText("99%")).toBeNull();
  });

  it("narrows to a subject from its row", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: "Show only Pharmacology" }));
    await waitFor(() => expect(progressIpc.getProgressOverview).toHaveBeenLastCalledWith({ range: "30d", subjectIds: [SUBJECT], deckIds: [], tagIds: [] }));
    expect((screen.getByRole("combobox", { name: "Subject" }) as HTMLSelectElement).value).toBe(SUBJECT);
  });

  it("applies a launch scope once and highlights the finished session", async () => {
    const launch: AnalyticsLaunchRequest = { requestId: 4, range: "7d", deckIds: ["d".repeat(32)], tagIds: [], highlightedSessionId: "s".repeat(32) };
    const { onLaunchConsumed } = renderScreen({ launchRequest: launch });
    await waitFor(() => expect(progressIpc.getProgressOverview).toHaveBeenLastCalledWith({ range: "7d", subjectIds: [], deckIds: ["d".repeat(32)], tagIds: [] }));
    expect(onLaunchConsumed).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("button", { name: "Remove Finished session highlighted filter" })).toBeTruthy();
  });

  it("lists recent answers, searches their tags and pages the review log", async () => {
    const { onSearchTag } = renderScreen();
    const panel = (await screen.findByRole("heading", { name: "Every answer" })).closest("section")!;
    expect(await within(panel).findByText("Which receptor does atropine block?")).toBeTruthy();
    fireEvent.click(within(panel).getByRole("button", { name: "Search cards tagged Muscarinic antagonists" }));
    expect(onSearchTag).toHaveBeenCalledWith(TAG);

    vi.mocked(progressIpc.getReviewLog).mockResolvedValueOnce({ reviews: [review({ id: "o".repeat(32), questionExcerpt: "An older answer" })], next: null });
    fireEvent.click(within(panel).getByRole("button", { name: "Open review log" }));
    const log = await screen.findByRole("dialog", { name: "Review log" });
    fireEvent.click(within(log).getByRole("button", { name: "Show older answers" }));
    expect(await within(log).findByText("An older answer")).toBeTruthy();
    expect(progressIpc.getReviewLog).toHaveBeenLastCalledWith({ range: "30d", subjectIds: [], deckIds: [], tagIds: [] }, firstPage.next);
  });

  it("keeps showing the other sections when one fails", async () => {
    vi.mocked(progressIpc.getProgressForecast).mockRejectedValue(new Error("Forecast unavailable"));
    renderScreen();
    expect(await screen.findByText("Forecast unavailable")).toBeTruthy();
    expect(await screen.findByText("12 days, since September 23")).toBeTruthy();
  });

  it("narrows by tag from the filter sheet", async () => {
    renderScreen();
    fireEvent.click(await screen.findByRole("button", { name: /Decks and tags/u }));
    const sheet = await screen.findByRole("dialog", { name: "Decks, tags and dates" });
    fireEvent.click(await within(sheet).findByRole("checkbox", { name: "Muscarinic antagonists" }));
    await waitFor(() => expect(progressIpc.getProgressTopics).toHaveBeenLastCalledWith({ range: "30d", subjectIds: [], deckIds: [], tagIds: [TAG] }));
  });
});
