import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as ipc from "../ipc";
import { PracticeScreen } from "./PracticeScreen";

vi.mock("../editor/RichTextView", () => ({ RichTextView: () => <div>Card content</div> }));
vi.mock("../ipc", () => ({
  applyScheduleAction: vi.fn(),
  completePracticeSession: vi.fn(),
  createMutationId: () => "e".repeat(32),
  createSubmissionId: () => "f".repeat(32),
  getAnalytics: vi.fn(),
  getErrorMessage: (error: unknown) => error instanceof Error ? error.message : "Unexpected error",
  getPracticeWorkload: vi.fn(),
  markPracticeCardRevealed: vi.fn(),
  markPracticeCardViewed: vi.fn(),
  startCardPractice: vi.fn(),
  startSelectedPractice: vi.fn(),
  submitPracticeReview: vi.fn(),
}));

const deck: ipc.Deck = { id: "a".repeat(32), name: "Dog facts", description: "", cardCount: 2, dueCount: 1, createdAt: 1, updatedAt: 1, revision: 1 };
const noDueDeck: ipc.Deck = { ...deck, id: "9".repeat(32), name: "Reviewed dog facts", dueCount: 0 };
const settings: ipc.AppSettings = {
  theme: "synthwave", uiFont: "system", cardFont: "system", uiScale: 100,
  cardBaseSize: 18, reduceMotion: false, confirmDeletions: true,
  practiceShuffle: true, desiredRetention: 0.9, dailyNewLimit: 20, dailyReviewLimit: 200,
};
const card: ipc.Flashcard = {
  id: "b".repeat(32), noteId: "c".repeat(32), deckId: deck.id, direction: "forward",
  frontDoc: { type: "doc", content: [] }, backDoc: { type: "doc", content: [] },
  frontText: "Do dogs have fur?", backText: "Yes", assets: [], createdAt: 1, updatedAt: 1, revision: 1,
};
const session: ipc.PracticeSession = {
  id: "d".repeat(32),
  cards: [{ card, deckName: deck.name, dueAt: Date.now(), scheduleState: 2, needsMorePractice: true }],
  shuffle: true, mode: "scheduled", startedAt: Date.now(),
};
const workload: ipc.PracticeWorkloadOverview = {
  newLimit: 20, reviewLimit: 200, newCompleted: 0, reviewsCompleted: 0,
  newRemaining: 20, reviewsRemaining: 200, dueReviews: 1, dueNew: 0,
  scheduledReviews: 1, scheduledNew: 0, omittedDueReviews: 0, omittedDueNew: 0,
  needsMorePractice: 1, extraAvailable: 2,
};
const noDueWorkload: ipc.PracticeWorkloadOverview = { ...workload, dueReviews: 0, scheduledReviews: 0, needsMorePractice: 0 };
const analytics: ipc.Analytics = {
  deckCount: 1, cardCount: 2, dueNow: 1, reviewCount: 0, correctCount: 0,
  correctRate: null, averageResponseTimeMs: null,
  ratingDistribution: { again: 0, hard: 0, good: 0, easy: 0 }, deckStats: [], recentReviews: [],
};
const previews: ipc.ReviewPreviewSet = {
  scheduleRevision: 4, previewedAt: Date.now(),
  previews: [
    { outcome: "wrong", dueAt: Date.now() + 600_000, intervalMs: 600_000 },
    { outcome: "not_confident", dueAt: Date.now() + 86_400_000, intervalMs: 86_400_000 },
    { outcome: "somewhat_confident", dueAt: Date.now() + 259_200_000, intervalMs: 259_200_000 },
    { outcome: "very_confident", dueAt: Date.now() + 604_800_000, intervalMs: 604_800_000 },
  ],
};

describe("PracticeScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(ipc.getAnalytics).mockResolvedValue(analytics);
    vi.mocked(ipc.getPracticeWorkload).mockResolvedValue(workload);
    vi.mocked(ipc.markPracticeCardViewed).mockResolvedValue(undefined);
    vi.mocked(ipc.markPracticeCardRevealed).mockResolvedValue(previews);
    vi.mocked(ipc.completePracticeSession).mockResolvedValue(undefined);
    vi.mocked(ipc.applyScheduleAction).mockResolvedValue({ affectedCount: 1, action: "bury" });
    vi.mocked(ipc.startSelectedPractice).mockResolvedValue(session);
    vi.mocked(ipc.startCardPractice).mockResolvedValue({ ...session, mode: "custom" });
    vi.mocked(ipc.submitPracticeReview).mockResolvedValue({
      cardId: card.id, rating: 2, outcome: "not_confident", dueAt: Date.now() + 86_400_000,
      intervalMs: 86_400_000, completed: true, ratedCount: 1, correctCount: 1,
    });
  });

  it("shows authoritative daily workload and starts scheduled mode", async () => {
    render(<PracticeScreen decks={[deck]} settings={settings} launchRequest={null} onLaunchConsumed={vi.fn()} showToast={vi.fn()} />);
    fireEvent.click(await screen.findByRole("checkbox", { name: /Dog facts/u }));
    expect(await screen.findByText("1 cards in the scheduled session")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Start scheduled practice" }));
    await waitFor(() => expect(ipc.startSelectedPractice).toHaveBeenCalledWith({ deckIds: [deck.id], shuffle: true, mode: "scheduled" }));
  });

  it("offers explicit extra practice when nothing fits the scheduled workload", async () => {
    vi.mocked(ipc.getPracticeWorkload).mockResolvedValue(noDueWorkload);
    render(<PracticeScreen decks={[noDueDeck]} settings={settings} launchRequest={null} onLaunchConsumed={vi.fn()} showToast={vi.fn()} />);
    fireEvent.click(await screen.findByRole("checkbox", { name: /Reviewed dog facts/u }));
    fireEvent.click(await screen.findByRole("button", { name: "Study this selection anyway" }));
    await waitFor(() => expect(ipc.startSelectedPractice).toHaveBeenCalledWith({ deckIds: [noDueDeck.id], shuffle: true, mode: "extra" }));
  });

  it("shows FSRS interval previews and submits the preview revision", async () => {
    render(<PracticeScreen decks={[deck]} settings={settings} launchRequest={null} onLaunchConsumed={vi.fn()} showToast={vi.fn()} />);
    fireEvent.click(await screen.findByRole("checkbox", { name: /Dog facts/u }));
    await screen.findByText("1 cards in the scheduled session");
    fireEvent.click(screen.getByRole("button", { name: "Start scheduled practice" }));
    fireEvent.click(await screen.findByRole("button", { name: "Show answer" }));
    expect(await screen.findByRole("button", { name: /1 Hard, About 1 day/u })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    await waitFor(() => expect(ipc.submitPracticeReview).toHaveBeenCalledWith(expect.objectContaining({ outcome: "not_confident", scheduleRevision: 4 })));
  });

  it("starts filtered cards as a custom session", async () => {
    const onLaunchConsumed = vi.fn();
    render(<PracticeScreen decks={[deck]} settings={settings} launchRequest={{ requestId: 8, deckIds: [], cardIds: [card.id], autoStart: true }} onLaunchConsumed={onLaunchConsumed} showToast={vi.fn()} />);
    await waitFor(() => {
      expect(onLaunchConsumed).toHaveBeenCalledTimes(1);
      expect(ipc.startCardPractice).toHaveBeenCalledWith({ cardIds: [card.id], shuffle: true });
    });
  });
});