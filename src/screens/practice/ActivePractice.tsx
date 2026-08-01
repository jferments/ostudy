import { useEffect, useRef } from "react";

import { RichTextView } from "../../editor/RichTextView";
import type { PracticeSession } from "../../ipc";
import type { ConfidenceScore } from "./confidence";

interface ActivePracticeProps {
  session: PracticeSession;
  position: number;
  revealed: boolean;
  ratedCount: number;
  isRevealing: boolean;
  isRating: boolean;
  isEnding: boolean;
  errorMessage: string | null;
  onReveal: () => void;
  onScore: (score: ConfidenceScore) => void;
  onEnd: () => void;
  onLinkError: (message: string) => void;
}

export function ActivePractice({
  session,
  position,
  revealed,
  ratedCount,
  isRevealing,
  isRating,
  isEnding,
  errorMessage,
  onReveal,
  onScore,
  onEnd,
  onLinkError,
}: ActivePracticeProps) {
  const current = session.cards[position];
  const ratingPanelRef = useRef<HTMLElement | null>(null);
  const busy = isRevealing || isRating || isEnding;
  const progress = session.cards.length === 0 ? 0 : ratedCount / session.cards.length;

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
          ["INPUT", "TEXTAREA", "SELECT", "BUTTON", "A"].includes(target.tagName))
      ) {
        return;
      }

      if (!revealed && (event.key === " " || event.key === "Enter")) {
        event.preventDefault();
        onReveal();
        return;
      }
      if (!revealed) return;

      if (["0", "1", "2", "3"].includes(event.key)) {
        event.preventDefault();
        onScore(Number(event.key) as ConfidenceScore);
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [busy, onReveal, onScore, revealed]);

  return (
    <div className="screen practice-screen active-practice-screen">
      <header className="practice-session-header">
        <div>
          <span className="eyebrow">PRACTICE</span>
          <h1>{current.deckName}</h1>
        </div>
        <div className="practice-progress-copy">
          <strong>{position + 1} / {session.cards.length}</strong>
          <span>{ratedCount} reviewed</span>
        </div>
        <button type="button" className="secondary-button" disabled={busy} onClick={onEnd}>
          {isEnding ? "Ending…" : "End session"}
        </button>
      </header>

      <div
        className="practice-progress-track"
        role="progressbar"
        aria-label="Practice session progress"
        aria-valuemin={0}
        aria-valuemax={session.cards.length}
        aria-valuenow={ratedCount}
      >
        <span style={{ width: `${progress * 100}%` }} />
      </div>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}

      <section className="practice-card-surface">
        <span className="practice-side-label">Question</span>
        <RichTextView
          document={current.card.frontDoc}
          assets={current.card.assets}
          onLinkError={onLinkError}
        />

        {revealed ? (
          <>
            <div className="practice-answer-divider" />
            <span className="practice-side-label">Answer</span>
            <RichTextView
              document={current.card.backDoc}
              assets={current.card.assets}
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
            {isRevealing ? "Revealing…" : "Show answer"}
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
            <p>Choose one score. Keyboard shortcuts: 0, 1, 2, or 3.</p>
          </div>
          <div className="confidence-score-actions">
            <ConfidenceButton score={0} label="Wrong" detail="I could not recall it." disabled={isRating} onClick={() => onScore(0)} />
            <ConfidenceButton score={1} label="Hard" detail="I recalled it with major difficulty." disabled={isRating} onClick={() => onScore(1)} />
            <ConfidenceButton score={2} label="Medium" detail="I recalled it with some effort." disabled={isRating} onClick={() => onScore(2)} />
            <ConfidenceButton score={3} label="Easy" detail="The answer came immediately." disabled={isRating} onClick={() => onScore(3)} />
          </div>
        </section>
      )}
    </div>
  );
}

interface ConfidenceButtonProps {
  score: ConfidenceScore;
  label: string;
  detail: string;
  disabled: boolean;
  onClick: () => void;
}

function ConfidenceButton({ score, label, detail, disabled, onClick }: ConfidenceButtonProps) {
  return (
    <button
      type="button"
      className={`confidence-score-button confidence-score-${score}`}
      disabled={disabled}
      aria-label={`${score} ${label}`}
      aria-keyshortcuts={String(score)}
      onClick={onClick}
    >
      <span className="confidence-score-number" aria-hidden="true">{score}</span>
      <span className="confidence-score-copy">
        <strong>{label}</strong>
        <small>{detail}</small>
      </span>
      <kbd aria-hidden="true">{score}</kbd>
    </button>
  );
}
