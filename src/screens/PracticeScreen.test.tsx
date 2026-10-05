import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as cardsIpc from "../ipc/cards";
import * as commonIpc from "../ipc/common";
import type { DeckSummary } from "../ipc/decks";
import * as platformIpc from "../ipc/platform";
import * as practiceIpc from "../ipc/practice";
import * as schedulingIpc from "../ipc/scheduling";
import {
  resetPracticeExitGuardForTests,
  terminateActivePracticeSession,
} from "../controllers/practiceExitGuard";
import { deferred, makeSettings } from "../test/factories";
import { PracticeScreen } from "./PracticeScreen";

vi.mock("../editor/RichTextView", () => ({ RichTextView: () => <div>Card content</div> }));
vi.mock("../ipc/common", () => ({
  createMutationId: vi.fn(() => "e".repeat(32)),
  createOperationId: vi.fn(() => "0".repeat(32)),
  createSubmissionId: vi.fn(() => "f".repeat(32)),
  getErrorCode: (error: unknown) =>
    typeof error === "object" && error !== null && "code" in error
      ? String((error as { code: unknown }).code)
      : null,
  getErrorMessage: (error: unknown) =>
    error instanceof Error
      ? error.message
      : typeof error === "object" && error !== null && "message" in error
        ? String((error as { message: unknown }).message)
        : "Unexpected error",
}));
vi.mock("../ipc/practice", () => ({
  isInteractivePracticePresentation: (presentation: object) => "question" in presentation,
  completePracticeSession: vi.fn(),
  getPracticeCardWindow: vi.fn(),
  getPracticeWorkload: vi.fn(),
  markPracticeCardRevealed: vi.fn(),
  markPracticeCardViewed: vi.fn(),
  refreshPracticeAfterCardEdit: vi.fn(),
  resumePracticeSession: vi.fn(),
  startCardPractice: vi.fn(),
  startSelectedPractice: vi.fn(),
  submitPracticeReview: vi.fn(),
}));
vi.mock("../ipc/scheduling", () => ({ applyScheduleAction: vi.fn() }));
vi.mock("../ipc/platform", () => ({ listenToLifecycle: vi.fn() }));
vi.mock("../ipc/cards", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../ipc/cards")>()),
  getCardNote: vi.fn(),
}));
const editorMounted = vi.hoisted(() => vi.fn());
vi.mock("./CardEditorScreen", async () => {
  const { useEffect } = await vi.importActual<typeof import("react")>("react");
  return {
    CardEditorScreen: ({ onCancel }: { onCancel: () => void }) => {
      useEffect(() => {
        editorMounted();
      }, []);
      return <button type="button" onClick={onCancel}>Close editor</button>;
    },
  };
});

const deck: DeckSummary = {
  id: "a".repeat(32),
  name: "Dog facts",
  description: "",
  sourceNoteCount: 1,
  generatedCardCount: 1,
  studyableDueCount: 1,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};
const card = {
  presentation: {
    cardId: "b".repeat(32),
    noteId: "c".repeat(32),
    deckId: deck.id,
    direction: "forward",
    frontDoc: { type: "doc" as const, content: [] },
    backDoc: { type: "doc" as const, content: [] },
    frontText: "Do dogs have fur?",
    backText: "Yes",
    assets: [],
    createdAt: 1,
    updatedAt: 1,
    noteRevision: 1,
  },
  deckName: deck.name,
  dueAt: 1,
  scheduleState: 2,
  needsMorePractice: false,
};
const session: practiceIpc.PracticeSession = {
  id: "d".repeat(32),
  cardCount: 1,
  firstWindow: { startPosition: 0, cards: [card], hasMore: false },
  shuffle: false,
  mode: "scheduled",
  startedAt: 1,
};
const previews: practiceIpc.ReviewPreviewSet = {
  scheduleRevision: 4,
  previewedAt: 1,
  previews: [
    { outcome: "wrong", dueAt: 2, intervalMs: 1 },
    { outcome: "not_confident", dueAt: 3, intervalMs: 2 },
    { outcome: "somewhat_confident", dueAt: 4, intervalMs: 3 },
    { outcome: "very_confident", dueAt: 5, intervalMs: 4 },
  ],
};

function selectedLaunch(
  overrides: Partial<Extract<practiceIpc.SelectedPracticeRequest, object>> = {},
) {
  return {
    requestId: 1,
    kind: "selected" as const,
    deckIds: [deck.id],
    tagIds: [],
    shuffle: false,
    mode: "scheduled" as const,
    ...overrides,
  };
}

function renderPractice(
  overrides: Partial<React.ComponentProps<typeof PracticeScreen>> = {},
) {
  const props: React.ComponentProps<typeof PracticeScreen> = {
    decks: [deck],
    settings: makeSettings(),
    launchRequest: selectedLaunch(),
    onLaunchConsumed: vi.fn(),
    onCheckpointChange: vi.fn(),
    returnTarget: { section: "decks", deckId: deck.id },
    onViewAnalytics: vi.fn(),
    onEndEarly: vi.fn(),
    onChooseDecks: vi.fn(),
    showToast: vi.fn(),
    ...overrides,
  };
  render(<PracticeScreen {...props} />);
  return props;
}

async function reveal(): Promise<void> {
  fireEvent.click(await screen.findByRole("button", { name: "Show answer" }));
  await screen.findByRole("button", { name: /1 Hard/u });
}

/**
 * Waits until the first card's timer has started, not just until it renders.
 * React can run a commit's effects in a later task than the one in which
 * findByRole sees the new DOM, so timing tests wait for the shown-card effect
 * before they move the clock.
 */
async function findFirstCardShown(): Promise<void> {
  await screen.findByRole("button", { name: "Show answer" });
  await waitFor(() => expect(practiceIpc.markPracticeCardViewed).toHaveBeenCalledTimes(1));
}

describe("PracticeScreen internal workflow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetPracticeExitGuardForTests();
    vi.mocked(platformIpc.listenToLifecycle).mockResolvedValue(() => undefined);
    vi.mocked(practiceIpc.startSelectedPractice).mockResolvedValue(session);
    vi.mocked(practiceIpc.startCardPractice).mockResolvedValue({ ...session, mode: "custom" });
    vi.mocked(practiceIpc.markPracticeCardViewed).mockResolvedValue(undefined);
    vi.mocked(practiceIpc.markPracticeCardRevealed).mockResolvedValue(previews);
    vi.mocked(practiceIpc.completePracticeSession).mockResolvedValue(undefined);
    vi.mocked(practiceIpc.submitPracticeReview).mockResolvedValue({
      cardId: card.presentation.cardId,
      rating: 2,
      outcome: "not_confident",
      dueAt: 3,
      intervalMs: 2,
      completed: true,
      ratedCount: 1,
      correctCount: 1,
    });
    vi.mocked(schedulingIpc.applyScheduleAction).mockResolvedValue({
      affectedCount: 1,
      requestedCount: 1,
      changedCount: 1,
      unchangedCount: 0,
      rejectedCount: 0,
      action: "bury",
      resolvedDueAt: null,
    });
  });

  it("starts the exact selected scope without filtering members or replacing mode and shuffle", async () => {
    const missingId = "9".repeat(32);
    renderPractice({
      launchRequest: selectedLaunch({
        deckIds: [missingId, deck.id],
        tagIds: ["8".repeat(32)],
        shuffle: true,
        mode: "extra",
      }),
    });

    await waitFor(() =>
      expect(practiceIpc.startSelectedPractice).toHaveBeenCalledWith({
        operationId: "0".repeat(32),
        deckIds: [missingId, deck.id],
        tagIds: ["8".repeat(32)],
        shuffle: true,
        mode: "extra",
      }),
    );
  });

  it("preserves de-duplicated exact-card ordering and custom shuffle", async () => {
    const second = "7".repeat(32);
    renderPractice({
      launchRequest: {
        requestId: 2,
        kind: "cards",
        cardIds: [second, card.presentation.cardId, second],
        shuffle: true,
      },
    });
    await waitFor(() =>
      expect(practiceIpc.startCardPractice).toHaveBeenCalledWith({
        operationId: "0".repeat(32),
        cardIds: [second, card.presentation.cardId],
        shuffle: true,
      }),
    );
  });

  it("reports the active cursor and returns early to the exact origin", async () => {
    const onCheckpointChange = vi.fn();
    const onEndEarly = vi.fn();
    const target = { section: "decks" as const, deckId: deck.id };
    renderPractice({ onCheckpointChange, onEndEarly, returnTarget: target });
    await screen.findByRole("button", { name: "Show answer" });
    await waitFor(() =>
      expect(onCheckpointChange).toHaveBeenCalledWith({
        sessionId: session.id,
        position: 0,
        revealed: false,
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "End session" }));
    await waitFor(() => expect(onEndEarly).toHaveBeenCalledWith(target));
  });

  it("retries every failed start with the same canonical request and operation identity", async () => {
    vi.mocked(practiceIpc.startSelectedPractice)
      .mockRejectedValueOnce(Object.assign(new Error("Scope changed."), { code: "invalid_scope" }))
      .mockResolvedValueOnce(session);
    renderPractice();

    expect(await screen.findByText("Scope changed.")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await screen.findByRole("button", { name: "Show answer" });
    const calls = vi.mocked(practiceIpc.startSelectedPractice).mock.calls;
    expect(calls).toHaveLength(2);
    expect(calls[1]?.[0]).toEqual(calls[0]?.[0]);
    expect(commonIpc.createOperationId).toHaveBeenCalledTimes(1);
  });

  it("offers bounded Retry and Return when restore fails", async () => {
    vi.mocked(practiceIpc.resumePracticeSession).mockRejectedValue(new Error("Session missing"));
    const onEndEarly = vi.fn();
    renderPractice({
      launchRequest: null,
      restoreCheckpoint: {
        sessionId: session.id,
        position: 4,
        revealed: true,
        returnTarget: {
          section: "analytics",
          flashcardsWorkspace: null,
          deckId: null,
        },
      },
      returnTarget: { section: "analytics" },
      onEndEarly,
    });
    expect(await screen.findByText(/could not be restored: Session missing/u)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Retry" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Return" }));
    expect(onEndEarly).toHaveBeenCalledWith({ section: "analytics" });
  });

  it("submits the preview revision and repeats the exact original request", async () => {
    renderPractice();
    await reveal();
    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    await screen.findByRole("button", { name: "Practice this scope again" });
    expect(practiceIpc.submitPracticeReview).toHaveBeenCalledWith(
      expect.objectContaining({ scheduleRevision: 4 }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Practice this scope again" }));
    await waitFor(() => expect(practiceIpc.startSelectedPractice).toHaveBeenCalledTimes(2));
    const calls = vi.mocked(practiceIpc.startSelectedPractice).mock.calls;
    expect({ ...calls[1]?.[0], operationId: undefined }).toEqual({
      ...calls[0]?.[0],
      operationId: undefined,
    });
  });

  it("guards completion navigation and the exit path while repeat is starting", async () => {
    const repeatedStart = deferred<practiceIpc.PracticeSession>();
    const onViewAnalytics = vi.fn();
    const onChooseDecks = vi.fn();
    vi.mocked(practiceIpc.startSelectedPractice)
      .mockResolvedValueOnce(session)
      .mockImplementationOnce(async () => repeatedStart.promise);
    renderPractice({ onViewAnalytics, onChooseDecks });
    await reveal();
    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    const repeat = await screen.findByRole("button", { name: "Practice this scope again" });
    fireEvent.click(repeat);

    await waitFor(() => expect(repeat.hasAttribute("disabled")).toBe(true));
    const analytics = screen.getByRole("button", { name: "View analytics" });
    const chooseDecks = screen.getByRole("button", { name: "Choose decks" });
    expect(analytics.hasAttribute("disabled")).toBe(true);
    expect(chooseDecks.hasAttribute("disabled")).toBe(true);
    fireEvent.click(analytics);
    fireEvent.click(chooseDecks);
    await expect(terminateActivePracticeSession()).resolves.toBe(false);
    expect(onViewAnalytics).not.toHaveBeenCalled();
    expect(onChooseDecks).not.toHaveBeenCalled();

    repeatedStart.resolve(session);
    await screen.findByRole("button", { name: "Show answer" });
  });

  it("guards the registered exit path while the initial start is in flight", async () => {
    const initialStart = deferred<practiceIpc.PracticeSession>();
    vi.mocked(practiceIpc.startSelectedPractice).mockImplementation(
      async () => initialStart.promise,
    );
    renderPractice();
    await screen.findByText("Starting practice…");
    await expect(terminateActivePracticeSession()).resolves.toBe(false);
    initialStart.resolve(session);
    await screen.findByRole("button", { name: "Show answer" });
  });

  it("guards the registered exit path while a restore is in flight", async () => {
    const restored = deferred<practiceIpc.ResumedPracticeSession>();
    vi.mocked(practiceIpc.resumePracticeSession).mockImplementation(async () => restored.promise);
    renderPractice({
      launchRequest: null,
      restoreCheckpoint: {
        sessionId: session.id,
        position: 0,
        revealed: false,
        returnTarget: {
          section: "decks",
          flashcardsWorkspace: "decks",
          deckId: null,
        },
      },
    });
    await screen.findByText("Restoring practice…");
    await expect(terminateActivePracticeSession()).resolves.toBe(false);
    restored.resolve({
      session,
      resumePosition: 0,
      revealed: false,
      ratedCount: 0,
      correctCount: 0,
      completed: false,
    });
    await screen.findByRole("button", { name: "Show answer" });
  });

  it("leaves restored multi-deck completion analytics unscoped without a full launch request", async () => {
    const onViewAnalytics = vi.fn();
    vi.mocked(practiceIpc.resumePracticeSession).mockResolvedValue({
      session: { ...session, cardCount: 2 },
      resumePosition: 1,
      revealed: false,
      ratedCount: 1,
      correctCount: 1,
      completed: true,
    });
    renderPractice({
      launchRequest: null,
      restoreCheckpoint: {
        sessionId: session.id,
        position: 1,
        revealed: false,
        returnTarget: {
          section: "decks",
          flashcardsWorkspace: "decks",
          deckId: null,
        },
      },
      onViewAnalytics,
    });
    fireEvent.click(await screen.findByRole("button", { name: "View analytics" }));
    expect(onViewAnalytics).toHaveBeenCalledWith(session.id, []);
  });

  it("preserves unique observed deck filters for in-process exact-card completion", async () => {
    const secondDeckId = "7".repeat(32);
    const onViewAnalytics = vi.fn();
    vi.mocked(practiceIpc.startCardPractice).mockResolvedValue({
      ...session,
      cardCount: 3,
      mode: "custom",
      firstWindow: {
        startPosition: 0,
        hasMore: false,
        cards: [
          card,
          {
            ...card,
            presentation: {
              ...card.presentation,
              cardId: "6".repeat(32),
              deckId: secondDeckId,
            },
          },
          {
            ...card,
            presentation: {
              ...card.presentation,
              cardId: "5".repeat(32),
            },
          },
        ],
      },
    });
    renderPractice({
      launchRequest: {
        requestId: 2,
        kind: "cards",
        cardIds: [card.presentation.cardId, "6".repeat(32), "5".repeat(32)],
        shuffle: false,
      },
      onViewAnalytics,
    });
    await reveal();
    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    fireEvent.click(await screen.findByRole("button", { name: "View analytics" }));
    expect(onViewAnalytics).toHaveBeenCalledWith(session.id, [deck.id, secondDeckId]);
  });

  it("keeps an ambiguous review retry on the same submission payload", async () => {
    vi.mocked(practiceIpc.submitPracticeReview)
      .mockRejectedValueOnce(new Error("Response lost"))
      .mockResolvedValueOnce({
        cardId: card.presentation.cardId,
        rating: 2,
        outcome: "not_confident",
        dueAt: 3,
        intervalMs: 2,
        completed: true,
        ratedCount: 1,
        correctCount: 1,
      });
    renderPractice();
    await reveal();
    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    await screen.findByText(/safely retry the same review/u);
    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    await screen.findByRole("button", { name: "View analytics" });
    const calls = vi.mocked(practiceIpc.submitPracticeReview).mock.calls;
    expect(calls[1]?.[0]).toEqual(calls[0]?.[0]);
  });

  it("sends only visible study time and keeps the measured value on a retry", async () => {
    let now = 1_000;
    vi.spyOn(performance, "now").mockImplementation(() => now);
    let visibility: DocumentVisibilityState = "visible";
    vi.spyOn(document, "visibilityState", "get").mockImplementation(() => visibility);
    const setVisibility = (state: DocumentVisibilityState): void => {
      visibility = state;
      act(() => {
        document.dispatchEvent(new Event("visibilitychange"));
      });
    };
    vi.mocked(practiceIpc.submitPracticeReview)
      .mockRejectedValueOnce(new Error("Response lost"))
      .mockResolvedValueOnce({
        cardId: card.presentation.cardId,
        rating: 2,
        outcome: "not_confident",
        dueAt: 3,
        intervalMs: 2,
        completed: true,
        ratedCount: 1,
        correctCount: 1,
      });
    renderPractice();
    await findFirstCardShown();

    now += 4_000;
    setVisibility("hidden");
    now += 120_000;
    setVisibility("visible");
    now += 1_500;
    await reveal();
    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    await screen.findByText(/safely retry the same review/u);
    now += 9_000;
    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    await screen.findByRole("button", { name: "View analytics" });

    const calls = vi.mocked(practiceIpc.submitPracticeReview).mock.calls;
    expect(calls[0]?.[0].activeTimeMs).toBe(5_500);
    expect(calls[1]?.[0]).toEqual(calls[0]?.[0]);
  });

  it("does not count time in the card editor as study time", async () => {
    let now = 1_000;
    vi.spyOn(performance, "now").mockImplementation(() => now);
    vi.mocked(cardsIpc.getCardNote).mockResolvedValue({
      noteId: card.presentation.noteId,
      deckId: deck.id,
    } as unknown as cardsIpc.CardNote);
    renderPractice();
    await findFirstCardShown();

    now += 2_000;
    fireEvent.click(screen.getByRole("button", { name: "Edit card" }));
    const close = await screen.findByRole("button", { name: "Close editor" });
    // The editor's mount effect runs in the same pass as the screen's pause.
    await waitFor(() => expect(editorMounted).toHaveBeenCalledTimes(1));
    now += 300_000;
    fireEvent.click(close);
    await screen.findByRole("button", { name: "Show answer" });
    now += 1_000;
    await reveal();
    fireEvent.click(screen.getByRole("button", { name: /1 Hard/u }));
    await screen.findByRole("button", { name: "View analytics" });

    expect(practiceIpc.submitPracticeReview).toHaveBeenCalledWith(
      expect.objectContaining({ activeTimeMs: 3_000 }),
    );
  });
});
