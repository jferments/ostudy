import { useCallback, useEffect, useRef, useState } from "react";

import type { PracticeLaunchRequest } from "../App";
import { getRecentPracticeReviews } from "../analytics-ipc";
import {
  applyScheduleAction,
  completePracticeSession,
  createMutationId,
  createOperationId,
  createSubmissionId,
  getErrorCode,
  getErrorMessage,
  getPracticeCardWindow,
  getPracticeWorkload,
  markPracticeCardRevealed,
  markPracticeCardViewed,
  startCardPractice,
  startSelectedPractice,
  submitPracticeReview,
  type AppSettings,
  type Deck,
  type PracticeSession,
  type PracticeWorkloadOverview,
  type ReviewPreviewSet,
  type ScheduleAction,
  type SubmitReviewInput,
} from "../ipc";
import { ActivePractice } from "./practice/ActivePractice";
import {
  outcomeForScore,
  summarizeRecentDecks,
  type ConfidenceCounts,
  type ConfidenceScore,
  type RecentDeckSummary,
} from "./practice/confidence";
import { PracticeComplete } from "./practice/PracticeComplete";
import { PracticeSetup } from "./practice/PracticeSetup";

interface PracticeScreenProps {
  decks: Deck[];
  settings: AppSettings;
  launchRequest: PracticeLaunchRequest | null;
  onLaunchConsumed: () => void;
  onViewAnalytics: (sessionId: string, deckIds: string[]) => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

interface PendingStartOperation {
  key: string;
  operationId: string;
}

interface PendingReviewOperation {
  sessionId: string;
  cardId: string;
  score: ConfidenceScore;
  input: SubmitReviewInput;
}

const COMMITTED_START_RETRY_CODE = "committed_practice_session_unavailable";

export function PracticeScreen({
  decks,
  settings,
  launchRequest,
  onLaunchConsumed,
  onViewAnalytics,
  showToast,
}: PracticeScreenProps) {
  const [selectedDeckIds, setSelectedDeckIds] = useState<Set<string>>(new Set());
  const [temporaryCardIds, setTemporaryCardIds] = useState<string[]>([]);
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>([]);
  const [isLoadingWindow, setIsLoadingWindow] = useState(false);
  const [shuffle, setShuffle] = useState(settings.practiceShuffle);
  const [session, setSession] = useState<PracticeSession | null>(null);
  const [position, setPosition] = useState(0);
  const [revealed, setRevealed] = useState(false);
  const [previewSet, setPreviewSet] = useState<ReviewPreviewSet | null>(null);
  const [workload, setWorkload] = useState<PracticeWorkloadOverview | null>(null);
  const [isLoadingWorkload, setIsLoadingWorkload] = useState(false);
  const [isStarting, setIsStarting] = useState(false);
  const [isRevealing, setIsRevealing] = useState(false);
  const [isRating, setIsRating] = useState(false);
  const [isEnding, setIsEnding] = useState(false);
  const [isManaging, setIsManaging] = useState(false);
  const [completed, setCompleted] = useState(false);
  const [ratedCount, setRatedCount] = useState(0);
  const [confidenceCounts, setConfidenceCounts] = useState<ConfidenceCounts>([0, 0, 0, 0]);
  const [recentDecks, setRecentDecks] = useState<RecentDeckSummary[]>([]);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const startingRef = useRef(false);
  const revealingRef = useRef(false);
  const ratingRef = useRef(false);
  const endingRef = useRef(false);
  const managingRef = useRef(false);
  const questionShownAtRef = useRef(Date.now());
  const handledLaunchRequestRef = useRef<number | null>(null);
  const pendingStartRef = useRef<PendingStartOperation | null>(null);
  const pendingReviewRef = useRef<PendingReviewOperation | null>(null);
  const current = session?.cards[position] ?? null;

  const loadRecentPractice = useCallback(async (): Promise<void> => {
    try {
      setRecentDecks(summarizeRecentDecks(await getRecentPracticeReviews()));
    } catch (error: unknown) {
      console.error("Unable to load recent practice results:", getErrorMessage(error));
      setRecentDecks([]);
    }
  }, []);

  const loadWorkload = useCallback(async (deckIds: string[]): Promise<void> => {
    if (deckIds.length === 0) {
      setWorkload(null);
      return;
    }
    setIsLoadingWorkload(true);
    try {
      setWorkload(await getPracticeWorkload(deckIds, selectedTagIds));
    } catch (error: unknown) {
      setWorkload(null);
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsLoadingWorkload(false);
    }
  }, [selectedTagIds]);

  const beginSession = useCallback(
    async (
      deckIds: string[],
      requestedShuffle: boolean,
      mode: "scheduled" | "extra",
      tagIds: string[] = selectedTagIds,
    ): Promise<void> => {
      if (startingRef.current || deckIds.length === 0) return;
      const key = selectedStartKey(deckIds, tagIds, requestedShuffle, mode);
      const operationId = operationIdForStart(pendingStartRef, key);
      startingRef.current = true;
      setIsStarting(true);
      setTemporaryCardIds([]);
      setErrorMessage(null);
      try {
        initializeSession(
          await startSelectedPractice({
            operationId,
            deckIds,
            tagIds,
            shuffle: requestedShuffle,
            mode,
          }),
        );
      } catch (error: unknown) {
        setSession(null);
        handleStartFailure(error, key);
      } finally {
        startingRef.current = false;
        setIsStarting(false);
      }
    },
    [],
  );

  const beginCardSession = useCallback(
    async (cardIds: string[], requestedShuffle: boolean): Promise<void> => {
      if (startingRef.current || cardIds.length === 0) return;
      const key = cardStartKey(cardIds, requestedShuffle);
      const operationId = operationIdForStart(pendingStartRef, key);
      startingRef.current = true;
      setIsStarting(true);
      setTemporaryCardIds(cardIds);
      setErrorMessage(null);
      try {
        initializeSession(
          await startCardPractice({ operationId, cardIds, shuffle: requestedShuffle }),
        );
      } catch (error: unknown) {
        setSession(null);
        handleStartFailure(error, key);
      } finally {
        startingRef.current = false;
        setIsStarting(false);
      }
    },
    [],
  );

  function handleStartFailure(error: unknown, key: string): void {
    const code = getErrorCode(error);
    if (code !== null && code !== COMMITTED_START_RETRY_CODE) {
      if (pendingStartRef.current?.key === key) pendingStartRef.current = null;
      setErrorMessage(getErrorMessage(error));
      return;
    }
    setErrorMessage(
      `${getErrorMessage(error)} Retry the same start action to safely resume it without creating a duplicate session.`,
    );
  }

  function initializeSession(started: PracticeSession): void {
    pendingStartRef.current = null;
    pendingReviewRef.current = null;
    setSession(started);
    setPosition(0);
    setRevealed(false);
    setPreviewSet(null);
    setCompleted(false);
    setRatedCount(0);
    setConfidenceCounts([0, 0, 0, 0]);
    questionShownAtRef.current = Date.now();
  }

  useEffect(() => {
    void loadRecentPractice();
  }, [loadRecentPractice]);

  useEffect(() => {
    void loadWorkload([...selectedDeckIds]);
  }, [loadWorkload, selectedDeckIds]);

  useEffect(() => {
    if (!launchRequest || handledLaunchRequestRef.current === launchRequest.requestId) return;
    handledLaunchRequestRef.current = launchRequest.requestId;
    pendingStartRef.current = null;
    pendingReviewRef.current = null;
    setSession(null);
    setCompleted(false);
    setErrorMessage(null);
    setSelectedTagIds(launchRequest.tagIds ?? []);
    if (launchRequest.cardIds.length > 0) {
      setSelectedDeckIds(new Set());
      setTemporaryCardIds(launchRequest.cardIds);
      onLaunchConsumed();
      if (launchRequest.autoStart) {
        void beginCardSession(launchRequest.cardIds, settings.practiceShuffle);
      }
      return;
    }
    const eligible = launchRequest.deckIds.filter((deckId) =>
      decks.some((deck) => deck.id === deckId && deck.generatedCardCount > 0),
    );
    setTemporaryCardIds([]);
    setSelectedDeckIds(new Set(eligible));
    onLaunchConsumed();
    if (launchRequest.autoStart && eligible.length > 0) {
      void beginSession(eligible, settings.practiceShuffle, "scheduled", launchRequest.tagIds ?? []);
    }
  }, [
    beginCardSession,
    beginSession,
    decks,
    launchRequest,
    onLaunchConsumed,
    settings.practiceShuffle,
  ]);

  useEffect(() => {
    if (!session || !current || completed) return;
    questionShownAtRef.current = Date.now();
    void markPracticeCardViewed(session.id, current.card.id).catch((error: unknown) => {
      showToast("error", getErrorMessage(error));
    });
  }, [completed, current, session, showToast]);

  async function loadNextWindow(): Promise<void> {
    if (!session || !session.hasMore || isLoadingWindow) return;
    setIsLoadingWindow(true);
    try {
      const window = await getPracticeCardWindow(session.id, session.cards.length, 50);
      setSession((current) => current && current.id === session.id
        ? { ...current, cards: [...current.cards, ...window.cards], hasMore: window.hasMore }
        : current);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsLoadingWindow(false);
    }
  }

  useEffect(() => {
    if (session?.hasMore && position >= Math.max(0, session.cards.length - 5)) {
      void loadNextWindow();
    }
  }, [position, session?.id, session?.cards.length, session?.hasMore]);

  async function revealAnswer(): Promise<void> {
    if (!session || !current || revealed || revealingRef.current) return;
    if (pendingReviewRef.current) {
      setErrorMessage("Retry the pending rating before revealing another answer.");
      return;
    }
    revealingRef.current = true;
    setIsRevealing(true);
    setErrorMessage(null);
    try {
      setPreviewSet(await markPracticeCardRevealed(session.id, current.card.id));
      setRevealed(true);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      revealingRef.current = false;
      setIsRevealing(false);
    }
  }

  async function submitScore(score: ConfidenceScore): Promise<void> {
    if (!session || !current || !revealed || !previewSet || ratingRef.current) return;
    const pending = pendingReviewRef.current;
    if (
      pending &&
      (pending.sessionId !== session.id ||
        pending.cardId !== current.card.id ||
        pending.score !== score)
    ) {
      setErrorMessage(
        `The result of rating ${pending.score} is still unknown. Choose ${pending.score} again to safely retry that exact review.`,
      );
      return;
    }

    const input: SubmitReviewInput =
      pending?.input ??
      {
        sessionId: session.id,
        cardId: current.card.id,
        submissionId: createSubmissionId(),
        outcome: outcomeForScore(score),
        responseTimeMs: Math.max(
          0,
          Math.min(86_400_000, Date.now() - questionShownAtRef.current),
        ),
        scheduleRevision: previewSet.scheduleRevision,
      };
    if (!pending) {
      pendingReviewRef.current = {
        sessionId: session.id,
        cardId: current.card.id,
        score,
        input,
      };
    }

    ratingRef.current = true;
    setIsRating(true);
    setErrorMessage(null);
    try {
      const result = await submitPracticeReview(input);
      pendingReviewRef.current = null;
      setRatedCount(result.ratedCount);
      setConfidenceCounts((counts) => {
        const next: ConfidenceCounts = [...counts];
        next[score] += 1;
        return next;
      });
      advanceOrComplete(result.completed);
    } catch (error: unknown) {
      const code = getErrorCode(error);
      if (code === null) {
        setErrorMessage(
          `${getErrorMessage(error)} Choose ${score} again to safely retry the same review without recording it twice.`,
        );
      } else {
        pendingReviewRef.current = null;
        setErrorMessage(getErrorMessage(error));
        if (code === "schedule_changed" || code === "idempotency_conflict") {
          setRevealed(false);
          setPreviewSet(null);
        }
      }
    } finally {
      ratingRef.current = false;
      setIsRating(false);
    }
  }

  async function manageCurrent(action: ScheduleAction, dueAt?: number): Promise<void> {
    if (!session || !current || managingRef.current) return;
    if (pendingReviewRef.current) {
      setErrorMessage("Retry the pending rating before changing this card's schedule.");
      return;
    }
    if (
      action === "reset" &&
      !window.confirm(
        "Reset this card to New? Its review history will remain, but its current FSRS schedule will restart.",
      )
    ) {
      return;
    }
    managingRef.current = true;
    setIsManaging(true);
    setErrorMessage(null);
    try {
      await applyScheduleAction({
        mutationId: createMutationId(),
        cardIds: [current.card.id],
        action,
        dueAt,
        sessionId: session.id,
      });
      if (action === "bury" || action === "suspend") {
        showToast(
          "info",
          action === "bury" ? "Card buried until tomorrow." : "Card suspended.",
        );
        advanceOrComplete(position + 1 >= session.cards.length);
      } else {
        showToast(
          "success",
          action === "reset" ? "Card schedule reset." : "Card rescheduled.",
        );
        setRevealed(false);
        setPreviewSet(null);
        questionShownAtRef.current = Date.now();
      }
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      managingRef.current = false;
      setIsManaging(false);
    }
  }

  function advanceOrComplete(backendCompleted: boolean): void {
    pendingReviewRef.current = null;
    if (!session || backendCompleted || position + 1 >= session.cards.length) {
      setCompleted(true);
      void loadRecentPractice();
      return;
    }
    setPosition((value) => value + 1);
    setRevealed(false);
    setPreviewSet(null);
  }

  async function endSession(): Promise<void> {
    if (!session || endingRef.current) return;
    if (pendingReviewRef.current) {
      setErrorMessage("Retry the pending rating before ending this session.");
      return;
    }
    endingRef.current = true;
    setIsEnding(true);
    setErrorMessage(null);
    try {
      await completePracticeSession(session.id);
      resetToSetup();
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      endingRef.current = false;
      setIsEnding(false);
    }
  }

  function resetToSetup(): void {
    pendingStartRef.current = null;
    pendingReviewRef.current = null;
    setSession(null);
    setPosition(0);
    setRevealed(false);
    setPreviewSet(null);
    setCompleted(false);
    setRatedCount(0);
    setConfidenceCounts([0, 0, 0, 0]);
    setTemporaryCardIds([]);
    setErrorMessage(null);
    void loadRecentPractice();
    void loadWorkload([...selectedDeckIds]);
  }

  if (session && completed) {
    const sessionDeckIds = [...new Set(session.cards.map((card) => card.card.deckId))];
    return (
      <PracticeComplete
        ratedCount={ratedCount}
        confidenceCounts={confidenceCounts}
        isStarting={isStarting}
        errorMessage={errorMessage}
        onViewAnalytics={() => onViewAnalytics(session.id, sessionDeckIds)}
        onChooseDecks={resetToSetup}
        onRestart={() => {
          if (temporaryCardIds.length > 0) {
            void beginCardSession(temporaryCardIds, shuffle);
          } else {
            void beginSession(
              [...selectedDeckIds],
              shuffle,
              session.mode === "extra" ? "extra" : "scheduled",
            );
          }
        }}
      />
    );
  }

  if (session && current) {
    return (
      <ActivePractice
        session={session}
        position={position}
        revealed={revealed}
        ratedCount={ratedCount}
        previewSet={previewSet}
        isRevealing={isRevealing}
        isRating={isRating}
        isEnding={isEnding}
        isManaging={isManaging}
        errorMessage={errorMessage}
        onReveal={() => void revealAnswer()}
        onScore={(score) => void submitScore(score)}
        onScheduleAction={(action, dueAt) => void manageCurrent(action, dueAt)}
        onEnd={() => void endSession()}
        onLinkError={setErrorMessage}
      />
    );
  }

  return (
    <div className="screen practice-screen">
      <header className="screen-header compact-screen-header">
        <div>
          <h1>Practice</h1>
          <p>
            Scheduled sessions protect due reviews and daily workload limits. Extra study
            is always an explicit choice.
          </p>
        </div>
      </header>
      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      <PracticeSetup
        decks={decks}
        selectedDeckIds={selectedDeckIds}
        shuffle={shuffle}
        isStarting={isStarting}
        isLoadingWorkload={isLoadingWorkload}
        workload={workload}
        recentDecks={recentDecks}
        onSelectedDeckIdsChange={setSelectedDeckIds}
        onShuffleChange={setShuffle}
        onStart={(deckIds, requestedShuffle, mode) =>
          void beginSession(deckIds, requestedShuffle, mode)
        }
      />
    </div>
  );
}

function operationIdForStart(
  pendingRef: React.MutableRefObject<PendingStartOperation | null>,
  key: string,
): string {
  if (pendingRef.current?.key === key) return pendingRef.current.operationId;
  const operationId = createOperationId();
  pendingRef.current = { key, operationId };
  return operationId;
}

function selectedStartKey(
  deckIds: string[],
  shuffle: boolean,
  mode: "scheduled" | "extra",
): string {
  const canonicalDeckIds = [...new Set(deckIds)].sort();
  return JSON.stringify({ kind: "selected", deckIds: canonicalDeckIds, shuffle, mode });
}

function cardStartKey(cardIds: string[], shuffle: boolean): string {
  const seen = new Set<string>();
  const canonicalCardIds = cardIds.filter((cardId) => {
    if (seen.has(cardId)) return false;
    seen.add(cardId);
    return true;
  });
  return JSON.stringify({ kind: "cards", cardIds: canonicalCardIds, shuffle });
}
