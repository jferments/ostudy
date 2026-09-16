import { useCallback, useEffect, useRef, useState } from "react";

import type { PracticeLaunchRequest, PracticeReturnTarget } from "../App";
import { PracticeController } from "../controllers/practiceController";
import { registerPracticeExitHandler } from "../controllers/practiceExitGuard";
import type { AuthoringNote } from "../ipc/authoringNotes";
import { getCardNote } from "../ipc/cards";
import {
  createMutationId,
  createOperationId,
  createSubmissionId,
  getErrorCode,
  getErrorMessage,
} from "../ipc/common";
import type { DeckSummary } from "../ipc/decks";
import {
  isInteractivePracticePresentation,
  type CardPracticeRequest,
  type PracticeCard,
  type PracticeSession,
  type ReviewPreviewSet,
  type SelectedPracticeRequest,
  type SubmitReviewInput,
} from "../ipc/practice";
import type {
  CheckPracticeAnswerInput,
  QuestionAttemptView,
  SavePracticeAnswerInput,
} from "../ipc/questionAttempts";
import { getQuestionNote } from "../ipc/questions";
import type { RescheduleIntent, ScheduleAction } from "../ipc/scheduling";
import type { AppSettings } from "../ipc/settings";
import type { PracticeCheckpoint, PracticeCursorCheckpoint } from "../ipc/uiCheckpoint";
import { PerformanceProfiler } from "../testing/PerformanceProfiler";
import { useExperience } from "../ui/feedback/ExperienceProvider";
import { useConfirm } from "../ui/overlays/overlayContext";
import { PageHeader, PageScaffold } from "../ui/patterns/WorkspacePatterns";
import { Button } from "../ui/primitives/Button";
import { LoadingState } from "../ui/primitives/EmptyState";
import { InlineNotice } from "../ui/primitives/InlineNotice";
import { CardEditorScreen } from "./CardEditorScreen";
import { PracticeComplete } from "./practice/PracticeComplete";
import { ActivePractice } from "./practice/ActivePractice";
import {
  outcomeForScore,
  type ConfidenceCounts,
  type ConfidenceScore,
} from "./practice/confidence";

interface PracticeScreenProps {
  decks: DeckSummary[];
  settings: AppSettings;
  launchRequest: PracticeLaunchRequest | null;
  onLaunchConsumed: () => void;
  restoreCheckpoint?: PracticeCheckpoint | null;
  onRestoreConsumed?: () => void;
  onCheckpointChange?: (checkpoint: PracticeCursorCheckpoint | null) => void;
  returnTarget: PracticeReturnTarget | null;
  onViewAnalytics: (sessionId: string, deckIds: string[]) => void;
  onEndEarly: (target: PracticeReturnTarget) => void | Promise<void>;
  onChooseDecks: () => void | Promise<void>;
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
  questionRevision: number | null;
}

type CanonicalPracticeRequest =
  | ({ kind: "selected" } & SelectedPracticeRequest)
  | ({ kind: "cards" } & CardPracticeRequest);
type PracticeRecovery =
  | { kind: "start"; request: CanonicalPracticeRequest }
  | { kind: "restore"; checkpoint: PracticeCheckpoint };

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
  onViewAnalytics,
  onEndEarly,
  onChooseDecks,
  showToast,
}: PracticeScreenProps) {
  const activeDecks = decks.filter((deck) => deck.archivedAt === null);
  const [practiceRequest, setPracticeRequest] = useState<CanonicalPracticeRequest | null>(null);
  const [recovery, setRecovery] = useState<PracticeRecovery | null>(null);
  const [session, setSession] = useState<PracticeSession | null>(null);
  const [sessionCards, setSessionCards] = useState<PracticeCard[]>([]);
  const [sessionWindowStart, setSessionWindowStart] = useState(0);
  const [position, setPosition] = useState(0);
  const [revealed, setRevealed] = useState(false);
  const [previewSet, setPreviewSet] = useState<ReviewPreviewSet | null>(null);
  const [isLoadingWindow, setIsLoadingWindow] = useState(false);
  const [isStarting, setIsStarting] = useState(false);
  const [isRevealing, setIsRevealing] = useState(false);
  const [isRating, setIsRating] = useState(false);
  const [isEnding, setIsEnding] = useState(false);
  const [isManaging, setIsManaging] = useState(false);
  const [isLoadingEditor, setIsLoadingEditor] = useState(false);
  const [editingCard, setEditingCard] = useState<AuthoringNote | null>(null);
  const [completed, setCompleted] = useState(false);
  const [ratedCount, setRatedCount] = useState(0);
  const [confidenceCounts, setConfidenceCounts] = useState<ConfidenceCounts>([0, 0, 0, 0]);
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
    if (isStarting) return registerPracticeExitHandler(async () => false);
    if (!session || completed) return registerPracticeExitHandler(null);
    const activeSessionId = session.id;
    return registerPracticeExitHandler(async () => {
      if (pendingReviewRef.current) {
        setErrorMessage("Finish or safely retry the pending rating before leaving this practice session.");
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
  }, [completed, controller, isStarting, session]);

  useEffect(() => () => controller.invalidate(), [controller]);

  const beginPractice = useCallback(async (request: CanonicalPracticeRequest): Promise<void> => {
    if (startingRef.current || !practiceRequestHasScope(request)) return;
    const key = practiceStartKey(request);
    const operationId = operationIdForStart(pendingStartRef, key);
    startingRef.current = true;
    setIsStarting(true);
    setPracticeRequest(request);
    setRecovery(null);
    setErrorMessage(null);
    try {
      initializeSession(await controller.start({ ...request, operationId }));
    } catch (error: unknown) {
      setSession(null);
      setSessionCards([]);
      setSessionWindowStart(0);
      setRecovery({ kind: "start", request });
      const code = getErrorCode(error);
      const ambiguity = code === null || code === COMMITTED_START_RETRY_CODE;
      setErrorMessage(ambiguity
        ? `${getErrorMessage(error)} Retry uses the same operation identity, so it cannot create a duplicate session.`
        : getErrorMessage(error));
    } finally {
      startingRef.current = false;
      setIsStarting(false);
    }
  }, [controller]);

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
    setRecovery(null);
    questionShownAtRef.current = Date.now();
  }

  const loadNextWindow = useCallback(async (): Promise<number> => {
    const loadedThrough = sessionWindowStart + sessionCards.length;
    if (!session || loadingWindowRef.current || loadedThrough >= session.cardCount) return loadedThrough;
    loadingWindowRef.current = true;
    setIsLoadingWindow(true);
    try {
      const window = await controller.window({
        sessionId: session.id,
        startPosition: loadedThrough,
        limit: PRACTICE_WINDOW_SIZE,
      });
      if (!window) return loadedThrough;
      setSessionCards((cards) => [...cards, ...window.cards]);
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
    const request = canonicalLaunchRequest(launchRequest);
    onLaunchConsumed();
    void beginPractice(request);
  }, [beginPractice, launchRequest, onLaunchConsumed]);

  const resumeCheckpoint = useCallback(async (checkpoint: PracticeCheckpoint): Promise<void> => {
    if (startingRef.current) return;
    startingRef.current = true;
    pendingStartRef.current = null;
    pendingReviewRef.current = null;
    setPracticeRequest(null);
    setRecovery({ kind: "restore", checkpoint });
    setErrorMessage(null);
    setIsStarting(true);
    try {
      const restored = await controller.resume(checkpoint.sessionId);
      setSession(restored.session);
      setSessionCards(restored.session.firstWindow.cards);
      setSessionWindowStart(restored.resumePosition);
      setPosition(0);
      setRatedCount(restored.ratedCount);
      setConfidenceCounts([0, 0, 0, 0]);
      setCompleted(restored.completed);
      setRevealed(restored.revealed);
      setPreviewSet(null);
      const first = restored.session.firstWindow.cards[0];
      if (restored.revealed && first && !restored.completed) {
        setPreviewSet(await controller.reveal(restored.session.id, first.presentation.cardId));
      }
      setRecovery(null);
      questionShownAtRef.current = Date.now();
    } catch (error: unknown) {
      setErrorMessage(`The interrupted practice session could not be restored: ${getErrorMessage(error)}`);
      setSession(null);
      setSessionCards([]);
      setSessionWindowStart(0);
      setRecovery({ kind: "restore", checkpoint });
    } finally {
      startingRef.current = false;
      setIsStarting(false);
    }
  }, [controller]);

  useEffect(() => {
    if (!restoreCheckpoint) return;
    onRestoreConsumed();
    void resumeCheckpoint(restoreCheckpoint);
  }, [onRestoreConsumed, restoreCheckpoint, resumeCheckpoint]);

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
  }, [completed, isStarting, onCheckpointChange, position, restoreCheckpoint, revealed, session, sessionWindowStart]);

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

  function replaceCurrentQuestionAttempt(attempt: QuestionAttemptView): void {
    setSessionCards((cards) => cards.map((card, index) => {
      if (index !== position || !isInteractivePracticePresentation(card.presentation)) return card;
      return {
        ...card,
        presentation: {
          ...card.presentation,
          question: { ...card.presentation.question, attempt },
        },
      };
    }));
  }

  async function saveQuestionAnswer(input: SavePracticeAnswerInput): Promise<QuestionAttemptView> {
    const saved = await controller.saveQuestionAnswer(input);
    replaceCurrentQuestionAttempt(saved);
    return saved;
  }

  async function checkQuestionAnswer(input: CheckPracticeAnswerInput): Promise<QuestionAttemptView> {
    const checked = await controller.checkQuestionAnswer(input);
    replaceCurrentQuestionAttempt(checked);
    return checked;
  }

  async function finishQuestionCheck(attempt: QuestionAttemptView): Promise<void> {
    if (!session || !current || !isInteractivePracticePresentation(current.presentation)) return;
    replaceCurrentQuestionAttempt(attempt);
    setErrorMessage(null);
    const preview = await controller.reveal(session.id, current.presentation.cardId);
    setPreviewSet(preview);
    setRevealed(true);
    try {
      const refreshed = await controller.question({
        sessionId: session.id,
        cardId: current.presentation.cardId,
        instanceId: attempt.instanceId,
      });
      if (!refreshed) return;
      setSessionCards((cards) => cards.map((card, index) => {
        if (index !== position || !isInteractivePracticePresentation(card.presentation)) return card;
        return {
          ...card,
          presentation: { ...card.presentation, question: refreshed },
        };
      }));
    } catch (error: unknown) {
      setErrorMessage(`Your answer was checked and saved, but its feedback media could not refresh: ${getErrorMessage(error)}`);
    }
  }

  async function submitScore(score: ConfidenceScore): Promise<void> {
    if (!session || !current || !revealed || !previewSet || ratingRef.current) return;
    const interactivePresentation = isInteractivePracticePresentation(current.presentation)
      ? current.presentation
      : null;
    const progress = interactivePresentation?.question.attempt.progress ?? null;
    if (interactivePresentation && progress?.phase !== "checked") {
      setErrorMessage("Check the answer before choosing its difficulty.");
      return;
    }
    const pending = pendingReviewRef.current;
    if (pending && (pending.sessionId !== session.id
      || pending.cardId !== current.presentation.cardId
      || pending.score !== score)) {
      setErrorMessage(`The result of rating ${pending.score} is still unknown. Choose ${pending.score} again to safely retry that exact review.`);
      return;
    }
    const responseTimeMs = progress?.phase === "checked"
      ? progress.responseTimeMs
      : Math.max(0, Math.min(86_400_000, Date.now() - questionShownAtRef.current));
    const input: SubmitReviewInput = pending?.input ?? {
      sessionId: session.id,
      cardId: current.presentation.cardId,
      submissionId: createSubmissionId(),
      outcome: outcomeForScore(score),
      responseTimeMs,
      scheduleRevision: previewSet.scheduleRevision,
    };
    const questionRevision = interactivePresentation?.question.attempt.revision ?? null;
    if (!pending) {
      pendingReviewRef.current = {
        sessionId: session.id,
        cardId: current.presentation.cardId,
        score,
        input,
        questionRevision,
      };
    }
    ratingRef.current = true;
    setIsRating(true);
    setErrorMessage(null);
    try {
      const result = interactivePresentation
        ? await controller.reviewQuestion({
            review: input,
            question: {
              instanceId: interactivePresentation.question.attempt.instanceId,
              attemptRevision: pending?.questionRevision ?? questionRevision ?? 0,
              overrideReason: null,
            },
          })
        : await controller.review(input);
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
        setErrorMessage(`${getErrorMessage(error)} Choose ${score} again to safely retry the same review without recording it twice.`);
      } else {
        pendingReviewRef.current = null;
        setErrorMessage(getErrorMessage(error));
        if (code === "schedule_changed") {
          try {
            setPreviewSet(await controller.reveal(session.id, current.presentation.cardId));
          } catch (previewError: unknown) {
            setPreviewSet(null);
            setErrorMessage(`${getErrorMessage(error)} ${getErrorMessage(previewError)}`);
          }
        } else if (code === "idempotency_conflict" && !interactivePresentation) {
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
      const note = isInteractivePracticePresentation(current.presentation)
        ? await getQuestionNote(current.presentation.noteId)
        : await getCardNote(current.presentation.noteId);
      setEditingCard(note);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsLoadingEditor(false);
    }
  }

  async function finishCardEdit(noteId: string): Promise<void> {
    if (!session) return;
    const refreshed = await controller.refreshAfterEdit({
      sessionId: session.id,
      noteId,
      startPosition: sessionWindowStart + position,
    });
    setSession((value) => value ? { ...value, cardCount: refreshed.cardCount } : value);
    setEditingCard(null);
    setSessionWindowStart(refreshed.startPosition);
    setPosition(0);
    setSessionCards(refreshed.window.cards);
    const first = refreshed.window.cards[0];
    const firstChecked = first && isInteractivePracticePresentation(first.presentation)
      && first.presentation.question.attempt.progress.phase === "checked";
    setRevealed(Boolean(firstChecked));
    setPreviewSet(firstChecked && first
      ? await controller.reveal(session.id, first.presentation.cardId)
      : null);
    questionShownAtRef.current = Date.now();
    if (refreshed.completed) {
      setCompleted(true);
      experience.emit({ type: "session_completed" });
    }
    showToast("success", "Card updated. The current practice attempt kept its captured answer key.");
  }

  async function manageCurrent(action: ScheduleAction, rescheduleIntent?: RescheduleIntent): Promise<void> {
    if (!session || !current || managingRef.current) return;
    if (pendingReviewRef.current) {
      setErrorMessage("Retry the pending rating before changing this card's schedule.");
      return;
    }
    if (action === "reset") {
      const accepted = await confirm({
        title: "Reset this card to New?",
        description: "Its review history will remain available for analytics, but the current FSRS schedule will restart.",
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
        showToast("info", action === "bury" ? "Card buried until tomorrow." : "Card suspended.");
        await advanceOrComplete(sessionWindowStart + position + 1 >= session.cardCount);
      } else {
        showToast("success", action === "reset" ? "Card schedule reset." : "Card rescheduled.");
        if (!isInteractivePracticePresentation(current.presentation)) {
          setRevealed(false);
          setPreviewSet(null);
        }
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
    if (!session || backendCompleted || sessionWindowStart + position + 1 >= session.cardCount) {
      setCompleted(true);
      experience.emit({ type: "session_completed" });
      return;
    }
    const nextPosition = position + 1;
    if (nextPosition >= sessionCards.length) {
      const loadedCount = await loadNextWindow();
      if (loadedCount <= sessionWindowStart + nextPosition) return;
    }
    if (session.mode !== "custom" && nextPosition >= PRACTICE_WINDOW_SIZE) {
      setSessionCards((cards) => cards.slice(nextPosition));
      setSessionWindowStart((start) => start + nextPosition);
      setPosition(0);
    } else {
      setPosition(nextPosition);
    }
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
        await onEndEarly(returnTarget);
        return;
      }
      await onChooseDecks();
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      endingRef.current = false;
      setIsEnding(false);
    }
  }

  if (session && editingCard) {
    const editorDeck = activeDecks.find((deck) => deck.id === editingCard.deckId);
    if (editorDeck) return <CardEditorScreen deck={editorDeck} decks={[editorDeck]} card={editingCard}
      onCancel={() => setEditingCard(null)} onSaved={async (saved) => finishCardEdit(saved.noteId)}
      onQuestionSaved={async (noteId) => finishCardEdit(noteId)} onTemplateGenerated={async () => undefined}
      onDirtyChange={() => undefined} onMoveCopy={() => undefined} />;
  }

  if (session && completed) {
    const sessionDeckIds = practiceRequest?.kind === "selected"
      ? practiceRequest.deckIds
      : practiceRequest?.kind === "cards"
        ? [...new Set(sessionCards.map((card) => card.presentation.deckId))]
        : [];
    return <PracticeComplete ratedCount={ratedCount} confidenceCounts={confidenceCounts} isStarting={isStarting}
      canRestart={practiceRequest !== null} errorMessage={errorMessage}
      onViewAnalytics={() => { if (!startingRef.current) onViewAnalytics(session.id, sessionDeckIds); }}
      onChooseDecks={() => { if (!startingRef.current) void onChooseDecks(); }}
      onRestart={() => { if (practiceRequest) void beginPractice(practiceRequest); }} />;
  }

  if (session && current) return <PerformanceProfiler id="active-practice">
    <ActivePractice session={session} current={current} position={sessionWindowStart + position} revealed={revealed}
      ratedCount={ratedCount} previewSet={previewSet} isRevealing={isRevealing} isRating={isRating} isEnding={isEnding}
      isManaging={isManaging || isLoadingWindow || isLoadingEditor}
      flipAnimationEnabled={settings.animationsEnabled && !settings.reduceMotion}
      displayQuestionOnAnswer={settings.displayQuestionOnAnswer ?? false} errorMessage={errorMessage}
      onReveal={() => void revealAnswer()} onScore={(score) => void submitScore(score)} onEdit={() => void editCurrentCard()}
      onScheduleAction={(action, intent) => void manageCurrent(action, intent)} onEnd={() => void endSession()}
      onLinkError={setErrorMessage} onSaveQuestionAnswer={saveQuestionAnswer}
      onCheckQuestionAnswer={checkQuestionAnswer} onQuestionAttempt={replaceCurrentQuestionAttempt}
      onQuestionChecked={finishQuestionCheck} />
  </PerformanceProfiler>;

  if (isStarting && !session) {
    return <LoadingState label={recovery?.kind === "restore" ? "Restoring practice…" : "Starting practice…"} />;
  }

  return <PageScaffold>
    <PageHeader title="Practice" icon="brain" description="The requested session could not be opened safely." />
    {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
    <div className="tw:mt-5 tw:flex tw:flex-wrap tw:gap-3">
      <Button variant="primary" disabled={!recovery || isStarting} onClick={() => {
        if (recovery?.kind === "start") void beginPractice(recovery.request);
        if (recovery?.kind === "restore") void resumeCheckpoint(recovery.checkpoint);
      }}>Retry</Button>
      <Button disabled={isStarting} onClick={() => {
        if (returnTarget) void onEndEarly(returnTarget);
        else void onChooseDecks();
      }}>Return</Button>
    </div>
  </PageScaffold>;
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

function canonicalLaunchRequest(request: PracticeLaunchRequest): CanonicalPracticeRequest {
  if (request.kind === "selected") {
    return {
      kind: "selected",
      deckIds: [...new Set(request.deckIds)],
      tagIds: [...new Set(request.tagIds)],
      shuffle: request.shuffle,
      mode: request.mode,
    };
  }
  const seen = new Set<string>();
  const cardIds = request.cardIds.filter((cardId) => {
    if (seen.has(cardId)) return false;
    seen.add(cardId);
    return true;
  });
  return { kind: "cards", cardIds, shuffle: request.shuffle };
}

function practiceRequestHasScope(request: CanonicalPracticeRequest): boolean {
  return request.kind === "cards"
    ? request.cardIds.length > 0
    : request.deckIds.length > 0 || request.tagIds.length > 0;
}

function practiceStartKey(request: CanonicalPracticeRequest): string {
  return JSON.stringify(request);
}
