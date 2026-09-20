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
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { useOverlayContext } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Progress } from "../../ui/primitives/Progress";
import { cn } from "../../ui/internal/cn";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { practiceStyles as view, scoreColors } from "./presentation";
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
      revealed={revealed} busy={busy} flipAnimationEnabled={flipAnimationEnabled}
      displayQuestionOnAnswer={displayQuestionOnAnswer} onReveal={onReveal} onLinkError={onLinkError} />
  ) : null;

  return <PageScaffold width="wide" className="tw:flex tw:h-full tw:min-h-0 tw:max-w-6xl tw:flex-col tw:overflow-y-auto tw:pt-5 tw:pb-5">
    <header className={view.practiceSessionHeader}>
      <div className={view.practiceSessionTitle}>
        <span className={view.eyebrow}>{session.mode === "extra" ? "EXTRA PRACTICE" : session.mode === "custom" ? "CUSTOM PRACTICE" : "PRACTICE"}</span>
        <h1>{current.deckName}</h1>
      </div>
      <div className={view.practiceHeaderActions}>
        <TutorEntryButton disabled={busy} className={view.practiceHeaderButton} target={{
          title: current.deckName,
          scope: { kind: "deck", id: current.presentation.deckId },
          focus: { kind: "card", sessionId: session.id, cardId: current.presentation.cardId, noteRevision: current.presentation.noteRevision },
          mode: revealed ? "explain" : "hint",
        }} />
        <OverflowMenu label="Schedule" triggerIcon="timer" className={view.practiceHeaderButton} disabled={busy} items={[
          { id: "bury", label: "Bury until tomorrow", onSelect: () => onScheduleAction("bury") },
          { id: "suspend", label: "Suspend card", onSelect: () => onScheduleAction("suspend") },
          { id: "reschedule-tomorrow", label: "Reschedule for tomorrow", onSelect: () => onScheduleAction("reschedule", { kind: "tomorrow" }) },
          { id: "reset", label: "Reset schedule", destructive: true, onSelect: () => onScheduleAction("reset") },
        ]} />
        <Button variant="secondary" className={view.practiceHeaderButton} disabled={busy} onClick={onEdit}>Edit card</Button>
        <Button variant="danger" data-practice-action="end" className={view.practiceHeaderButton} disabled={busy} onClick={onEnd}>{isEnding ? "Ending…" : "End session"}</Button>
      </div>
    </header>
    <Progress className={view.practiceProgressTrack} label="Practice session progress" value={studiedCount} max={cardCount}
      variant="inline-content" inlineContent={{ start: `${studiedCount} studied · ${remainingCount} remaining`, end: `${percentComplete}%` }}
      valueText={`Card ${currentPosition} of ${cardCount}, ${studiedCount} studied, ${remainingCount} remaining, ${percentComplete}% complete`} />
    {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
    <div key={current.presentation.cardId} className={interactive ? "tw:grid tw:gap-5" : styles.stack}
      data-layers={interactive ? undefined : stackLayers} data-tour-anchor="practice.card">{cardSurface}</div>
    {!revealed ? interactive ? null : <div className={view.practicePrimaryAction}>
      <Button variant="primary" size="large" disabled={busy} onClick={onReveal}>{isRevealing ? "Calculating intervals…" : "Show answer"}</Button>
    </div> : <section ref={ratingPanelRef} tabIndex={-1} className={view.reviewRatingPanelConfidenceRatingPanel}
      aria-labelledby="confidence-rating-heading" data-tour-anchor="practice.confidence">
      <div className={view.confidenceRatingHeading}>
        <h2 id="confidence-rating-heading">{objectiveCorrect === false ? "First answer recorded as wrong" : "How difficult was that answer?"}</h2>
        <p>{objectiveCorrect === false
          ? "Continue with Wrong. The checked result remains immutable even if you practice the item again."
          : "Choose the result that best matches your recall."}</p>
      </div>
      {!previewSet ? <div className="tw:mb-3">
        <Button disabled={busy} onClick={onReveal}>{isRevealing ? "Calculating intervals…" : "Retry interval preview"}</Button>
      </div> : null}
      <div className={view.confidenceScoreActions}>
        <ConfidenceButton score={0} label="Wrong" disabled={busy || !previewSet || objectiveCorrect === true} onClick={() => onScore(0)} />
        <ConfidenceButton score={1} label="Hard" disabled={busy || !previewSet || objectiveCorrect === false} onClick={() => onScore(1)} />
        <ConfidenceButton score={2} label="Medium" disabled={busy || !previewSet || objectiveCorrect === false} onClick={() => onScore(2)} />
        <ConfidenceButton score={3} label="Easy" disabled={busy || !previewSet || objectiveCorrect === false} onClick={() => onScore(3)} />
      </div>
    </section>}
  </PageScaffold>;
}

function ConfidenceButton({ score, label, disabled, onClick }: {
  score: ConfidenceScore;
  label: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return <Button className={cn(view.confidenceButton, scoreColors[score])} disabled={disabled}
    aria-label={`${score} ${label}`} aria-keyshortcuts={String(score)} onClick={onClick}>
    <span className={view.confidenceButtonContent}>
      <span className={view.confidenceScoreNumber} aria-hidden="true">{score}</span>
      <span className={view.confidenceScoreCopy}><strong>{label}</strong></span>
    </span>
  </Button>;
}

function unavailableSave(_input: SavePracticeAnswerInput): Promise<QuestionAttemptView> {
  return Promise.reject(new Error("Interactive answer saving is unavailable in this host."));
}

function unavailableCheck(_input: CheckPracticeAnswerInput): Promise<QuestionAttemptView> {
  return Promise.reject(new Error("Interactive answer checking is unavailable in this host."));
}

function ignoreAttempt(_attempt: PracticeAttemptView): void {}
