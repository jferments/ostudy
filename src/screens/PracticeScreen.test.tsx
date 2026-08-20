import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as analyticsIpc from "../ipc/analytics";
import * as cardsIpc from "../ipc/cards";
import * as commonIpc from "../ipc/common";
import * as decksIpc from "../ipc/decks";
import * as practiceIpc from "../ipc/practice";
import * as schedulingIpc from "../ipc/scheduling";
import * as settingsIpc from "../ipc/settings";
import { deferred } from "../test/factories";
import { PracticeScreen } from "./PracticeScreen";

vi.mock("../editor/RichTextView", () => ({
  RichTextView: () => <div>Card content</div>,
}));
vi.mock("../ipc/analytics", () => ({ getRecentPracticeReviews: vi.fn() }));
vi.mock("../ipc/common", () => ({
  createMutationId: vi.fn(() => "e".repeat(32)),
  createOperationId: vi.fn(() => "0".repeat(32)),
  createSubmissionId: vi.fn(() => "f".repeat(32)),
  getErrorCode: (error: unknown) => {
    if (typeof error !== "object" || error === null) return null;
    const code = (error as { code?: unknown }).code;
    return typeof code === "string" ? code : null;
  },
  getErrorMessage: (error: unknown) => {
    if (error instanceof Error) return error.message;
    if (typeof error === "object" && error !== null) {
      const message = (error as { message?: unknown }).message;
      if (typeof message === "string") return message;
    }
    return "Unexpected error";
  },
}));
vi.mock("../ipc/practice", () => ({
  completePracticeSession: vi.fn(),
  getPracticeCardWindow: vi.fn(),
  getPracticeWorkload: vi.fn(),
  markPracticeCardRevealed: vi.fn(),
  markPracticeCardViewed: vi.fn(),
  refreshPracticeAfterCardEdit: vi.fn(),
  startCardPractice: vi.fn(),
  startSelectedPractice: vi.fn(),
  submitPracticeReview: vi.fn(),
}));
vi.mock("../ipc/scheduling", () => ({
  applyScheduleAction: vi.fn(),
}));

const deck: decksIpc.DeckSummary = {
  id: "a".repeat(32),
  name: "Dog facts",
  description: "",
  sourceNoteCount: 2,
  generatedCardCount: 2,
  studyableDueCount: 1,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};
const secondDeck: decksIpc.DeckSummary = {
  ...deck,
  id: "8".repeat(32),
  name: "Cat facts",
};
const noDueDeck: decksIpc.DeckSummary = {
  ...deck,
  id: "9".repeat(32),
  name: "Reviewed dog facts",
  studyableDueCount: 0,
};
const settings: settingsIpc.AppSettings = {
  theme: "synthwave",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  animationsEnabled: true,
  reduceMotion: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};
const card: cardsIpc.CardPresentation = {
  cardId: "b".repeat(32),
  noteId: "c".repeat(32),
  deckId: deck.id,
  direction: "forward",
  frontDoc: { type: "doc", content: [] },
  backDoc: { type: "doc", content: [] },
  frontText: "Do dogs have fur?",
  backText: "Yes",
  assets: [],
  createdAt: 1,
  updatedAt: 1,
  noteRevision: 1,
};
const practiceCard: practiceIpc.PracticeCard = {
  presentation: card,
  deckName: deck.name,
  dueAt: Date.now(),
  scheduleState: 2,
  needsMorePractice: true,
};
const session: practiceIpc.PracticeSession = {
  id: "d".repeat(32),
  cardCount: 1,
  firstWindow: {
    startPosition: 0,
    cards: [practiceCard],
    hasMore: false,
  },
  shuffle: true,
  mode: "scheduled",
  startedAt: Date.now(),
};
const workload: practiceIpc.PracticeWorkloadOverview = {
  newLimit: 20,
  reviewLimit: 200,
  newCompleted: 0,
  reviewsCompleted: 0,
  newRemaining: 20,
  reviewsRemaining: 200,
  studyableDueReviews: 1,
  studyableDueNew: 0,
  scheduledReviews: 1,
  scheduledNew: 0,
  omittedDueReviews: 0,
  omittedDueNew: 0,
  needsMorePractice: 1,
  extraAvailable: 2,
};
const noDueWorkload: practiceIpc.PracticeWorkloadOverview = {
  ...workload,
  studyableDueReviews: 0,
  studyableDueNew: 0,
  scheduledReviews: 0,
  scheduledNew: 0,
  needsMorePractice: 0,
};
const previews: practiceIpc.ReviewPreviewSet = {
  scheduleRevision: 4,
  previewedAt: Date.now(),
  previews: [
    { outcome: "wrong", dueAt: Date.now() + 600_000, intervalMs: 600_000 },
    {
      outcome: "not_confident",
      dueAt: Date.now() + 86_400_000,
      intervalMs: 86_400_000,
    },
    {
      outcome: "somewhat_confident",
      dueAt: Date.now() + 259_200_000,
      intervalMs: 259_200_000,
    },
    {
      outcome: "very_confident",
      dueAt: Date.now() + 604_800_000,
      intervalMs: 604_800_000,
    },
  ],
};
const completedReview: practiceIpc.ReviewResult = {
  cardId: card.cardId,
  rating: 2,
  outcome: "not_confident",
  dueAt: Date.now() + 86_400_000,
  intervalMs: 86_400_000,
  completed: true,
  ratedCount: 1,
  correctCount: 1,
};

function renderPractice(overrides?: Partial<React.ComponentProps<typeof PracticeScreen>>) {
  const props: React.ComponentProps<typeof PracticeScreen> = {
    decks: [deck],
    settings,
    launchRequest: null,
    onLaunchConsumed: vi.fn(),
    onViewAnalytics: vi.fn(),
    onEndEarly: vi.fn(),
    showToast: vi.fn(),
    ...overrides,
  };
  render(<PracticeScreen {...props} />);
  return props;
}

async function startAndReveal(): Promise<void> {
  await screen.findByText("1 cards in the scheduled session");
  fireEvent.click(screen.getByRole("button", { name: "Start scheduled practice" }));
  fireEvent.click(await screen.findByRole("button", { name: "Show answer" }));
  await screen.findByRole("button", { name: /1 Hard/u });
}

describe("PracticeScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    vi.mocked(analyticsIpc.getRecentPracticeReviews).mockResolvedValue([]);
    vi.mocked(practiceIpc.getPracticeWorkload).mockResolvedValue(workload);
    vi.mocked(practiceIpc.getPracticeCardWindow).mockResolvedValue({
      startPosition: 0,
      cards: [practiceCard],
      hasMore: false,
    });
    vi.mocked(practiceIpc.markPracticeCardViewed).mockResolvedValue(undefined);
    vi.mocked(practiceIpc.markPracticeCardRevealed).mockResolvedValue(previews);
    vi.mocked(practiceIpc.completePracticeSession).mockResolvedValue(undefined);
    vi.mocked(schedulingIpc.applyScheduleAction).mockResolvedValue({
      affectedCount: 1,
      requestedCount: 1,
      changedCount: 1,
      unchangedCount: 0,
      rejectedCount: 0,
      action: "bury",
      resolvedDueAt: null,
    });
    vi.mocked(practiceIpc.startSelectedPractice).mockResolvedValue(session);
    vi.mocked(practiceIpc.startCardPractice).mockResolvedValue({ ...session, mode: "custom" });
    vi.mocked(practiceIpc.submitPracticeReview).mockResolvedValue(completedReview);
  });

  it("selects the eligible setup scope by default and starts with that exact workload scope", async () => {
    renderPractice({ decks: [deck, secondDeck] });
    const dog = await screen.findByRole("checkbox", { name: /Dog facts/u });
    const cat = screen.getByRole("checkbox", { name: /Cat facts/u });
    await waitFor(() => {
      expect((dog as HTMLInputElement).checked).toBe(true);
      expect((cat as HTMLInputElement).checked).toBe(true);
      expect(practiceIpc.getPracticeWorkload).toHaveBeenCalledWith(
        expect.arrayContaining([deck.id, secondDeck.id]),
        [],
      );
    });

    expect(await screen.findByText("1 cards in the scheduled session")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Start scheduled practice" }));
    await waitFor(() => {
      const input = vi.mocked(practiceIpc.startSelectedPractice).mock.calls[0]?.[0];
      expect(new Set(input?.deckIds)).toEqual(new Set([deck.id, secondDeck.id]));
      expect(input?.tagIds).toEqual([]);
      expect(input?.mode).toBe("scheduled");
    });
  });

  it("discards a slower workload response for an obsolete deck scope", async () => {
    const first = deferred<practiceIpc.PracticeWorkloadOverview>();
    const second = deferred<practiceIpc.PracticeWorkloadOverview>();
    vi.mocked(practiceIpc.getPracticeWorkload)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    renderPractice({ decks: [deck, secondDeck] });

    const cat = await screen.findByRole("checkbox", { name: /Cat facts/u });
    await waitFor(() => expect(practiceIpc.getPracticeWorkload).toHaveBeenCalledTimes(1));
    fireEvent.click(cat);
    await waitFor(() => expect(practiceIpc.getPracticeWorkload).toHaveBeenCalledTimes(2));

    second.resolve({
      ...workload,
      studyableDueReviews: 9,
      scheduledReviews: 9,
    });
    expect(await screen.findByText("9 cards in the scheduled session")).toBeTruthy();
    first.resolve(workload);
    await Promise.resolve();
    expect(screen.getByText("9 cards in the scheduled session")).toBeTruthy();
    expect(screen.queryByText("1 cards in the scheduled session")).toBeNull();
  });

  it("shows workload errors as unavailable state instead of zero metrics", async () => {
    vi.mocked(practiceIpc.getPracticeWorkload).mockRejectedValue(new Error("Planner unavailable"));
    renderPractice();

    expect(await screen.findByText("Workload unavailable: Planner unavailable")).toBeTruthy();
    const panel = screen.getByRole("region", { name: "Practice workload" });
    expect(within(panel).getAllByText("—")).toHaveLength(4);
    expect(within(panel).queryByText(/^0$/u)).toBeNull();
    expect(screen.getByRole("button", { name: "Start scheduled practice" })).toBeDisabled();
  });

  it("offers explicit extra practice when the loaded workload is truly zero", async () => {
    vi.mocked(practiceIpc.getPracticeWorkload).mockResolvedValue(noDueWorkload);
    renderPractice({ decks: [noDueDeck] });
    const panel = await screen.findByRole("region", { name: "Practice workload" });
    expect(within(panel).getAllByText("0").length).toBeGreaterThan(0);
    fireEvent.click(await screen.findByRole("button", { name: "Study this selection anyway" }));
    await waitFor(() =>
      expect(practiceIpc.startSelectedPractice).toHaveBeenCalledWith({
        operationId: "0".repeat(32),
        deckIds: [noDueDeck.id],
        tagIds: [],
        shuffle: true,
        mode: "extra",
      }),
    );
  });

  it("submits the FSRS preview revision from the compact rating controls", async () => {
    renderPractice();
    await startAndReveal();
    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    await waitFor(() =>
      expect(practiceIpc.submitPracticeReview).toHaveBeenCalledWith(
        expect.objectContaining({
          outcome: "not_confident",
          scheduleRevision: 4,
        }),
      ),
    );
  });

  it("opens analytics for the completed session and its deck scope", async () => {
    const onViewAnalytics = vi.fn();
    renderPractice({ onViewAnalytics });
    await startAndReveal();
    fireEvent.click(await screen.findByRole("button", { name: /1 Hard/u }));
    fireEvent.click(await screen.findByRole("button", { name: "View analytics" }));
    expect(onViewAnalytics).toHaveBeenCalledWith(session.id, [deck.id]);
  });

  it("starts filtered cards as a custom session", async () => {
    const onLaunchConsumed = vi.fn();
    renderPractice({
      launchRequest: {
        requestId: 8,
        deckIds: [],
        tagIds: [],
        cardIds: [card.cardId],
        autoStart: true,
        mode: "scheduled",
        returnTarget: { section: "cards" },
      },
      onLaunchConsumed,
    });
    await waitFor(() => {
      expect(onLaunchConsumed).toHaveBeenCalledTimes(1);
      expect(practiceIpc.startCardPractice).toHaveBeenCalledWith({
        operationId: "0".repeat(32),
        cardIds: [card.cardId],
        shuffle: true,
      });
    });
  });

  it("auto-starts a deck-origin launch as explicit extra practice instead of the daily limit", async () => {
    renderPractice({
      launchRequest: {
        requestId: 9,
        deckIds: [deck.id],
        tagIds: [],
        cardIds: [],
        autoStart: true,
        mode: "extra",
        returnTarget: { section: "decks", deckId: deck.id },
      },
    });
    await waitFor(() =>
      expect(practiceIpc.startSelectedPractice).toHaveBeenCalledWith({
        operationId: "0".repeat(32),
        deckIds: [deck.id],
        tagIds: [],
        shuffle: true,
        mode: "extra",
      }),
    );
  });

  it("returns to an external launch origin when End session terminates early", async () => {
    const onEndEarly = vi.fn();
    const returnTarget = { section: "decks" as const, deckId: deck.id };
    renderPractice({
      launchRequest: {
        requestId: 10,
        deckIds: [deck.id],
        tagIds: [],
        cardIds: [],
        autoStart: true,
        mode: "extra",
        returnTarget,
      },
      onEndEarly,
    });
    fireEvent.click(await screen.findByRole("button", { name: "End session" }));
    await waitFor(() => {
      expect(practiceIpc.completePracticeSession).toHaveBeenCalledWith(session.id);
      expect(onEndEarly).toHaveBeenCalledWith(returnTarget);
    });
  });

  it("reuses one practice-start operation after an ambiguous transport failure", async () => {
    vi.mocked(practiceIpc.startSelectedPractice)
      .mockRejectedValueOnce(new Error("Native response was lost."))
      .mockResolvedValueOnce(session);
    renderPractice();
    await screen.findByText("1 cards in the scheduled session");

    fireEvent.click(screen.getByRole("button", { name: "Start scheduled practice" }));
    expect(
      await screen.findByText(/safely resume it without creating a duplicate session/u),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Start scheduled practice" }));

    await screen.findByRole("button", { name: "Show answer" });
    expect(practiceIpc.startSelectedPractice).toHaveBeenCalledTimes(2);
    const first = vi.mocked(practiceIpc.startSelectedPractice).mock.calls[0]?.[0];
    const second = vi.mocked(practiceIpc.startSelectedPractice).mock.calls[1]?.[0];
    expect(first?.operationId).toBe("0".repeat(32));
    expect(second).toEqual(first);
    expect(commonIpc.createOperationId).toHaveBeenCalledTimes(1);
  });

  it("retries an ambiguous review with the same submission and payload", async () => {
    vi.mocked(practiceIpc.submitPracticeReview)
      .mockRejectedValueOnce(new Error("Native response was lost."))
      .mockResolvedValueOnce(completedReview);
    renderPractice();
    await startAndReveal();

    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    expect(
      await screen.findByText(/Choose 1 again to safely retry the same review/u),
    ).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: /2 Medium/u }));
    expect(await screen.findByText(/result of rating 1 is still unknown/u)).toBeTruthy();
    expect(practiceIpc.submitPracticeReview).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    await screen.findByRole("button", { name: "View analytics" });

    expect(practiceIpc.submitPracticeReview).toHaveBeenCalledTimes(2);
    const first = vi.mocked(practiceIpc.submitPracticeReview).mock.calls[0]?.[0];
    const second = vi.mocked(practiceIpc.submitPracticeReview).mock.calls[1]?.[0];
    expect(second).toEqual(first);
    expect(first?.submissionId).toBe("f".repeat(32));
    expect(commonIpc.createSubmissionId).toHaveBeenCalledTimes(1);
  });
});
