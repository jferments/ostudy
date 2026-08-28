import { useCallback, useEffect, useRef, useState } from "react";

import type { PracticeLaunchRequest, PracticeReturnTarget } from "../App";
import { PracticeController } from "../controllers/practiceController";
import { registerPracticeExitHandler } from "../controllers/practiceExitGuard";
import { getCardNote, type CardNote } from "../ipc/cards";
import {
  createMutationId,
  createOperationId,
  createSubmissionId,
  getErrorCode,
  getErrorMessage,
} from "../ipc/common";
import { type DeckSummary } from "../ipc/decks";
import {
  type PracticeCard,
  type PracticeSession,
  type PracticeWorkloadOverview,
  type ReviewPreviewSet,
  type SubmitReviewInput,
} from "../ipc/practice";
import { type RescheduleIntent, type ScheduleAction } from "../ipc/scheduling";
import { type AppSettings } from "../ipc/settings";
import {
  type PracticeCheckpoint,
  type PracticeCursorCheckpoint,
} from "../ipc/uiCheckpoint";
import { useExperience } from "../ui/feedback/ExperienceProvider";
import { Icon } from "../ui/icons/Icon";
import { useConfirm } from "../ui/overlays/overlayContext";
import { CardEditorScreen } from "./CardEditorScreen";
import { ActivePractice } from "./practice/ActivePractice";
import {
  outcomeForScore,
  summarizeRecentDecks,
  type ConfidenceCounts,
  type ConfidenceScore,
  type RecentDeckSummary,
} from "./practice/confidence";
import { PracticeComplete } from "./practice/PracticeComplete";
import { PracticeSetup, type PracticeWorkloadStatus } from "./practice/PracticeSetup";
import { StudyGuideLibrarySection } from "./StudyGuideLibrarySection";
import { ownedClassNames, sharedClassMaps } from "../ui/styles/classOwnership";
import ownershipStyles from "./practice/Practice.module.css";
import { PerformanceProfiler } from "../testing/PerformanceProfiler";

const ownedStyleMaps = [...sharedClassMaps, ownershipStyles] as const;


interface PracticeScreenProps {
  decks: DeckSummary[];
  settings: AppSettings;
  launchRequest: PracticeLaunchRequest | null;
  onLaunchConsumed: () => void;
  restoreCheckpoint?: PracticeCheckpoint | null;
  onRestoreConsumed?: () => void;
  onCheckpointChange?: (checkpoint: PracticeCursorCheckpoint | null) => void;
  returnTarget: PracticeReturnTarget | null;
  onClearReturnTarget: () => void;
  onViewAnalytics: (sessionId: string, deckIds: string[]) => void;
  onEndEarly: (target: PracticeReturnTarget) => void;
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
const NOOP = (): void => undefined;

export function PracticeScreen({
  decks,
  settings,
  launchRequest,
  onLaunchConsumed,
  restoreCheckpoint = null,
  onRestoreConsumed = NOOP,
  onCheckpointChange = NOOP,
  returnTarget,
  onClearReturnTarget,
  onViewAnalytics,
  onEndEarly,
  showToast,
}: PracticeScreenProps) {
  const activeDecks = decks.filter((deck) => deck.archivedAt === null);
  const [selectedDeckIds, setSelectedDeckIds] = useState<Set<string>>(new Set());
  const [selectedTagIds, setSelectedTagIds] = useState<Set<string>>(new Set());
  const [temporaryCardIds, setTemporaryCardIds] = useState<string[]>([]);
  const [shuffle, setShuffle] = useState(settings.practiceShuffle);
  const [session, setSession] = useState<PracticeSession | null>(null);
  const [sessionCards, setSessionCards] = useState<PracticeCard[]>([]);
  const [sessionWindowStart, setSessionWindowStart] = useState(0);
  const [position, setPosition] = useState(0);
  const [revealed, setRevealed] = useState(false);
  const [previewSet, setPreviewSet] = useState<ReviewPreviewSet | null>(null);
  const [workload, setWorkload] = useState<PracticeWorkloadOverview | null>(null);
  const [workloadStatus, setWorkloadStatus] = useState<PracticeWorkloadStatus>("idle");
  const [workloadError, setWorkloadError] = useState<string | null>(null);
  const [isLoadingWindow, setIsLoadingWindow] = useState(false);
  const [isStarting, setIsStarting] = useState(false);
  const [isRevealing, setIsRevealing] = useState(false);
  const [isRating, setIsRating] = useState(false);
  const [isEnding, setIsEnding] = useState(false);
  const [isManaging, setIsManaging] = useState(false);
  const [isLoadingEditor, setIsLoadingEditor] = useState(false);
  const [editingCard, setEditingCard] = useState<CardNote | null>(null);
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
  const confirm = useConfirm();
  const experience = useExperience();

  useEffect(() => {
    if (!session || completed) return registerPracticeExitHandler(null);
    const activeSessionId = session.id;
    return registerPracticeExitHandler(async () => {
      if (pendingReviewRef.current) {
        setErrorMessage(
          "Finish or safely retry the pending rating before leaving this practice session.",
        );
        return false;
      }
      if (endingRef.current) return false;
      endingRef.current = true;
      setIsEnding(true);
      setErrorMessage(null);
      try {
        await controller.complete(activeSessionId);
        return true;
      } catch (error: unknown) {
        setErrorMessage(getErrorMessage(error));
        return false;
      } finally {
        endingRef.current = false;
        setIsEnding(false);
      }
    });
  }, [completed, controller, session]);

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
      setWorkloadError(null);
      setWorkloadStatus("empty");
      return;
    }
    setWorkload(null);
    setWorkloadError(null);
    setWorkloadStatus("loading");
    try {
      const loaded = await controller.workload(deckIds, tagIds);
      if (!loaded) return;
      setWorkload(loaded);
      setWorkloadStatus("ready");
    } catch (error: unknown) {
      setWorkload(null);
      setWorkloadError(getErrorMessage(error));
      setWorkloadStatus("error");
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
          await controller.start({
            kind: "selected",
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
        setSessionWindowStart(0);
        handleStartFailure(error, key);
      } finally {
        startingRef.current = false;
        setIsStarting(false);
      }
    },
    [controller],
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
          await controller.start({
            kind: "cards",
            operationId,
            cardIds,
            shuffle: requestedShuffle,
          }),
        );
      } catch (error: unknown) {
        setSession(null);
        setSessionCards([]);
        setSessionWindowStart(0);
        handleStartFailure(error, key);
      } finally {
        startingRef.current = false;
        setIsStarting(false);
      }
    },
    [controller],
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
    setSessionWindowStart(started.firstWindow.startPosition);
    setPosition(0);
    setRevealed(false);
    setPreviewSet(null);
    setCompleted(false);
    setRatedCount(0);
    setConfidenceCounts([0, 0, 0, 0]);
    questionShownAtRef.current = Date.now();
  }

  const loadNextWindow = useCallback(async (): Promise<number> => {
    const loadedThrough = sessionWindowStart + sessionCards.length;
    if (!session || loadingWindowRef.current || loadedThrough >= session.cardCount) {
      return loadedThrough;
    }
    loadingWindowRef.current = true;
    setIsLoadingWindow(true);
    try {
      const window = await controller.window({
        sessionId: session.id,
        startPosition: loadedThrough,
        limit: PRACTICE_WINDOW_SIZE,
      });
      if (!window) return loadedThrough;
      setSessionCards((currentCards) => [...currentCards, ...window.cards]);
      setErrorMessage(null);
      return loadedThrough + window.cards.length;
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
      return loadedThrough;
    } finally {
      loadingWindowRef.current = false;
      setIsLoadingWindow(false);
    }
  }, [controller, session, sessionCards.length, sessionWindowStart]);

  useEffect(() => {
    void loadRecentPractice();
  }, [loadRecentPractice]);

  useEffect(() => {
    void loadWorkload([...selectedDeckIds], [...selectedTagIds]);
  }, [loadWorkload, selectedDeckIds, selectedTagIds]);

  useEffect(() => {
    if (!session || sessionWindowStart + sessionCards.length >= session.cardCount) return;
    if (position + 5 >= sessionCards.length) void loadNextWindow();
  }, [loadNextWindow, position, session, sessionCards.length, sessionWindowStart]);

  useEffect(() => {
    if (!launchRequest || handledLaunchRequestRef.current === launchRequest.requestId) return;
    handledLaunchRequestRef.current = launchRequest.requestId;
    pendingStartRef.current = null;
    pendingReviewRef.current = null;
    setSession(null);
    setSessionCards([]);
    setSessionWindowStart(0);
    setCompleted(false);
    setErrorMessage(null);
    if (launchRequest.kind === "cards") {
      setSelectedDeckIds(new Set());
      setSelectedTagIds(new Set());
      setTemporaryCardIds(launchRequest.cardIds);
      onLaunchConsumed();
      if (launchRequest.autoStart) {
        void beginCardSession(launchRequest.cardIds, settings.practiceShuffle);
      }
      return;
    }
    const eligible = launchRequest.deckIds.filter((deckId) =>
      activeDecks.some((deck) => deck.id === deckId && deck.generatedCardCount > 0),
    );
    setTemporaryCardIds([]);
    setSelectedDeckIds(new Set(eligible));
    setSelectedTagIds(new Set(launchRequest.tagIds));
    onLaunchConsumed();
    if (launchRequest.autoStart && (eligible.length > 0 || launchRequest.tagIds.length > 0)) {
      void beginSession(
        eligible,
        launchRequest.tagIds,
        settings.practiceShuffle,
        launchRequest.mode,
      );
    }
  }, [
    activeDecks,
    beginCardSession,
    beginSession,
    launchRequest,
    onLaunchConsumed,
    settings.practiceShuffle,
  ]);

  useEffect(() => {
    if (!restoreCheckpoint) return;
    let active = true;
    onRestoreConsumed();
    pendingStartRef.current = null;
    pendingReviewRef.current = null;
    setErrorMessage(null);
    setIsStarting(true);
    void controller
      .resume(restoreCheckpoint.sessionId)
      .then(async (restored) => {
        if (!active) return;
        setSession(restored.session);
        setSessionCards(restored.session.firstWindow.cards);
        setSessionWindowStart(restored.resumePosition);
        setPosition(0);
        setRatedCount(restored.ratedCount);
        setConfidenceCounts([0, 0, 0, 0]);
        setCompleted(restored.completed);
        setRevealed(restored.revealed);
        setPreviewSet(null);
        setTemporaryCardIds([]);
        const first = restored.session.firstWindow.cards[0];
        if (restored.revealed && first && !restored.completed) {
          setPreviewSet(
            await controller.reveal(restored.session.id, first.presentation.cardId),
          );
        }
        questionShownAtRef.current = Date.now();
      })
      .catch((error: unknown) => {
        if (!active) return;
        setErrorMessage(
          `The interrupted practice session could not be restored: ${getErrorMessage(error)}`,
        );
        setSession(null);
        setSessionCards([]);
        setSessionWindowStart(0);
      })
      .finally(() => {
        if (active) setIsStarting(false);
      });
    return () => {
      active = false;
    };
  }, [controller, onRestoreConsumed, restoreCheckpoint]);

  useEffect(() => {
    if (!session && (restoreCheckpoint || isStarting)) return;
    if (!session || completed) {
      onCheckpointChange(null);
      return;
    }
    onCheckpointChange({
      sessionId: session.id,
      position: sessionWindowStart + position,
      revealed,
    });
  }, [
    completed,
    isStarting,
    onCheckpointChange,
    position,
    restoreCheckpoint,
    revealed,
    session,
    sessionWindowStart,
  ]);

  useEffect(() => {
    if (!session || !current || completed) return;
    questionShownAtRef.current = Date.now();
    void controller.markViewed(session.id, current.presentation.cardId).catch((error: unknown) => {
      showToast("error", getErrorMessage(error));
    });
  }, [completed, controller, current, session, showToast]);

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
      setPreviewSet(await controller.reveal(session.id, current.presentation.cardId));
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
        pending.cardId !== current.presentation.cardId ||
        pending.score !== score)
    ) {
      setErrorMessage(
        `The result of rating ${pending.score} is still unknown. Choose ${pending.score} again to safely retry that exact review.`,
      );
      return;
    }

    const input: SubmitReviewInput = pending?.input ?? {
      sessionId: session.id,
      cardId: current.presentation.cardId,
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
        cardId: current.presentation.cardId,
        score,
        input,
      };
    }

    ratingRef.current = true;
    setIsRating(true);
    setErrorMessage(null);
    try {
      const result = await controller.review(input);
      pendingReviewRef.current = null;
      setRatedCount(result.ratedCount);
      setConfidenceCounts((counts) => {
        const next: ConfidenceCounts = [...counts];
        next[score] += 1;
        return next;
      });
      experience.emit({ type: "review_committed", score });
      await advanceOrComplete(result.completed);
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

  async function editCurrentCard(): Promise<void> {
    if (!session || !current || isLoadingEditor || pendingReviewRef.current) return;
    setIsLoadingEditor(true);
    setErrorMessage(null);
    try {
      setEditingCard(await getCardNote(current.presentation.noteId));
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsLoadingEditor(false);
    }
  }

  async function finishCardEdit(saved: CardNote): Promise<void> {
    if (!session) return;
    const refreshed = await controller.refreshAfterEdit({
      sessionId: session.id,
      noteId: saved.noteId,
      startPosition: sessionWindowStart + position,
    });
    setSession((currentSession) =>
      currentSession ? { ...currentSession, cardCount: refreshed.cardCount } : currentSession,
    );
    setEditingCard(null);
    setSessionWindowStart(refreshed.startPosition);
    setPosition(0);
    setSessionCards(refreshed.window.cards);
    setRevealed(false);
    setPreviewSet(null);
    questionShownAtRef.current = Date.now();
    if (refreshed.completed) {
      setCompleted(true);
      experience.emit({ type: "session_completed" });
    }
    showToast("success", "Card updated. Practice session refreshed.");
  }

  async function manageCurrent(
    action: ScheduleAction,
    rescheduleIntent?: RescheduleIntent,
  ): Promise<void> {
    if (!session || !current || managingRef.current) return;
    if (pendingReviewRef.current) {
      setErrorMessage("Retry the pending rating before changing this card's schedule.");
      return;
    }
    if (action === "reset") {
      const accepted = await confirm({
        title: "Reset this card to New?",
        description:
          "Its review history will remain available for analytics, but the current FSRS schedule will restart.",
        confirmLabel: "Reset schedule",
        destructive: true,
      });
      if (!accepted) return;
    }
    managingRef.current = true;
    setIsManaging(true);
    setErrorMessage(null);
    try {
      await controller.schedule({
        mutationId: createMutationId(),
        cardIds: [current.presentation.cardId],
        action,
        rescheduleIntent,
        sessionId: session.id,
      });
      if (action === "bury" || action === "suspend") {
        showToast(
          "info",
          action === "bury" ? "Card buried until tomorrow." : "Card suspended.",
        );
        await advanceOrComplete(
          sessionWindowStart + position + 1 >= session.cardCount,
        );
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

  async function advanceOrComplete(backendCompleted: boolean): Promise<void> {
    pendingReviewRef.current = null;
    if (
      !session ||
      backendCompleted ||
      sessionWindowStart + position + 1 >= session.cardCount
    ) {
      setCompleted(true);
      experience.emit({ type: "session_completed" });
      void loadRecentPractice();
      return;
    }
    const nextPosition = position + 1;
    if (nextPosition >= sessionCards.length) {
      const loadedCount = await loadNextWindow();
      if (loadedCount <= sessionWindowStart + nextPosition) return;
    }
    setPosition(nextPosition);
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
      await controller.complete(session.id);
      if (returnTarget) {
        onEndEarly(returnTarget);
        return;
      }
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
    setSessionCards([]);
    setSessionWindowStart(0);
    setPosition(0);
    setRevealed(false);
    setPreviewSet(null);
    setCompleted(false);
    setRatedCount(0);
    setConfidenceCounts([0, 0, 0, 0]);
    setTemporaryCardIds([]);
    setEditingCard(null);
    onClearReturnTarget();
    setSelectedTagIds(new Set());
    setErrorMessage(null);
    void loadRecentPractice();
  }

  if (session && editingCard) {
    const editorDeck = activeDecks.find((deck) => deck.id === editingCard.deckId);
    if (editorDeck) {
      return (
        <CardEditorScreen
          deck={editorDeck}
          decks={[editorDeck]}
          card={editingCard}
          onCancel={() => setEditingCard(null)}
          onSaved={async (saved) => finishCardEdit(saved)}
          onTemplateGenerated={async () => undefined}
          onDirtyChange={() => undefined}
          onMoveCopy={() => undefined}
        />
      );
    }
  }

  if (session && completed) {
    const sessionDeckIds = [...new Set(sessionCards.map((card) => card.presentation.deckId))];
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
              [...selectedTagIds],
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
      <PerformanceProfiler id="active-practice">
        <ActivePractice
        session={session}
        current={current}
        position={sessionWindowStart + position}
        revealed={revealed}
        ratedCount={ratedCount}
        previewSet={previewSet}
        isRevealing={isRevealing}
        isRating={isRating}
        isEnding={isEnding}
        isManaging={isManaging || isLoadingWindow || isLoadingEditor}
        flipAnimationEnabled={settings.animationsEnabled && !settings.reduceMotion}
        errorMessage={errorMessage}
        onReveal={() => void revealAnswer()}
        onScore={(score) => void submitScore(score)}
        onEdit={() => void editCurrentCard()}
        onScheduleAction={(action, intent) => void manageCurrent(action, intent)}
        onEnd={() => void endSession()}
        onLinkError={setErrorMessage}
        />
      </PerformanceProfiler>
    );
  }

  return (
    <div className={ownedClassNames(ownedStyleMaps, "screen practice-screen")}>
      <header className={ownedClassNames(ownedStyleMaps, "screen-header compact-screen-header")}>
        <div>
          <span className={ownershipStyles.screenIcon} aria-hidden="true">
            <Icon name="brain" size={22} />
          </span>
          <h1>Practice</h1>
          <p>
            Scheduled sessions protect ready reviews and daily workload limits. Extra study is
            always an explicit choice.
          </p>
        </div>
      </header>
      {errorMessage ? <div className={ownedClassNames(ownedStyleMaps, "inline-alert error-alert")}>{errorMessage}</div> : null}
      {selectedTagIds.size > 0 ? (
        <div className={ownedClassNames(ownedStyleMaps, "inline-alert info-alert")}>
          This launch is scoped to {selectedTagIds.size} selected tag
          {selectedTagIds.size === 1 ? "" : "s"}.
        </div>
      ) : null}
      <PerformanceProfiler id="practice-setup">
        <PracticeSetup
          decks={activeDecks}
          selectedDeckIds={selectedDeckIds}
          shuffle={shuffle}
          isStarting={isStarting}
          workloadStatus={workloadStatus}
          workload={workload}
          workloadError={workloadError}
          recentDecks={recentDecks}
          onSelectedDeckIdsChange={(next) => {
            setSelectedDeckIds(next);
            setSelectedTagIds(new Set());
          }}
          onShuffleChange={setShuffle}
          onStart={(deckIds, requestedShuffle, mode) =>
            void beginSession(deckIds, [...selectedTagIds], requestedShuffle, mode)
          }
        />
      </PerformanceProfiler>
      <StudyGuideLibrarySection
        showToast={showToast}
        onPracticeCards={(cardIds) => beginCardSession(cardIds, settings.practiceShuffle)}
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
  tagIds: string[],
  shuffle: boolean,
  mode: "scheduled" | "extra",
): string {
  const canonicalDeckIds = [...new Set(deckIds)].sort();
  const canonicalTagIds = [...new Set(tagIds)].sort();
  return JSON.stringify({
    kind: "selected",
    deckIds: canonicalDeckIds,
    tagIds: canonicalTagIds,
    shuffle,
    mode,
  });
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
