import { useCallback, useEffect, useRef, useState } from "react";

import type { PracticeLaunchRequest } from "../App";
import { PracticeController } from "../controllers/practiceController";
import {
  createMutationId,
  createOperationId,
  createSubmissionId,
  getErrorCode,
  getErrorMessage,
} from "../ipc/common";
import {
  type DeckSummary,
} from "../ipc/decks";
import {
  completePracticeSession,
  markPracticeCardRevealed,
  markPracticeCardViewed,
  startCardPractice,
  startSelectedPractice,
  submitPracticeReview,
  type PracticeCard,
  type PracticeSession,
  type PracticeWorkloadOverview,
  type ReviewPreviewSet,
  type SubmitReviewInput,
} from "../ipc/practice";
import {
  applyScheduleAction,
  type RescheduleIntent,
  type ScheduleAction,
} from "../ipc/scheduling";
import {
  type AppSettings,
} from "../ipc/settings";
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
  decks: DeckSummary[];
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
const PRACTICE_WINDOW_SIZE = 50;

export function PracticeScreen({
  decks,
  settings,
  launchRequest,
  onLaunchConsumed,
  onViewAnalytics,
  showToast,
}: PracticeScreenProps) {
  const activeDecks = decks.filter((deck) => deck.archivedAt === null);
  const [selectedDeckIds, setSelectedDeckIds] = useState<Set<string>>(new Set());
  const [selectedTagIds, setSelectedTagIds] = useState<Set<string>>(new Set());
  const [temporaryCardIds, setTemporaryCardIds] = useState<string[]>([]);
  const [shuffle, setShuffle] = useState(settings.practiceShuffle);
  const [session, setSession] = useState<PracticeSession | null>(null);
  const [sessionCards, setSessionCards] = useState<PracticeCard[]>([]);
  const [position, setPosition] = useState(0);
  const [revealed, setRevealed] = useState(false);
  const [previewSet, setPreviewSet] = useState<ReviewPreviewSet | null>(null);
  const [workload, setWorkload] = useState<PracticeWorkloadOverview | null>(null);
  const [isLoadingWorkload, setIsLoadingWorkload] = useState(false);
  const [isLoadingWindow, setIsLoadingWindow] = useState(false);
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
  const loadingWindowRef = useRef(false);
  const revealingRef = useRef(false);
  const ratingRef = useRef(false);
  const endingRef = useRef(false);
  const managingRef = useRef(false);
  const questionShownAtRef = useRef(Date.now());
  const handledLaunchRequestRef = useRef<number | null>(null);
  const pendingStartRef = useRef<PendingStartOperation | null>(null);
  const pendingReviewRef = useRef<PendingReviewOperation | null>(null);
  const controllerRef = useRef<PracticeController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new PracticeController();
  const controller = controllerRef.current;
  const current = sessionCards[position] ?? null;

  useEffect(() => () => controller.invalidate(), [controller]);

  const loadRecentPractice = useCallback(async (): Promise<void> => {
    try {
      const reviews = await controller.recentReviews();
      if (reviews) setRecentDecks(summarizeRecentDecks(reviews));
    } catch (error: unknown) {
      console.error("Unable to load recent practice results:", getErrorMessage(error));
      setRecentDecks([]);
    }
  }, [controller]);

  const loadWorkload = useCallback(async (deckIds: string[], tagIds: string[]): Promise<void> => {
    if (deckIds.length === 0 && tagIds.length === 0) {
      controller.invalidateWorkload();
      setWorkload(null);
      setIsLoadingWorkload(false);
      return;
    }
    setIsLoadingWorkload(true);
    try {
      const loaded = await controller.workload(deckIds, tagIds);
      if (!loaded) return;
      setWorkload(loaded);
      setErrorMessage(null);
      setIsLoadingWorkload(false);
    } catch (error: unknown) {
      setWorkload(null);
      setErrorMessage(getErrorMessage(error));
      setIsLoadingWorkload(false);
    }
  }, [controller]);

  const beginSession = useCallback(
    async (
      deckIds: string[],
      tagIds: string[],
      requestedShuffle: boolean,
      mode: "scheduled" | "extra",
    ): Promise<void> => {
      if (startingRef.current || (deckIds.length === 0 && tagIds.length === 0)) return;
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
        setSessionCards([]);
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
        setSessionCards([]);
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
    setSessionCards(started.firstWindow.cards);
    setPosition(0);
    setRevealed(false);
    setPreviewSet(null);
    setCompleted(false);
    setRatedCount(0);
    setConfidenceCounts([0, 0, 0, 0]);
    questionShownAtRef.current = Date.now();
  }

  const loadNextWindow = useCallback(async (): Promise<number> => {
    if (!session || loadingWindowRef.current || sessionCards.length >= session.cardCount) {
      return sessionCards.length;
    }
    loadingWindowRef.current = true;
    setIsLoadingWindow(true);
    try {
      const window = await controller.window({
        sessionId: session.id,
        startPosition: sessionCards.length,
        limit: PRACTICE_WINDOW_SIZE,
      });
      if (!window) return sessionCards.length;
      setSessionCards((currentCards) => [...currentCards, ...window.cards]);
      setErrorMessage(null);
      return sessionCards.length + window.cards.length;
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
      return sessionCards.length;
    } finally {
      loadingWindowRef.current = false;
      setIsLoadingWindow(false);
    }
  }, [controller, session, sessionCards.length]);

  useEffect(() => {
    void loadRecentPractice();
  }, [loadRecentPractice]);

  useEffect(() => {
    if (session || completed) return;
    void loadWorkload([...selectedDeckIds], [...selectedTagIds]);
  }, [completed, loadWorkload, selectedDeckIds, selectedTagIds, session]);

  useEffect(() => {
    if (!launchRequest || handledLaunchRequestRef.current === launchRequest.requestId) return;
    handledLaunchRequestRef.current = launchRequest.requestId;
    setSelectedDeckIds(new Set(launchRequest.deckIds));
    setSelectedTagIds(new Set(launchRequest.tagIds));
    setTemporaryCardIds(launchRequest.cardIds);
    setCompleted(false);
    setSession(null);
    setSessionCards([]);
    setPosition(0);
    setRevealed(false);
    setPreviewSet(null);
    setErrorMessage(null);
    onLaunchConsumed();
    if (!launchRequest.autoStart) return;
    if (launchRequest.cardIds.length > 0) {
      void beginCardSession(launchRequest.cardIds, shuffle);
      return;
    }
    void beginSession(launchRequest.deckIds, launchRequest.tagIds, shuffle, "scheduled");
  }, [beginCardSession, beginSession, launchRequest, onLaunchConsumed, shuffle]);

  const activeCardId = current?.presentation.cardId ?? null;
  useEffect(() => {
    if (!session || !activeCardId || completed) return;
    questionShownAtRef.current = Date.now();
    void markPracticeCardViewed(session.id, activeCardId).catch((error: unknown) => {
      console.error("Unable to persist question view:", getErrorMessage(error));
    });
  }, [activeCardId, completed, session]);

  useEffect(() => {
    if (!session || sessionCards.length >= session.cardCount || sessionCards.length === 0) return;
    if (position < sessionCards.length - 5) return;
    void loadNextWindow();
  }, [loadNextWindow, position, session, sessionCards.length]);

  async function reveal(): Promise<void> {
    if (!session || !current || revealed || isRevealing || revealingRef.current) return;
    revealingRef.current = true;
    setIsRevealing(true);
    setErrorMessage(null);
    try {
      setPreviewSet(await markPracticeCardRevealed(session.id, current.presentation.cardId));
      setRevealed(true);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      revealingRef.current = false;
      setIsRevealing(false);
    }
  }

  async function rate(score: ConfidenceScore): Promise<void> {
    if (!session || !current || !revealed || !previewSet || isRating || ratingRef.current) return;
    ratingRef.current = true;
    setIsRating(true);
    setErrorMessage(null);
    const responseTimeMs = Math.max(0, Date.now() - questionShownAtRef.current);
    const pending = operationForReview(
      pendingReviewRef,
      session.id,
      current.presentation.cardId,
      score,
      responseTimeMs,
      previewSet.scheduleRevision,
    );
    try {
      const result = await submitPracticeReview(pending.input);
      pendingReviewRef.current = null;
      const nextCounts = [...confidenceCounts] as ConfidenceCounts;
      nextCounts[score] += 1;
      setConfidenceCounts(nextCounts);
      setRatedCount((value) => value + 1);
      setPreviewSet(null);
      setRevealed(false);
      if (result.completed) {
        setCompleted(true);
        setSession(null);
        setSessionCards([]);
        setPosition(0);
        void loadRecentPractice();
        return;
      }
      const nextPosition = position + 1;
      if (nextPosition >= sessionCards.length && sessionCards.length < session.cardCount) {
        await loadNextWindow();
      }
      setPosition(nextPosition);
    } catch (error: unknown) {
      setErrorMessage(
        `${getErrorMessage(error)} Retry the same rating to safely resume this review if its result was lost.`,
      );
    } finally {
      ratingRef.current = false;
      setIsRating(false);
    }
  }

  async function manageCurrent(action: ScheduleAction, intent?: RescheduleIntent): Promise<void> {
    if (!session || !current || isManaging || managingRef.current) return;
    managingRef.current = true;
    setIsManaging(true);
    setErrorMessage(null);
    const cardId = current.presentation.cardId;
    const mutationId = createMutationId();
    try {
      await applyScheduleAction({
        mutationId,
        cardIds: [cardId],
        action,
        rescheduleIntent: intent,
        sessionId: session.id,
      });
      showToast("success", `The current card was ${scheduleActionPastTense(action)}.`);
      setPreviewSet(null);
      setRevealed(false);
      if (position + 1 >= session.cardCount) {
        await finishSession();
      } else {
        setPosition((value) => value + 1);
      }
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      managingRef.current = false;
      setIsManaging(false);
    }
  }

  async function finishSession(): Promise<void> {
    if (!session || isEnding || endingRef.current) return;
    endingRef.current = true;
    setIsEnding(true);
    try {
      await completePracticeSession(session.id);
      setCompleted(true);
      setSession(null);
      setSessionCards([]);
      setPosition(0);
      setPreviewSet(null);
      setRevealed(false);
      void loadRecentPractice();
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      endingRef.current = false;
      setIsEnding(false);
    }
  }

  if (completed) {
    return (
      <PracticeComplete
        ratedCount={ratedCount}
        confidenceCounts={confidenceCounts}
        recentDecks={recentDecks}
        onStudyAgain={() => {
          setCompleted(false);
          setSelectedDeckIds(new Set());
          setSelectedTagIds(new Set());
          setTemporaryCardIds([]);
        }}
        onViewAnalytics={onViewAnalytics}
      />
    );
  }

  if (session && current) {
    return (
      <ActivePractice
        session={session}
        current={current}
        position={position}
        revealed={revealed}
        previewSet={previewSet}
        isRevealing={isRevealing}
        isRating={isRating}
        isManaging={isManaging}
        isEnding={isEnding}
        isLoadingWindow={isLoadingWindow}
        errorMessage={errorMessage}
        onReveal={() => void reveal()}
        onRate={(score) => void rate(score)}
        onManage={(action, intent) => void manageCurrent(action, intent)}
        onFinish={() => void finishSession()}
      />
    );
  }

  return (
    <PracticeSetup
      decks={activeDecks}
      selectedDeckIds={selectedDeckIds}
      selectedTagIds={selectedTagIds}
      temporaryCardIds={temporaryCardIds}
      shuffle={shuffle}
      workload={workload}
      isLoadingWorkload={isLoadingWorkload}
      isStarting={isStarting}
      errorMessage={errorMessage}
      recentDecks={recentDecks}
      onSelectedDeckIdsChange={setSelectedDeckIds}
      onSelectedTagIdsChange={setSelectedTagIds}
      onShuffleChange={setShuffle}
      onStartScheduled={() =>
        void beginSession([...selectedDeckIds], [...selectedTagIds], shuffle, "scheduled")
      }
      onStartExtra={() =>
        void beginSession([...selectedDeckIds], [...selectedTagIds], shuffle, "extra")
      }
      onStartCards={() => void beginCardSession(temporaryCardIds, shuffle)}
      onViewAnalytics={onViewAnalytics}
    />
  );
}

function selectedStartKey(
  deckIds: string[],
  tagIds: string[],
  shuffle: boolean,
  mode: "scheduled" | "extra",
): string {
  return JSON.stringify({ kind: "selected", deckIds: [...deckIds].sort(), tagIds: [...tagIds].sort(), shuffle, mode });
}

function cardStartKey(cardIds: string[], shuffle: boolean): string {
  return JSON.stringify({ kind: "cards", cardIds: [...cardIds].sort(), shuffle });
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

function operationForReview(
  pendingRef: React.MutableRefObject<PendingReviewOperation | null>,
  sessionId: string,
  cardId: string,
  score: ConfidenceScore,
  responseTimeMs: number,
  scheduleRevision: number,
): PendingReviewOperation {
  const current = pendingRef.current;
  if (
    current &&
    current.sessionId === sessionId &&
    current.cardId === cardId &&
    current.score === score
  ) {
    return current;
  }
  const input: SubmitReviewInput = {
    sessionId,
    cardId,
    submissionId: createSubmissionId(),
    outcome: outcomeForScore(score),
    responseTimeMs,
    scheduleRevision,
  };
  const pending = { sessionId, cardId, score, input };
  pendingRef.current = pending;
  return pending;
}

function scheduleActionPastTense(action: ScheduleAction): string {
  switch (action) {
    case "suspend": return "suspended";
    case "resume": return "resumed";
    case "bury": return "buried";
    case "unbury": return "unburied";
    case "reset": return "reset";
    case "reschedule": return "rescheduled";
  }
}
