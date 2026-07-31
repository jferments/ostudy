import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { PracticeLaunchRequest } from "../App";
import { RichTextView } from "../editor/RichTextView";
import {
  completePracticeSession,
  createSubmissionId,
  getErrorMessage,
  markPracticeCardRevealed,
  markPracticeCardViewed,
  startPractice,
  submitPracticeReview,
  type AppSettings,
  type Deck,
  type PracticeSession,
  type ReviewOutcome,
} from "../ipc";

interface PracticeScreenProps {
  decks: Deck[];
  settings: AppSettings;
  launchRequest: PracticeLaunchRequest | null;
  onLaunchConsumed: () => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

type RatingStep = "known" | "confidence";

/**
 * Builds scheduled review sessions from one or more decks.
 *
 * Deck-level Practice actions arrive as an auto-start request, while the
 * sidebar opens the selector. The distinction is kept in one screen so both
 * paths share the same session and error-handling code.
 */
export function PracticeScreen({
  decks,
  settings,
  launchRequest,
  onLaunchConsumed,
  showToast,
}: PracticeScreenProps) {
  const [selectedDeckIds, setSelectedDeckIds] = useState<Set<string>>(new Set());
  const [shuffle, setShuffle] = useState(settings.practiceShuffle);
  const [session, setSession] = useState<PracticeSession | null>(null);
  const [position, setPosition] = useState(0);
  const [revealed, setRevealed] = useState(false);
  const [ratingStep, setRatingStep] = useState<RatingStep>("known");
  const [isStarting, setIsStarting] = useState(false);
  const [isRevealing, setIsRevealing] = useState(false);
  const [isRating, setIsRating] = useState(false);
  const [isEnding, setIsEnding] = useState(false);
  const [completed, setCompleted] = useState(false);
  const [ratedCount, setRatedCount] = useState(0);
  const [correctCount, setCorrectCount] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const startingRef = useRef(false);
  const revealingRef = useRef(false);
  const ratingRef = useRef(false);
  const endingRef = useRef(false);
  const questionShownAtRef = useRef(Date.now());
  const handledLaunchRequestRef = useRef<number | null>(null);

  const decksWithCards = useMemo(
    () => decks.filter((deck) => deck.cardCount > 0),
    [decks],
  );
  const current = session?.cards[position] ?? null;
  const busy = isRevealing || isRating || isEnding;

  const beginSession = useCallback(
    async (deckIds: string[], requestedShuffle: boolean): Promise<void> => {
      if (startingRef.current || deckIds.length === 0) return;
      startingRef.current = true;
      setIsStarting(true);
      setErrorMessage(null);
      try {
        const started = await startPractice({
          deckIds,
          shuffle: requestedShuffle,
        });
        setSession(started);
        setPosition(0);
        setRevealed(false);
        setRatingStep("known");
        setCompleted(false);
        setRatedCount(0);
        setCorrectCount(0);
        questionShownAtRef.current = Date.now();
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

  useEffect(() => {
    if (!launchRequest) return;
    if (handledLaunchRequestRef.current === launchRequest.requestId) return;
    handledLaunchRequestRef.current = launchRequest.requestId;

    const eligible = launchRequest.deckIds.filter((deckId) =>
      decks.some((deck) => deck.id === deckId && deck.dueCount > 0),
    );
    setSelectedDeckIds(new Set(eligible));
    setSession(null);
    setCompleted(false);
    setErrorMessage(null);
    onLaunchConsumed();

    if (launchRequest.autoStart && eligible.length > 0) {
      void beginSession(eligible, settings.practiceShuffle);
    }
  }, [beginSession, decks, launchRequest, onLaunchConsumed, settings.practiceShuffle]);

  useEffect(() => {
    if (!session || !current || completed) return;
    questionShownAtRef.current = Date.now();
    void markPracticeCardViewed(session.id, current.card.id).catch((error: unknown) => {
      showToast("error", getErrorMessage(error));
    });
  }, [completed, current, session, showToast]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent): void {
      if (!session || !current || completed || busy) return;
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
        void revealAnswer();
        return;
      }
      if (!revealed) return;

      if (ratingStep === "known") {
        if (event.key === "1") {
          event.preventDefault();
          void submitOutcome("wrong");
        } else if (event.key.toLocaleLowerCase() === "r" || event.key === "Enter") {
          event.preventDefault();
          setRatingStep("confidence");
        }
        return;
      }

      if (event.key === "2") {
        event.preventDefault();
        void submitOutcome("not_confident");
      } else if (event.key === "3") {
        event.preventDefault();
        void submitOutcome("somewhat_confident");
      } else if (event.key === "4") {
        event.preventDefault();
        void submitOutcome("very_confident");
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  });

  async function revealAnswer(): Promise<void> {
    if (!session || !current || revealed || revealingRef.current) return;
    revealingRef.current = true;
    setIsRevealing(true);
    setErrorMessage(null);
    try {
      // Persist the reveal before showing the answer so the visible state and
      // review ledger cannot disagree after an unexpected application exit.
      await markPracticeCardRevealed(session.id, current.card.id);
      setRevealed(true);
      setRatingStep("known");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      revealingRef.current = false;
      setIsRevealing(false);
    }
  }

  async function submitOutcome(outcome: ReviewOutcome): Promise<void> {
    if (!session || !current || !revealed || ratingRef.current) return;
    ratingRef.current = true;
    setIsRating(true);
    setErrorMessage(null);
    try {
      const responseTimeMs = Math.max(
        0,
        Math.min(86_400_000, Date.now() - questionShownAtRef.current),
      );
      const result = await submitPracticeReview({
        sessionId: session.id,
        cardId: current.card.id,
        submissionId: createSubmissionId(),
        outcome,
        responseTimeMs,
      });
      setRatedCount(result.ratedCount);
      setCorrectCount(result.correctCount);

      if (result.completed || position + 1 >= session.cards.length) {
        setCompleted(true);
      } else {
        setPosition((value) => value + 1);
        setRevealed(false);
        setRatingStep("known");
      }
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      ratingRef.current = false;
      setIsRating(false);
    }
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
    setRatingStep("known");
    setCompleted(false);
    setRatedCount(0);
    setCorrectCount(0);
    setErrorMessage(null);
  }

  if (session && completed) {
    const accuracy = ratedCount === 0 ? 0 : Math.round((correctCount / ratedCount) * 100);
    return (
      <div className="screen practice-screen">
        <section className="practice-complete-card">
          <span className="completion-mark" aria-hidden="true">✓</span>
          <h1>Session complete</h1>
          <p>
            {correctCount} of {ratedCount} correct · {accuracy}%
          </p>
          <div className="completion-actions">
            <button type="button" className="secondary-button" onClick={resetToSetup}>
              Choose decks
            </button>
            <button
              type="button"
              className="primary-button"
              disabled={isStarting}
              onClick={() =>
                void beginSession([...selectedDeckIds], shuffle)
              }
            >
              {isStarting ? "Starting…" : "Practice due cards again"}
            </button>
          </div>
          {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
        </section>
      </div>
    );
  }

  if (session && current) {
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
          <button
            type="button"
            className="secondary-button"
            disabled={busy}
            onClick={() => void endSession()}
          >
            {isEnding ? "Ending…" : "End session"}
          </button>
        </header>

        <div className="practice-progress-track" aria-hidden="true">
          <span style={{ width: `${(ratedCount / session.cards.length) * 100}%` }} />
        </div>

        {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}

        <section className="practice-card-surface">
          <span className="practice-side-label">Question</span>
          <RichTextView
            document={current.card.frontDoc}
            assets={current.card.assets}
            onLinkError={setErrorMessage}
          />

          {revealed ? (
            <>
              <div className="practice-answer-divider" />
              <span className="practice-side-label">Answer</span>
              <RichTextView
                document={current.card.backDoc}
                assets={current.card.assets}
                onLinkError={setErrorMessage}
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
              onClick={() => void revealAnswer()}
            >
              {isRevealing ? "Revealing…" : "Show answer"}
            </button>
          </div>
        ) : ratingStep === "known" ? (
          <section className="review-rating-panel" aria-label="Rate this answer">
            <h2>Did you know it?</h2>
            <div className="binary-review-actions">
              <button
                type="button"
                className="review-button wrong"
                disabled={isRating}
                onClick={() => void submitOutcome("wrong")}
              >
                <strong>Wrong</strong>
                <span>1</span>
              </button>
              <button
                type="button"
                className="review-button right"
                disabled={isRating}
                onClick={() => setRatingStep("confidence")}
              >
                <strong>Right</strong>
                <span>R</span>
              </button>
            </div>
          </section>
        ) : (
          <section className="review-rating-panel" aria-label="Choose confidence">
            <div className="confidence-heading">
              <button
                type="button"
                className="text-button"
                disabled={isRating}
                onClick={() => setRatingStep("known")}
              >
                ← Back
              </button>
              <h2>How confident were you?</h2>
            </div>
            <div className="confidence-review-actions">
              <ConfidenceButton
                shortcut="2"
                label="Not confident"
                disabled={isRating}
                onClick={() => void submitOutcome("not_confident")}
              />
              <ConfidenceButton
                shortcut="3"
                label="Somewhat confident"
                disabled={isRating}
                onClick={() => void submitOutcome("somewhat_confident")}
              />
              <ConfidenceButton
                shortcut="4"
                label="Very confident"
                disabled={isRating}
                onClick={() => void submitOutcome("very_confident")}
              />
            </div>
          </section>
        )}
      </div>
    );
  }

  return (
    <div className="screen practice-screen">
      <header className="screen-header compact-screen-header">
        <div>
          <h1>Practice</h1>
          <p>Select one or more decks. Only cards that are currently due are included.</p>
        </div>
      </header>

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}

      {decksWithCards.length === 0 ? (
        <section className="compact-empty-state large-empty-state">
          <h2>No flashcards yet</h2>
          <p>Add a card to a deck before starting a session.</p>
        </section>
      ) : (
        <div className="practice-setup-grid">
          <section className="settings-panel practice-deck-panel">
            <div className="panel-heading-row">
              <div>
                <h2>Choose decks</h2>
                <span>{selectedDeckIds.size} selected</span>
              </div>
              <div className="inline-actions">
                <button
                  type="button"
                  className="text-button"
                  onClick={() => setSelectedDeckIds(new Set(decksWithCards.filter((deck) => deck.dueCount > 0).map((deck) => deck.id)))}
                >
                  Select all
                </button>
                {selectedDeckIds.size > 0 ? (
                  <button
                    type="button"
                    className="text-button"
                    onClick={() => setSelectedDeckIds(new Set())}
                  >
                    Clear
                  </button>
                ) : null}
              </div>
            </div>

            <div className="practice-deck-list">
              {decksWithCards.map((deck) => {
                const checked = selectedDeckIds.has(deck.id);
                return (
                  <label key={deck.id} className={["practice-deck-choice", checked ? "selected" : "", deck.dueCount === 0 ? "disabled" : ""].filter(Boolean).join(" ")}>
                    <input
                      type="checkbox"
                      checked={checked}
                      disabled={deck.dueCount === 0}
                      onChange={(event) => {
                        // Capture the primitive before entering the state updater.
                        // React does not guarantee that an event's currentTarget is
                        // still available when an updater is evaluated.
                        const nextChecked = event.currentTarget.checked;
                        setSelectedDeckIds((currentIds) => {
                          const next = new Set(currentIds);
                          if (nextChecked) next.add(deck.id);
                          else next.delete(deck.id);
                          return next;
                        });
                      }}
                    />
                    <span>
                      <strong>{deck.name}</strong>
                      <small>
                        {deck.dueCount} due · {deck.cardCount} total
                      </small>
                    </span>
                  </label>
                );
              })}
            </div>
          </section>

          <section className="settings-panel practice-options-panel">
            <h2>Session options</h2>
            <label className="toggle-row">
              <span>
                <strong>Shuffle cards</strong>
                <small>Mix cards from all selected decks.</small>
              </span>
              <input
                type="checkbox"
                checked={shuffle}
                onChange={(event) => setShuffle(event.currentTarget.checked)}
              />
            </label>
            <div className="practice-availability">
              <strong>{selectedDueCount(decks, selectedDeckIds)}</strong>
              <span>cards due</span>
            </div>
            <button
              type="button"
              className="primary-button full-width-button"
              disabled={selectedDeckIds.size === 0 || selectedDueCount(decks, selectedDeckIds) === 0 || isStarting}
              onClick={() => void beginSession([...selectedDeckIds], shuffle)}
            >
              {isStarting ? "Starting…" : "Start practice session"}
            </button>
          </section>
        </div>
      )}
    </div>
  );
}

interface ConfidenceButtonProps {
  shortcut: string;
  label: string;
  disabled: boolean;
  onClick: () => void;
}

function ConfidenceButton({ shortcut, label, disabled, onClick }: ConfidenceButtonProps) {
  return (
    <button
      type="button"
      className="review-button confidence"
      disabled={disabled}
      onClick={onClick}
    >
      <strong>{label}</strong>
      <span>{shortcut}</span>
    </button>
  );
}

function selectedDueCount(decks: Deck[], selectedDeckIds: Set<string>): number {
  return decks.reduce(
    (total, deck) => total + (selectedDeckIds.has(deck.id) ? deck.dueCount : 0),
    0,
  );
}
