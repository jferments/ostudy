import { useEffect, useRef } from "react";
import { TutorEntryButton } from "../tutor/TutorEntryButton";
import {
  isInteractivePracticePresentation,
  type PracticeCard,
  type PracticeSession,
  type ReviewPreviewSet,
} from "../../ipc/practice";
import type {
  CheckPracticeAnswerInput,
  QuestionAttemptView,
  PracticeAttemptView,
  ImageOcclusionAttemptView,
  RevealPracticeQuestionInput,
  SavePracticeAnswerInput,
} from "../../ipc/questionAttempts";
import type { RescheduleIntent, ScheduleAction } from "../../ipc/scheduling";
import { useExperience } from "../../ui/feedback/ExperienceProvider";
import { Icon } from "../../ui/icons/Icon";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { OverflowMenu, type OverflowMenuItem } from "../../ui/overlays/OverflowMenu";
import { useOverlayContext } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Progress } from "../../ui/primitives/Progress";
import { cn } from "../../ui/internal/cn";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { practiceStyles as view, scoreTones } from "./presentation";
import styles from "./PracticeCard.module.css";
import type { ConfidenceScore } from "./confidence";
import { InteractiveQuestionActivity } from "./InteractiveQuestionActivity";
import { RecallCardSurface } from "./RecallCardSurface";

interface ActivePracticeProps {
  session: PracticeSession;
  current: PracticeCard;
  position: number;
  revealed: boolean;
  ratedCount: number;
  previewSet: ReviewPreviewSet | null;
  isRevealing: boolean;
  isRating: boolean;
  isEnding: boolean;
  isManaging: boolean;
  flipAnimationEnabled?: boolean;
  displayQuestionOnAnswer?: boolean;
  errorMessage: string | null;
  onReveal: () => void;
  onScore: (score: ConfidenceScore) => void;
  onEdit: () => void;
  onScheduleAction: (action: ScheduleAction, intent?: RescheduleIntent) => void;
  onEnd: () => void;
  onLinkError: (message: string) => void;
  onSaveQuestionAnswer?: (input: SavePracticeAnswerInput) => Promise<QuestionAttemptView>;
  onCheckQuestionAnswer?: (input: CheckPracticeAnswerInput) => Promise<QuestionAttemptView>;
  onRevealQuestionAnswer?: (input: RevealPracticeQuestionInput) => Promise<ImageOcclusionAttemptView>;
  onQuestionAttempt?: (attempt: PracticeAttemptView) => void;
  onQuestionChecked?: (attempt: PracticeAttemptView) => void | Promise<void>;
}

export function ActivePractice({
  session,
  current,
  position,
  revealed,
  ratedCount,
  previewSet,
  isRevealing,
  isRating,
  isEnding,
  isManaging,
  flipAnimationEnabled,
  displayQuestionOnAnswer = false,
  errorMessage,
  onReveal,
  onScore,
  onEdit,
  onScheduleAction,
  onEnd,
  onLinkError,
  onSaveQuestionAnswer = unavailableSave,
  onCheckQuestionAnswer = unavailableCheck,
  onRevealQuestionAnswer,
  onQuestionAttempt = ignoreAttempt,
  onQuestionChecked = ignoreAttempt,
}: ActivePracticeProps) {
  const ratingPanelRef = useRef<HTMLElement | null>(null);
  const wasRevealed = useRef(revealed);
  const busy = isRevealing || isRating || isEnding || isManaging;
  const { topId } = useOverlayContext();
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const experience = useExperience();
  const cardCount = Math.max(0, session.cardCount);
  const studiedCount = Math.min(Math.max(0, ratedCount), cardCount);
  const remainingCount = Math.max(0, cardCount - studiedCount);
  const percentComplete = cardCount > 0 ? Math.round((studiedCount / cardCount) * 100) : 0;
  const currentPosition = cardCount > 0 ? Math.min(Math.max(1, position + 1), cardCount) : 0;
  const stackLayers = remainingCount > 3 ? 2 : remainingCount > 1 ? 1 : 0;
  const interactivePresentation = isInteractivePracticePresentation(current.presentation)
    ? current.presentation
    : null;
  const recallPresentation = isInteractivePracticePresentation(current.presentation)
    ? null
    : current.presentation;
  const interactive = interactivePresentation !== null;
  const checkedProgress = interactivePresentation?.question.attempt.progress.phase === "checked"
    ? interactivePresentation.question.attempt.progress
    : null;
  const objectiveCorrect = checkedProgress
    ? checkedProgress.evaluation.verdict === "correct"
    : null;

  useEffect(() => {
    if (revealed) ratingPanelRef.current?.focus();
    if (revealed && !wasRevealed.current) experience.emit({ type: "answer_revealed" });
    wasRevealed.current = revealed;
  }, [experience, revealed]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      if (busy || event.repeat || event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey || topId !== null) return;
      const target = event.target;
      if (target instanceof Element && target.closest("[data-occlusion-viewport]")) return;
      if (target instanceof HTMLElement && (target.isContentEditable
        || ["INPUT", "TEXTAREA", "SELECT", "BUTTON", "A", "SUMMARY", "AUDIO", "VIDEO"].includes(target.tagName))) return;
      if (!interactive && !revealed && (event.key === " " || event.key === "Enter")) {
        event.preventDefault();
        onReveal();
        return;
      }
      if (revealed && ["0", "1", "2", "3"].includes(event.key)) {
        const score = Number(event.key) as ConfidenceScore;
        if (objectiveCorrect === true && score === 0) return;
        if (objectiveCorrect === false && score !== 0) return;
        event.preventDefault();
        onScore(score);
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [busy, interactive, objectiveCorrect, onReveal, onScore, revealed, topId]);

  const cardSurface = interactivePresentation ? (
    <InteractiveQuestionActivity
      key={interactivePresentation.question.attempt.instanceId}
      sessionId={session.id}
      cardId={interactivePresentation.cardId}
      attempt={interactivePresentation.question.attempt}
      assets={interactivePresentation.question.assets}
      disabled={busy}
      onSave={onSaveQuestionAnswer}
      onCheck={onCheckQuestionAnswer}
      onReveal={onRevealQuestionAnswer}
      onAttempt={onQuestionAttempt}
      onChecked={onQuestionChecked}
      onError={onLinkError}
    />
  ) : recallPresentation ? (
    <RecallCardSurface presentation={recallPresentation} presentationKey={recallPresentation.cardId}
      revealed={revealed} busy={busy} flipAnimationEnabled={flipAnimationEnabled} sideToggle
      displayQuestionOnAnswer={displayQuestionOnAnswer} onReveal={onReveal} onLinkError={onLinkError} />
  ) : null;

  const scheduleItems: OverflowMenuItem[] = [
    { id: "bury", label: "Bury until tomorrow", onSelect: () => onScheduleAction("bury") },
    { id: "suspend", label: "Suspend card", onSelect: () => onScheduleAction("suspend") },
    { id: "reschedule-tomorrow", label: "Reschedule for tomorrow", onSelect: () => onScheduleAction("reschedule", { kind: "tomorrow" }) },
    { id: "reset", label: "Reset schedule", destructive: true, onSelect: () => onScheduleAction("reset") },
  ];
  const modeLabel = session.mode === "extra" ? "Extra practice" : session.mode === "custom" ? "Custom practice" : "Practice";

  return <PageScaffold width="full" inset={false} className={view.page}>
    <header className={view.topBar}>
      <div className={view.topBarRow}>
        <div className={view.topBarTitle}>
          <h1 className={view.title}>{current.deckName}</h1>
          {/* A phone's app bar already says Practice, so its narrow bar keeps to the counts. */}
          <p className={view.subtitle}>{compact ? "" : `${modeLabel} · `}{studiedCount} studied · {remainingCount} remaining</p>
        </div>
        <div className={view.topBarActions}>
          <TutorEntryButton disabled={busy} className={view.quietIconButton} target={{
            title: current.deckName,
            scope: { kind: "deck", id: current.presentation.deckId },
            focus: { kind: "card", sessionId: session.id, cardId: current.presentation.cardId, noteRevision: current.presentation.noteRevision },
            mode: revealed ? "explain" : "hint",
          }} />
          {compact ? <OverflowMenu label="Card actions" disabled={busy}
            items={[{ id: "edit", label: "Edit card", onSelect: onEdit }, ...scheduleItems]} /> : <>
            <OverflowMenu label="Schedule" triggerIcon="timer" disabled={busy} items={scheduleItems} />
            <Button variant="ghost" disabled={busy} onClick={onEdit}><Icon name="edit" size={16} />Edit card</Button>
          </>}
          <Button variant="danger" data-practice-action="end" className={view.endAction} disabled={busy} onClick={onEnd}>{isEnding ? "Ending…" : "End session"}</Button>
        </div>
      </div>
      <Progress size="small" label="Practice session progress" value={studiedCount} max={cardCount}
        valueText={`Card ${currentPosition} of ${cardCount}, ${studiedCount} studied, ${remainingCount} remaining, ${percentComplete}% complete`} />
    </header>
    <div className={view.body}>
      <div className={view.column}>
        {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
        <div key={current.presentation.cardId} className={interactive ? "tw:grid tw:gap-5" : styles.stack}
          data-layers={interactive ? undefined : stackLayers} data-tour-anchor="practice.card">{cardSurface}</div>
      </div>
    </div>
    {interactive && !revealed ? null : <div className={view.actionArea} data-practice-actions="">
      <div className={view.column}>
        {!revealed ? <Button variant="primary" size="large" className={view.primaryAction} disabled={busy} onClick={onReveal}>
          {isRevealing ? "Calculating intervals…" : "Show answer"}
        </Button> : <section ref={ratingPanelRef} tabIndex={-1} className={view.ratingGroup}
          aria-labelledby="confidence-rating-heading" data-tour-anchor="practice.confidence">
          <h2 id="confidence-rating-heading" className={view.ratingHeading}>
            {objectiveCorrect === false ? "First answer recorded as wrong" : "How difficult was that answer?"}
          </h2>
          {objectiveCorrect === false ? <p className={view.ratingNote}>
            Continue with Wrong. The checked result remains immutable even if you practice the item again.
          </p> : null}
          {!previewSet ? <Button className={view.ratingRetry} disabled={busy} onClick={onReveal}>
            {isRevealing ? "Calculating intervals…" : "Retry interval preview"}
          </Button> : null}
          <div className={view.ratingButtons}>
            <ConfidenceButton score={0} label="Wrong" disabled={busy || !previewSet || objectiveCorrect === true} onClick={() => onScore(0)} />
            <ConfidenceButton score={1} label="Hard" disabled={busy || !previewSet || objectiveCorrect === false} onClick={() => onScore(1)} />
            <ConfidenceButton score={2} label="Medium" disabled={busy || !previewSet || objectiveCorrect === false} onClick={() => onScore(2)} />
            <ConfidenceButton score={3} label="Easy" disabled={busy || !previewSet || objectiveCorrect === false} onClick={() => onScore(3)} />
          </div>
        </section>}
      </div>
    </div>}
  </PageScaffold>;
}

/** Each rating is tinted with its confidence color; the solid key carries the color and the
 * number that is its keyboard shortcut. */
function ConfidenceButton({ score, label, disabled, onClick }: {
  score: ConfidenceScore;
  label: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return <Button className={cn(view.ratingButton, scoreTones[score])} disabled={disabled}
    aria-label={`${score} ${label}`} aria-keyshortcuts={String(score)} onClick={onClick}>
    <span className={view.ratingKey} aria-hidden="true">{score}</span>
    <span className={view.ratingName}>{label}</span>
  </Button>;
}

function unavailableSave(_input: SavePracticeAnswerInput): Promise<QuestionAttemptView> {
  return Promise.reject(new Error("Interactive answer saving is unavailable in this host."));
}

function unavailableCheck(_input: CheckPracticeAnswerInput): Promise<QuestionAttemptView> {
  return Promise.reject(new Error("Interactive answer checking is unavailable in this host."));
}

function ignoreAttempt(_attempt: PracticeAttemptView): void {}
