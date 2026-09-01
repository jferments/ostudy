import { useEffect, useRef, useState } from "react";

import { RichTextView } from "../../editor/RichTextView";
import {
  type PracticeCard,
  type PracticeSession,
  type ReviewPreviewSet,
} from "../../ipc/practice";
import {
  type RescheduleIntent,
  type ScheduleAction,
} from "../../ipc/scheduling";
import { useAppearance } from "../../ui/appearance/AppearanceProvider";
import { useExperience } from "../../ui/feedback/ExperienceProvider";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { useOverlayContext } from "../../ui/overlays/overlayContext";
import { Progress } from "../../ui/primitives/Progress";
import type { ConfidenceScore } from "./confidence";
import styles from "./ActivePractice.module.css";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ownedClassNames, sharedClassMaps } from "../../ui/styles/classOwnership";
import ownershipStyles from "./Practice.module.css";

const ownedStyleMaps = [...sharedClassMaps, ownershipStyles] as const;


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
  errorMessage: string | null;
  onReveal: () => void;
  onScore: (score: ConfidenceScore) => void;
  onEdit: () => void;
  onScheduleAction: (action: ScheduleAction, intent?: RescheduleIntent) => void;
  onEnd: () => void;
  onLinkError: (message: string) => void;
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
  errorMessage,
  onReveal,
  onScore,
  onEdit,
  onScheduleAction,
  onEnd,
  onLinkError,
}: ActivePracticeProps) {
  const ratingPanelRef = useRef<HTMLElement | null>(null);
  const wasRevealed = useRef(revealed);
  const [showingAnswer, setShowingAnswer] = useState(revealed);
  const busy = isRevealing || isRating || isEnding || isManaging;
  const { topId } = useOverlayContext();
  const appearance = useAppearance();
  const experience = useExperience();
  const animateFlip = flipAnimationEnabled?? appearance.motion === "full";
  const cardCount = Math.max(0, session.cardCount);
  const studiedCount = Math.min(Math.max(0, ratedCount), cardCount);
  const remainingCount = Math.max(0, cardCount - studiedCount);
  const percentComplete = cardCount > 0 ? Math.round((studiedCount / cardCount) * 100) : 0;
  const currentPosition = cardCount > 0
    ? Math.min(Math.max(1, position + 1), cardCount)
    : 0;
  const stackLayers = remainingCount > 3 ? 2 : remainingCount > 1 ? 1 : 0;

  useEffect(() => {
    setShowingAnswer(revealed);
  }, [current.presentation.cardId, revealed]);

  useEffect(() => {
    if (revealed) ratingPanelRef.current?.focus();
    if (revealed && !wasRevealed.current) {
      experience.emit({ type: "answer_revealed" });
    }
    wasRevealed.current = revealed;
  }, [experience, revealed]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      if (busy || event.repeat || topId !== null) return;
      const target = event.target;
      if (
        target instanceof HTMLElement &&
        (target.isContentEditable ||
          ["INPUT", "TEXTAREA", "SELECT", "BUTTON", "A", "SUMMARY"].includes(target.tagName))
      ) {
        return;
      }
      if (!revealed && (event.key === " " || event.key === "Enter")) {
        event.preventDefault();
        onReveal();
        return;
      }
      if (revealed && ["0", "1", "2", "3"].includes(event.key)) {
        event.preventDefault();
        onScore(Number(event.key) as ConfidenceScore);
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [busy, onReveal, onScore, revealed, topId]);

  function activateAnimatedCard(): void {
    if (busy) return;
    if (!revealed) {
      onReveal();
      return;
    }
    setShowingAnswer((currentSide) => !currentSide);
  }

  const cardSurface = animateFlip ? (
    <section
      className={ownedClassNames(ownedStyleMaps, `${styles.cardSurface} practice-card-surface practice-flip-card`)}
      data-flipped={showingAnswer}
      role="button"
      tabIndex={0}
      aria-label={!revealed ? "Reveal answer" : showingAnswer ? "Show question" : "Show answer"}
      aria-disabled={busy ? "true" : undefined}
      onClick={(event) => {
        const target = event.target;
        if (
          target instanceof HTMLElement &&
          target.closest("a, button, input, textarea, select")
        ) {
          return;
        }
        activateAnimatedCard();
      }}
      onKeyDown={(event) => {
        if (event.key !== " " && event.key !== "Enter") return;
        event.preventDefault();
        event.stopPropagation();
        activateAnimatedCard();
      }}
    >
      <div className={ownedClassNames(ownedStyleMaps, "practice-flip-inner")}>
        <div className={ownedClassNames(ownedStyleMaps, "practice-flip-face practice-flip-front")} aria-hidden={showingAnswer}>
          <span className={ownedClassNames(ownedStyleMaps, "practice-side-label")}>Question</span>
          <RichTextView
            document={current.presentation.frontDoc}
            assets={current.presentation.assets}
            onLinkError={onLinkError}
          />
          {!showingAnswer ? (
            <small className={ownedClassNames(ownedStyleMaps, "practice-flip-hint")}>Click the card to flip it</small>
           ) : null}
        </div>
        <div className={ownedClassNames(ownedStyleMaps, "practice-flip-face practice-flip-back")} aria-hidden={!showingAnswer}>
          <span className={ownedClassNames(ownedStyleMaps, "practice-side-label")}>Answer</span>
          <RichTextView
            document={current.presentation.backDoc}
            assets={current.presentation.assets}
            onLinkError={onLinkError}
          />
        </div>
      </div>
    </section>
  ) : (
    <section className={ownedClassNames(ownedStyleMaps, `${styles.cardSurface} practice-card-surface`)}>
      <span className={ownedClassNames(ownedStyleMaps, "practice-side-label")}>Question</span>
      <RichTextView
        document={current.presentation.frontDoc}
        assets={current.presentation.assets}
        onLinkError={onLinkError}
      />
      {revealed ? (
        <>
          <div className={ownedClassNames(ownedStyleMaps, "practice-answer-divider")} />
          <span className={ownedClassNames(ownedStyleMaps, "practice-side-label")}>Answer</span>
          <RichTextView
            document={current.presentation.backDoc}
            assets={current.presentation.assets}
            onLinkError={onLinkError}
          />
        </>
      ) : null}
    </section>
  );

  return (
    <div className={ownedClassNames(ownedStyleMaps, "screen practice-screen active-practice-screen")}>
      <header className={ownedClassNames(ownedStyleMaps, "practice-session-header")}>
        <div className={ownedClassNames(ownedStyleMaps, "practice-session-title")}>
          <span className={ownedClassNames(ownedStyleMaps, "eyebrow")}>
            {session.mode === "extra"
              ? "EXTRA PRACTICE"
              : session.mode === "custom"
                ? "CUSTOM PRACTICE"
                : "PRACTICE"}
          </span>
          <h1>{current.deckName}</h1>
        </div>
        <div className={ownedClassNames(ownedStyleMaps, "practice-header-actions")}>
          <Button
            type="button"
            variant="secondary" className={ownedClassNames(ownedStyleMaps, "practice-header-button")}
            disabled={busy}
            onClick={onEdit}
          >
            Edit card
          </Button>
          <OverflowMenu
            label="Schedule"
            className={ownedClassNames(ownedStyleMaps, "practice-header-button")}
            disabled={busy}
            items={[
              {
                id: "bury",
                label: "Bury until tomorrow",
                onSelect: () => onScheduleAction("bury"),
              },
              {
                id: "suspend",
                label: "Suspend card",
                onSelect: () => onScheduleAction("suspend"),
              },
              {
                id: "reschedule-tomorrow",
                label: "Reschedule for tomorrow",
                onSelect: () => onScheduleAction("reschedule", { kind: "tomorrow" }),
              },
              {
                id: "reset",
                label: "Reset schedule",
                destructive: true,
                onSelect: () => onScheduleAction("reset"),
              },
            ]}
          />
          <button
            type="button"
            className={ownedClassNames(ownedStyleMaps, "practice-end-button practice-header-button")}
            disabled={busy}
            onClick={onEnd}
          >
            {isEnding ? "Ending…" : "End session"}
          </button>
        </div>
      </header>

      <Progress
        className={ownedClassNames(ownedStyleMaps, "practice-progress-track")}
        label="Practice session progress"
        value={studiedCount}
        max={cardCount}
        variant="inline-content"
        inlineContent={{
          start: `${studiedCount} studied · ${remainingCount} remaining`,
          end: `${percentComplete}%`,
        }}
        valueText={`Card ${currentPosition} of ${cardCount}, ${studiedCount} studied, ${remainingCount} remaining, ${percentComplete}% complete`}
      />

      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}

      <div
        key={current.presentation.cardId}
        className={styles.stack}
        data-layers={stackLayers}
        data-tour-anchor="practice.card"
      >
        {cardSurface}
      </div>

      {!revealed ? (
        <div className={ownedClassNames(ownedStyleMaps, "practice-primary-action")}>
          <Button
            type="button"
            variant="primary" size="large"
            disabled={isRevealing}
            onClick={onReveal}
          >
            {isRevealing ? "Calculating intervals…" : "Show answer"}
          </Button>
        </div>
      ) : (
        <section
          ref={ratingPanelRef}
          tabIndex={-1}
          className={ownedClassNames(ownedStyleMaps, "review-rating-panel confidence-rating-panel")}
          aria-labelledby="confidence-rating-heading"
          data-tour-anchor="practice.confidence"
        >
          <div className={ownedClassNames(ownedStyleMaps, "confidence-rating-heading")}>
            <h2 id="confidence-rating-heading">How difficult was that answer?</h2>
            <p>Choose the result that best matches your recall.</p>
          </div>
          <div className={ownedClassNames(ownedStyleMaps, "confidence-score-actions")}>
            <ConfidenceButton
              score={0}
              label="Wrong"
              disabled={isRating || !previewSet}
              onClick={() => onScore(0)}
            />
            <ConfidenceButton
              score={1}
              label="Hard"
              disabled={isRating || !previewSet}
              onClick={() => onScore(1)}
            />
            <ConfidenceButton
              score={2}
              label="Medium"
              disabled={isRating || !previewSet}
              onClick={() => onScore(2)}
            />
            <ConfidenceButton
              score={3}
              label="Easy"
              disabled={isRating || !previewSet}
              onClick={() => onScore(3)}
            />
          </div>
        </section>
      )}
    </div>
  );
}

function ConfidenceButton({
  score,
  label,
  disabled,
  onClick,
}: {
  score: ConfidenceScore;
  label: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={ownedClassNames(ownedStyleMaps, `confidence-score-button confidence-score-${score}`)}
      disabled={disabled}
      aria-label={`${score} ${label}`}
      aria-keyshortcuts={String(score)}
      onClick={onClick}
    >
      <span className={ownedClassNames(ownedStyleMaps, "confidence-score-number")} aria-hidden="true">
        {score}
      </span>
      <span className={ownedClassNames(ownedStyleMaps, "confidence-score-copy")}>
        <strong>{label}</strong>
      </span>
    </button>
  );
}
