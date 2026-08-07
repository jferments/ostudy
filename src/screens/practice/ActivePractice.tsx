import { useEffect, useRef } from "react";

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
import type { ConfidenceScore } from "./confidence";

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
  errorMessage: string | null;
  onReveal: () => void;
  onScore: (score: ConfidenceScore) => void;
  onScheduleAction: (action: ScheduleAction, intent?: RescheduleIntent) => void;
  onEnd: () => void;
  onLinkError: (message: string) => void;
}

const outcomes = ["wrong", "not_confident", "somewhat_confident", "very_confident"] as const;

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
  errorMessage,
  onReveal,
  onScore,
  onScheduleAction,
  onEnd,
  onLinkError,
}: ActivePracticeProps) {
  const ratingPanelRef = useRef<HTMLElement | null>(null);
  const busy = isRevealing || isRating || isEnding || isManaging;
  const progress = session.cardCount === 0 ? 0 : position / session.cardCount;

  useEffect(() => {
    if (revealed) ratingPanelRef.current?.focus();
  }, [revealed]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      if (busy || event.repeat) return;
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
  }, [busy, onReveal, onScore, revealed]);

  const previewFor = (score: ConfidenceScore) =>
    previewSet?.previews.find((preview) => preview.outcome === outcomes[score]);

  return (
    <div className="screen practice-screen active-practice-screen">
      <header className="practice-session-header">
        <div>
          <span className="eyebrow">
            {session.mode === "extra"
              ? "EXTRA PRACTICE"
              : session.mode === "custom"
                ? "CUSTOM PRACTICE"
                : "PRACTICE"}
          </span>
          <h1>{current.deckName}</h1>
          {current.needsMorePractice ? (
            <span className="schedule-status-badge needs-practice">Needs more practice</span>
          ) : null}
        </div>
        <div className="practice-progress-copy">
          <strong>
            {position + 1} / {session.cardCount}
          </strong>
          <span>{ratedCount} reviewed</span>
        </div>
        <div className="practice-header-actions">
          <details className="practice-more-menu">
            <summary>More</summary>
            <div>
              <button type="button" disabled={busy} onClick={() => onScheduleAction("bury")}>
                Bury until tomorrow
              </button>
              <button type="button" disabled={busy} onClick={() => onScheduleAction("suspend")}>
                Suspend card
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={() => onScheduleAction("reschedule", { kind: "tomorrow" })}
              >
                Reschedule for tomorrow
              </button>
              <button
                type="button"
                className="danger-text"
                disabled={busy}
                onClick={() => onScheduleAction("reset")}
              >
                Reset schedule
              </button>
            </div>
          </details>
          <button
            type="button"
            className="secondary-button"
            disabled={busy}
            onClick={onEnd}
          >
            {isEnding ? "Ending…" : "End session"}
          </button>
        </div>
      </header>

      <div
        className="practice-progress-track"
        role="progressbar"
        aria-label="Practice session progress"
        aria-valuemin={0}
        aria-valuemax={session.cardCount}
        aria-valuenow={position}
      >
        <span style={{ width: `${progress * 100}%` }} />
      </div>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}

      <section className="practice-card-surface">
        <span className="practice-side-label">Question</span>
        <RichTextView
          document={current.presentation.frontDoc}
          assets={current.presentation.assets}
          onLinkError={onLinkError}
        />
        {revealed ? (
          <>
            <div className="practice-answer-divider" />
            <span className="practice-side-label">Answer</span>
            <RichTextView
              document={current.presentation.backDoc}
              assets={current.presentation.assets}
              onLinkError={onLinkError}
            />
          </>
        ) : null}
      </section>

      {!revealed ? (
        <div className="practice-primary-action">
          <button
            type="button"
            className="primary-button large-action-button"
            disabled={isRevealing}
            onClick={onReveal}
          >
            {isRevealing ? "Calculating intervals…" : "Show answer"}
          </button>
        </div>
      ) : (
        <section
          ref={ratingPanelRef}
          tabIndex={-1}
          className="review-rating-panel confidence-rating-panel"
          aria-labelledby="confidence-rating-heading"
        >
          <div className="confidence-rating-heading">
            <h2 id="confidence-rating-heading">How difficult was that answer?</h2>
            <p>Approximate next intervals are calculated by FSRS. Keyboard shortcuts: 0–3.</p>
          </div>
          <div className="confidence-score-actions">
            <ConfidenceButton
              score={0}
              label="Wrong"
              detail="I could not recall it."
              interval={formatInterval(previewFor(0)?.intervalMs)}
              disabled={isRating || !previewSet}
              onClick={() => onScore(0)}
            />
            <ConfidenceButton
              score={1}
              label="Hard"
              detail="I recalled it with major difficulty."
              interval={formatInterval(previewFor(1)?.intervalMs)}
              disabled={isRating || !previewSet}
              onClick={() => onScore(1)}
            />
            <ConfidenceButton
              score={2}
              label="Medium"
              detail="I recalled it with some effort."
              interval={formatInterval(previewFor(2)?.intervalMs)}
              disabled={isRating || !previewSet}
              onClick={() => onScore(2)}
            />
            <ConfidenceButton
              score={3}
              label="Easy"
              detail="The answer came immediately."
              interval={formatInterval(previewFor(3)?.intervalMs)}
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
  detail,
  interval,
  disabled,
  onClick,
}: {
  score: ConfidenceScore;
  label: string;
  detail: string;
  interval: string;
  disabled: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      className={`confidence-score-button confidence-score-${score}`}
      disabled={disabled}
      aria-label={`${score} ${label}, ${interval}`}
      aria-keyshortcuts={String(score)}
      onClick={onClick}
    >
      <span className="confidence-score-number" aria-hidden="true">
        {score}
      </span>
      <span className="confidence-score-copy">
        <strong>{label}</strong>
        <small>{detail}</small>
        <em>{interval}</em>
      </span>
      <kbd aria-hidden="true">{score}</kbd>
    </button>
  );
}

function formatInterval(value: number | undefined): string {
  if (value === undefined) return "Calculating…";
  if (value < 60_000) return "Less than 1 minute";
  if (value < 3_600_000) return `About ${Math.max(1, Math.round(value / 60_000))} min`;
  if (value < 86_400_000) return `About ${Math.max(1, Math.round(value / 3_600_000))} hr`;
  const days = Math.max(1, Math.round(value / 86_400_000));
  if (days < 30) return `About ${days} ${days === 1 ? "day" : "days"}`;
  const months = Math.max(1, Math.round(days / 30));
  return `About ${months} ${months === 1 ? "month" : "months"}`;
}
