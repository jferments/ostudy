import { useCallback, useEffect, useRef, useState } from "react";

import type { PracticeLaunchRequest } from "../App";
import {
  applyScheduleAction,
  completePracticeSession,
  createMutationId,
  createSubmissionId,
  getAnalytics,
  getErrorMessage,
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
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

export function PracticeScreen({
  decks,
  settings,
  launchRequest,
  onLaunchConsumed,
  showToast,
}: PracticeScreenProps) {
  const [selectedDeckIds, setSelectedDeckIds] = useState<Set<string>>(new Set());
  const [temporaryCardIds, setTemporaryCardIds] = useState<string[]>([]);
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
  const current = session?.cards[position] ?? null;

  const loadRecentPractice = useCallback(async (): Promise<void> => {
    try {
      const analytics = await getAnalytics();
      setRecentDecks(summarizeRecentDecks(analytics.recentReviews));
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
      setWorkload(await getPracticeWorkload(deckIds));
    } catch (error: unknown) {
      setWorkload(null);
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsLoadingWorkload(false);
    }
  }, []);

  const beginSession = useCallback(
    async (deckIds: string[], requestedShuffle: boolean, mode: "scheduled" | "extra"): Promise<void> => {
      if (startingRef.current || deckIds.length === 0) return;
      startingRef.current = true;
      setIsStarting(true);
      setTemporaryCardIds([]);
      setErrorMessage(null);
      try {
        initializeSession(await startSelectedPractice({ deckIds, shuffle: requestedShuffle, mode }));
      } catch (error: unknown) {
        setSession(null);
        setErrorMessage(getErrorMessage(error));
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
      startingRef.current = true;
      setIsStarting(true);
      setTemporaryCardIds(cardIds);
      setErrorMessage(null);
      try {
        initializeSession(await startCardPractice({ cardIds, shuffle: requestedShuffle }));
      } catch (error: unknown) {
        setSession(null);
        setErrorMessage(getErrorMessage(error));
      } finally {
        startingRef.current = false;
        setIsStarting(false);
      }
    },
    [],
  );

  function initializeSession(started: PracticeSession): void {
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
    setSession(null);
    setCompleted(false);
    setErrorMessage(null);
    if (launchRequest.cardIds.length > 0) {
      setSelectedDeckIds(new Set());
      setTemporaryCardIds(launchRequest.cardIds);
      onLaunchConsumed();
      if (launchRequest.autoStart) void beginCardSession(launchRequest.cardIds, settings.practiceShuffle);
      return;
    }
    const eligible = launchRequest.deckIds.filter((deckId) =>
      decks.some((deck) => deck.id === deckId && deck.cardCount > 0),
    );
    setTemporaryCardIds([]);
    setSelectedDeckIds(new Set(eligible));
    onLaunchConsumed();
    if (launchRequest.autoStart && eligible.length > 0) {
      void beginSession(eligible, settings.practiceShuffle, "scheduled");
    }
  }, [beginCardSession, beginSession, decks, launchRequest, onLaunchConsumed, settings.practiceShuffle]);

  useEffect(() => {
    if (!session || !current || completed) return;
    questionShownAtRef.current = Date.now();
    void markPracticeCardViewed(session.id, current.card.id).catch((error: unknown) => {
      showToast("error", getErrorMessage(error));
    });
  }, [completed, current, session, showToast]);

  async function revealAnswer(): Promise<void> {
    if (!session || !current || revealed || revealingRef.current) return;
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
    ratingRef.current = true;
    setIsRating(true);
    setErrorMessage(null);
    try {
      const result = await submitPracticeReview({
        sessionId: session.id,
        cardId: current.card.id,
        submissionId: createSubmissionId(),
        outcome: outcomeForScore(score),
        responseTimeMs: Math.max(0, Math.min(86_400_000, Date.now() - questionShownAtRef.current)),
        scheduleRevision: previewSet.scheduleRevision,
      });
      setRatedCount(result.ratedCount);
      setConfidenceCounts((counts) => {
        const next: ConfidenceCounts = [...counts];
        next[score] += 1;
        return next;
      });
      advanceOrComplete(result.completed);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      ratingRef.current = false;
      setIsRating(false);
    }
  }

  async function manageCurrent(action: ScheduleAction, dueAt?: number): Promise<void> {
    if (!session || !current || managingRef.current) return;
    if (action === "reset" && !window.confirm("Reset this card to New? Its review history will remain, but its current FSRS schedule will restart.")) return;
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
        showToast("info", action === "bury" ? "Card buried until tomorrow." : "Card suspended.");
        advanceOrComplete(position + 1 >= session.cards.length);
      } else {
        showToast("success", action === "reset" ? "Card schedule reset." : "Card rescheduled.");
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
    return (
      <PracticeComplete
        ratedCount={ratedCount}
        confidenceCounts={confidenceCounts}
        isStarting={isStarting}
        errorMessage={errorMessage}
        onChooseDecks={resetToSetup}
        onRestart={() => {
          if (temporaryCardIds.length > 0) void beginCardSession(temporaryCardIds, shuffle);
          else void beginSession([...selectedDeckIds], shuffle, session.mode === "extra" ? "extra" : "scheduled");
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
          <p>Scheduled sessions protect due reviews and daily workload limits. Extra study is always an explicit choice.</p>
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
        onStart={(deckIds, requestedShuffle, mode) => void beginSession(deckIds, requestedShuffle, mode)}
      />
    </div>
  );
}