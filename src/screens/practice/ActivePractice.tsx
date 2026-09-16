import { useEffect, useRef, useState } from "react";
import { TutorEntryButton } from "../tutor/TutorEntryButton";
import { RichTextView } from "../../editor/RichTextView";
import {
  isInteractivePracticePresentation,
  type PracticeCard,
  type PracticeSession,
  type ReviewPreviewSet,
} from "../../ipc/practice";
import type {
  CheckPracticeAnswerInput,
  QuestionAttemptView,
  SavePracticeAnswerInput,
} from "../../ipc/questionAttempts";
import type { RescheduleIntent, ScheduleAction } from "../../ipc/scheduling";
import { useAppearance } from "../../ui/appearance/AppearanceProvider";
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
  onQuestionAttempt?: (attempt: QuestionAttemptView) => void;
  onQuestionChecked?: (attempt: QuestionAttemptView) => void | Promise<void>;
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
  onQuestionAttempt = ignoreAttempt,
  onQuestionChecked = ignoreAttempt,
}: ActivePracticeProps) {
  const ratingPanelRef = useRef<HTMLElement | null>(null);
  const wasRevealed = useRef(revealed);
  const [showingAnswer, setShowingAnswer] = useState(revealed);
  const busy = isRevealing || isRating || isEnding || isManaging;
  const { topId } = useOverlayContext();
  const appearance = useAppearance();
  const experience = useExperience();
  const animateFlip = flipAnimationEnabled ?? appearance.motion === "full";
  const cardCount = Math.max(0, session.cardCount);
  const studiedCount = Math.min(Math.max(0, ratedCount), cardCount);
  const remainingCount = Math.max(0, cardCount - studiedCount);
  const percentComplete = cardCount > 0 ? Math.round((studiedCount / cardCount) * 100) : 0;
  const currentPosition = cardCount > 0 ? Math.min(Math.max(1, position + 1), cardCount) : 0;
  const stackLayers = remainingCount > 3 ? 2 : remainingCount > 1 ? 1 : 0;
  const interactivePresentation = isInteractivePracticePresentation(current.presentation)
    ? current.presentation
    : null;
  const recallPresentation = interactivePresentation ? null : current.presentation;
  const interactive = interactivePresentation !== null;
  const normalizedQuestion = current.presentation.frontText.trim();
  const backAlreadyIncludesQuestion = recallPresentation !== null
    && normalizedQuestion.length > 0
    && recallPresentation.backText.includes(normalizedQuestion);
  const combineQuestionAndAnswer = recallPresentation !== null
    && displayQuestionOnAnswer
    && !backAlreadyIncludesQuestion;
  const checkedProgress = interactivePresentation?.question.attempt.progress.phase === "checked"
    ? interactivePresentation.question.attempt.progress
    : null;
  const objectiveCorrect = checkedProgress
    ? checkedProgress.evaluation.verdict === "correct"
    : null;

  useEffect(() => {
    setShowingAnswer(revealed);
  }, [current.presentation.cardId, revealed]);

  useEffect(() => {
    if (revealed) ratingPanelRef.current?.focus();
    if (revealed && !wasRevealed.current) experience.emit({ type: "answer_revealed" });
    wasRevealed.current = revealed;
  }, [experience, revealed]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      if (busy || event.repeat || topId !== null) return;
      const target = event.target;
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

  function activateAnimatedCard(): void {
    if (busy || interactive) return;
    if (!revealed) {
      onReveal();
      return;
    }
    setShowingAnswer((side) => !side);
  }

  function questionContent(mediaPlaybackActive: boolean) {
    if (!recallPresentation) return null;
    return <>
      <span className={view.practiceSideLabel}>Question</span>
      <RichTextView document={recallPresentation.frontDoc} assets={recallPresentation.assets}
        mediaPlaybackActive={mediaPlaybackActive} onLinkError={onLinkError} />
    </>;
  }

  function answerContent(mediaPlaybackActive: boolean) {
    if (!recallPresentation) return null;
    return <>
      <span className={view.practiceSideLabel}>Answer</span>
      <RichTextView document={recallPresentation.backDoc} assets={recallPresentation.assets}
        mediaPlaybackActive={mediaPlaybackActive} onLinkError={onLinkError} />
    </>;
  }

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
      onAttempt={onQuestionAttempt}
      onChecked={onQuestionChecked}
      onError={onLinkError}
    />
  ) : animateFlip ? (
    <section className={cn(styles.cardSurface, view.cardSurface, styles.flipCard)} data-flipped={showingAnswer}
      role="button" tabIndex={0} aria-label={!revealed ? "Reveal answer" : showingAnswer ? "Show question" : "Show answer"}
      aria-disabled={busy ? "true" : undefined}
      onClick={(event) => {
        const target = event.target;
        if (target instanceof HTMLElement && target.closest("a, button, input, textarea, select, audio, video")) return;
        activateAnimatedCard();
      }}
      onKeyDown={(event) => {
        if (event.key !== " " && event.key !== "Enter") return;
        if (event.target !== event.currentTarget) return;
        event.preventDefault();
        event.stopPropagation();
        activateAnimatedCard();
      }}>
      <div className={styles.flipInner}>
        <div className={cn(styles.flipFace, styles.flipFront)} aria-hidden={showingAnswer} inert={showingAnswer}>
          {questionContent(!showingAnswer)}
          {!showingAnswer ? <small className={view.practiceFlipHint}>Click the card to flip it</small> : null}
        </div>
        <div className={cn(styles.flipFace, styles.flipBack)} aria-hidden={!showingAnswer} inert={!showingAnswer}>
          {combineQuestionAndAnswer ? <>{questionContent(showingAnswer)}<div className={view.practiceAnswerDivider} /></> : null}
          {answerContent(showingAnswer)}
        </div>
      </div>
    </section>
  ) : (
    <section className={cn(styles.cardSurface, view.cardSurface)}>
      {!revealed || combineQuestionAndAnswer ? questionContent(!revealed || combineQuestionAndAnswer) : null}
      {revealed ? <>{combineQuestionAndAnswer ? <div className={view.practiceAnswerDivider} /> : null}{answerContent(true)}</> : null}
    </section>
  );

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

function ignoreAttempt(_attempt: QuestionAttemptView): void {}
