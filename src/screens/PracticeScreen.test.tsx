import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as analyticsIpc from "../analytics-ipc";
import * as ipc from "../ipc";
import { deferred } from "../test/factories";
import { PracticeScreen } from "./PracticeScreen";

vi.mock("../editor/RichTextView", () => ({
  RichTextView: () => <div>Card content</div>,
}));
vi.mock("../analytics-ipc", () => ({ getRecentPracticeReviews: vi.fn() }));
vi.mock("../ipc", () => ({
  applyScheduleAction: vi.fn(),
  completePracticeSession: vi.fn(),
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
  getPracticeCardWindow: vi.fn(),
  getPracticeWorkload: vi.fn(),
  markPracticeCardRevealed: vi.fn(),
  markPracticeCardViewed: vi.fn(),
  startCardPractice: vi.fn(),
  startSelectedPractice: vi.fn(),
  submitPracticeReview: vi.fn(),
}));

const deck: ipc.DeckSummary = {
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
const secondDeck: ipc.DeckSummary = {
  ...deck,
  id: "8".repeat(32),
  name: "Cat facts",
};
const noDueDeck: ipc.DeckSummary = {
  ...deck,
  id: "9".repeat(32),
  name: "Reviewed dog facts",
  studyableDueCount: 0,
};
const settings: ipc.AppSettings = {
  theme: "synthwave",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  reduceMotion: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};
const card: ipc.Flashcard = {
  id: "b".repeat(32),
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
  revision: 1,
};
const practiceCard: ipc.PracticeCard = {
  card,
  deckName: deck.name,
  dueAt: Date.now(),
  scheduleState: 2,
  needsMorePractice: true,
};
const session: ipc.PracticeSession = {
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
const workload: ipc.PracticeWorkloadOverview = {
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
const noDueWorkload: ipc.PracticeWorkloadOverview = {
  ...workload,
  studyableDueReviews: 0,
  scheduledReviews: 0,
  needsMorePractice: 0,
};
const previews: ipc.ReviewPreviewSet = {
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
const completedReview: ipc.ReviewResult = {
  cardId: card.id,
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
    showToast: vi.fn(),
    ...overrides,
  };
  render(<PracticeScreen {...props} />);
  return props;
}

async function startAndReveal(): Promise<void> {
  fireEvent.click(await screen.findByRole("checkbox", { name: /Dog facts/u }));
  await screen.findByText("1 cards in the scheduled session");
  fireEvent.click(screen.getByRole("button", { name: "Start scheduled practice" }));
  fireEvent.click(await screen.findByRole("button", { name: "Show answer" }));
  await screen.findByRole("button", { name: /1 Hard, About 1 day/u });
}

describe("PracticeScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(analyticsIpc.getRecentPracticeReviews).mockResolvedValue([]);
    vi.mocked(ipc.getPracticeWorkload).mockResolvedValue(workload);
    vi.mocked(ipc.getPracticeCardWindow).mockResolvedValue({
      startPosition: 0,
      cards: [practiceCard],
      hasMore: false,
    });
    vi.mocked(ipc.markPracticeCardViewed).mockResolvedValue(undefined);
    vi.mocked(ipc.markPracticeCardRevealed).mockResolvedValue(previews);
    vi.mocked(ipc.completePracticeSession).mockResolvedValue(undefined);
    vi.mocked(ipc.applyScheduleAction).mockResolvedValue({
      affectedCount: 1,
      requestedCount: 1,
      changedCount: 1,
      unchangedCount: 0,
      rejectedCount: 0,
      action: "bury",
      resolvedDueAt: null,
    });
    vi.mocked(ipc.startSelectedPractice).mockResolvedValue(session);
    vi.mocked(ipc.startCardPractice).mockResolvedValue({ ...session, mode: "custom" });
    vi.mocked(ipc.submitPracticeReview).mockResolvedValue(completedReview);
  });

  it("shows authoritative daily workload and starts scheduled mode", async () => {
    renderPractice();
    fireEvent.click(await screen.findByRole("checkbox", { name: /Dog facts/u }));
    expect(await screen.findByText("1 cards in the scheduled session")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Start scheduled practice" }));
    await waitFor(() =>
      expect(ipc.startSelectedPractice).toHaveBeenCalledWith({
        operationId: "0".repeat(32),
        deckIds: [deck.id],
        tagIds: [],
        shuffle: true,
        mode: "scheduled",
      }),
    );
  });

  it("discards a slower workload response for an obsolete deck scope", async () => {
    const first = deferred<ipc.PracticeWorkloadOverview>();
    const second = deferred<ipc.PracticeWorkloadOverview>();
    vi.mocked(ipc.getPracticeWorkload)
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    renderPractice({ decks: [deck, secondDeck] });

    fireEvent.click(await screen.findByRole("checkbox", { name: /Dog facts/u }));
    await waitFor(() => expect(ipc.getPracticeWorkload).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("checkbox", { name: /Cat facts/u }));
    await waitFor(() => expect(ipc.getPracticeWorkload).toHaveBeenCalledTimes(2));

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

  it("offers explicit extra practice when nothing fits the scheduled workload", async () => {
    vi.mocked(ipc.getPracticeWorkload).mockResolvedValue(noDueWorkload);
    renderPractice({ decks: [noDueDeck] });
    fireEvent.click(await screen.findByRole("checkbox", { name: /Reviewed dog facts/u }));
    fireEvent.click(await screen.findByRole("button", { name: "Study this selection anyway" }));
    await waitFor(() =>
      expect(ipc.startSelectedPractice).toHaveBeenCalledWith({
        operationId: "0".repeat(32),
        deckIds: [noDueDeck.id],
        tagIds: [],
        shuffle: true,
        mode: "extra",
      }),
    );
  });

  it("shows FSRS interval previews and submits the preview revision", async () => {
    renderPractice();
    await startAndReveal();
    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    await waitFor(() =>
      expect(ipc.submitPracticeReview).toHaveBeenCalledWith(
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
        cardIds: [card.id],
        autoStart: true,
      },
      onLaunchConsumed,
    });
    await waitFor(() => {
      expect(onLaunchConsumed).toHaveBeenCalledTimes(1);
      expect(ipc.startCardPractice).toHaveBeenCalledWith({
        operationId: "0".repeat(32),
        cardIds: [card.id],
        shuffle: true,
      });
    });
  });

  it("reuses one practice-start operation after an ambiguous transport failure", async () => {
    vi.mocked(ipc.startSelectedPractice)
      .mockRejectedValueOnce(new Error("Native response was lost."))
      .mockResolvedValueOnce(session);
    renderPractice();
    fireEvent.click(await screen.findByRole("checkbox", { name: /Dog facts/u }));
    await screen.findByText("1 cards in the scheduled session");

    fireEvent.click(screen.getByRole("button", { name: "Start scheduled practice" }));
    expect(
      await screen.findByText(/safely resume it without creating a duplicate session/u),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Start scheduled practice" }));

    await screen.findByRole("button", { name: "Show answer" });
    expect(ipc.startSelectedPractice).toHaveBeenCalledTimes(2);
    const first = vi.mocked(ipc.startSelectedPractice).mock.calls[0]?.[0];
    const second = vi.mocked(ipc.startSelectedPractice).mock.calls[1]?.[0];
    expect(first?.operationId).toBe("0".repeat(32));
    expect(second).toEqual(first);
    expect(ipc.createOperationId).toHaveBeenCalledTimes(1);
  });

  it("retries an ambiguous review with the same submission and payload", async () => {
    vi.mocked(ipc.submitPracticeReview)
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
    expect(ipc.submitPracticeReview).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    await screen.findByRole("button", { name: "View analytics" });

    expect(ipc.submitPracticeReview).toHaveBeenCalledTimes(2);
    const first = vi.mocked(ipc.submitPracticeReview).mock.calls[0]?.[0];
    const second = vi.mocked(ipc.submitPracticeReview).mock.calls[1]?.[0];
    expect(second).toEqual(first);
    expect(first?.submissionId).toBe("f".repeat(32));
    expect(ipc.createSubmissionId).toHaveBeenCalledTimes(1);
  });
});
