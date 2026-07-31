import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { PracticeLaunchRequest } from "../App";
import { RichTextView } from "../editor/RichTextView";
import {
  completePracticeSession,
  createSubmissionId,
  getAnalytics,
  getErrorCode,
  getErrorMessage,
  markPracticeCardRevealed,
  markPracticeCardViewed,
  previewReviewIntervals,
  startStudy,
  submitStudyReview,
  undoLastStudyReview,
  type AppSettings,
  type Deck,
  type IntervalPreview,
  type RecentDeckPractice,
  type ReviewScore,
  type StudyReviewResult,
  type StudySession,
} from "../ipc";

interface PracticeScreenProps {
  decks: Deck[];
  settings: AppSettings;
  launchRequest: PracticeLaunchRequest | null;
  onLaunchConsumed: () => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

type PracticeView = "setup" | "session" | "complete";

const SCORE_OPTIONS: Array<{
  score: ReviewScore;
  label: string;
  description: string;
  keyLabel: string;
  className: string;
}> = [
  { score: 0, label: "Wrong", description: "Could not answer", keyLabel: "0", className: "wrong" },
  { score: 1, label: "Hard", description: "Answered with difficulty", keyLabel: "1", className: "hard" },
  { score: 2, label: "OK", description: "Answered adequately", keyLabel: "2", className: "ok" },
  { score: 3, label: "Easy", description: "Answered immediately", keyLabel: "3", className: "easy" },
];

export function PracticeScreen({
  decks,
  settings,
  launchRequest,
  onLaunchConsumed,
  showToast,
}: PracticeScreenProps) {
  const [view, setView] = useState<PracticeView>("setup");
  const [selectedDeckIds, setSelectedDeckIds] = useState<string[]>([]);
  const [shuffle, setShuffle] = useState(settings.practiceShuffle);
  const [session, setSession] = useState<StudySession | null>(null);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [answerRevealed, setAnswerRevealed] = useState(false);
  const [intervals, setIntervals] = useState<IntervalPreview[]>([]);
  const [ratedCount, setRatedCount] = useState(0);
  const [correctCount, setCorrectCount] = useState(0);
  const [lastResult, setLastResult] = useState<StudyReviewResult | null>(null);
  const [isStarting, setIsStarting] = useState(false);
  const [isRevealing, setIsRevealing] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isUndoing, setIsUndoing] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState("");
  const [recentPractice, setRecentPractice] = useState<RecentDeckPractice[]>([]);
  const [isLoadingRecentPractice, setIsLoadingRecentPractice] = useState(false);
  const shownAtRef = useRef(Date.now());
  const startRef = useRef(false);
  const lastLaunchRef = useRef<number | null>(null);
  const restoreRevealedCardRef = useRef<string | null>(null);

  const current = session?.cards[currentIndex] ?? null;
  const intervalByScore = useMemo(
    () => new Map(intervals.map((interval) => [interval.score, interval])),
    [intervals],
  );

  const startSession = useCallback(async (
    deckIds: string[],
    selectedShuffle: boolean,
  ): Promise<void> => {
    if (startRef.current || deckIds.length === 0) return;
    startRef.current = true;
    setIsStarting(true);
    setErrorMessage(null);
    try {
      const { dayStartMs, nextDayStartMs } = localDayBoundaries();
      const started = await startStudy({
        deckIds,
        shuffle: selectedShuffle,
        dayStartMs,
        nextDayStartMs,
      });
      setSession(started);
      setCurrentIndex(0);
      setRatedCount(0);
      setCorrectCount(0);
      setLastResult(null);
      setAnswerRevealed(false);
      setIntervals([]);
      shownAtRef.current = Date.now();
      setView("session");
      setAnnouncement(`Practice started with ${started.cards.length} cards.`);
    } catch (error: unknown) {
      const code = getErrorCode(error);
      const message = getErrorMessage(error);
      setErrorMessage(message);
      if (code === "no_cards_due" || code === "daily_limit_reached") {
        showToast("info", message);
      }
      setView("setup");
    } finally {
      startRef.current = false;
      setIsStarting(false);
    }
  }, [showToast]);

  useEffect(() => {
    setShuffle(settings.practiceShuffle);
  }, [settings.practiceShuffle]);

  useEffect(() => {
    if (view !== "setup") return undefined;
    let cancelled = false;
    setIsLoadingRecentPractice(true);
    void getAnalytics()
      .then((analytics) => {
        if (!cancelled) setRecentPractice(analytics.recentDeckPractice);
      })
      .catch(() => {
        if (!cancelled) setRecentPractice([]);
      })
      .finally(() => {
        if (!cancelled) setIsLoadingRecentPractice(false);
      });
    return () => {
      cancelled = true;
    };
  }, [view, decks.length]);

  useEffect(() => {
    if (launchRequest === null || lastLaunchRef.current === launchRequest.requestId) return;
    lastLaunchRef.current = launchRequest.requestId;
    const available = new Set(decks.map((deck) => deck.id));
    const requested = launchRequest.deckIds.filter((id) => available.has(id));
    setSelectedDeckIds(requested);
    setSession(null);
    setCurrentIndex(0);
    setView("setup");
    onLaunchConsumed();
    if (launchRequest.autoStart && requested.length > 0) {
      void startSession(requested, settings.practiceShuffle);
    }
  }, [decks, launchRequest, onLaunchConsumed, settings.practiceShuffle, startSession]);

  useEffect(() => {
    if (view !== "session" || session === null || current === null) return;
    const restoreRevealed = restoreRevealedCardRef.current === current.card.id;
    restoreRevealedCardRef.current = null;
    setAnswerRevealed(restoreRevealed);
    if (!restoreRevealed) setIntervals([]);
    setLastResult(null);
    shownAtRef.current = Date.now();
    setAnnouncement(
      restoreRevealed
        ? `Card ${currentIndex + 1} of ${session.cards.length}. Review undone; answer shown.`
        : `Card ${currentIndex + 1} of ${session.cards.length}. Question shown.`,
    );
    void markPracticeCardViewed(session.id, current.card.id).catch((error: unknown) => {
      setErrorMessage(getErrorMessage(error));
    });
  }, [current?.card.id, currentIndex, session, view]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent): void {
      if (view !== "session" || current === null) return;
      const target = event.target;
      const isTyping =
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target instanceof HTMLElement && target.isContentEditable);
      if (isTyping) return;
      if (!answerRevealed && (event.key === " " || event.key === "Enter")) {
        event.preventDefault();
        void revealAnswer();
        return;
      }
      if (answerRevealed && /^[0-3]$/u.test(event.key)) {
        event.preventDefault();
        void rate(Number(event.key) as ReviewScore);
        return;
      }
      if (event.key.toLowerCase() === "u" || ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z")) {
        event.preventDefault();
        void undo();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  async function revealAnswer(): Promise<void> {
    if (session === null || current === null || answerRevealed || isRevealing) return;
    setIsRevealing(true);
    setErrorMessage(null);
    try {
      await markPracticeCardRevealed(session.id, current.card.id);
      setAnswerRevealed(true);
      setAnnouncement("Answer revealed. Rate the difficulty from 0 through 3.");
      try {
        setIntervals(await previewReviewIntervals(current.card.id, Date.now()));
      } catch (error: unknown) {
        setIntervals([]);
        showToast("info", `Interval previews are unavailable: ${getErrorMessage(error)}`);
      }
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsRevealing(false);
    }
  }

  async function rate(score: ReviewScore): Promise<void> {
    if (session === null || current === null || !answerRevealed || isSubmitting) return;
    setIsSubmitting(true);
    setErrorMessage(null);
    try {
      const result = await submitStudyReview({
        sessionId: session.id,
        cardId: current.card.id,
        submissionId: createSubmissionId(),
        score,
        responseTimeMs: Math.max(0, Date.now() - shownAtRef.current),
        nextDayStartMs: nextLocalDayStart(),
      });
      setRatedCount(result.ratedCount);
      setCorrectCount(result.correctCount);
      setLastResult(result);
      if (result.struggling) {
        showToast(
          "info",
          result.autoSuspended
            ? "This is now a Struggling card and was automatically suspended."
            : "This card is now marked Struggling so you can find and revise it.",
        );
      }
      const nextIndex = currentIndex + 1;
      if (result.completed || nextIndex >= session.cards.length) {
        setView("complete");
        setAnnouncement(`Session complete. ${result.correctCount} of ${result.ratedCount} answers were not Wrong.`);
      } else {
        setCurrentIndex(nextIndex);
      }
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsSubmitting(false);
    }
  }

  async function undo(): Promise<void> {
    if (session === null || isUndoing || ratedCount === 0) return;
    setIsUndoing(true);
    setErrorMessage(null);
    try {
      const result = await undoLastStudyReview(session.id);
      const card = session.cards[result.position];
      if (!card) {
        throw new Error("The undone card is no longer available in this practice session.");
      }
      setRatedCount(result.ratedCount);
      setCorrectCount(result.correctCount);
      restoreRevealedCardRef.current = card.card.id;
      setIntervals([]);
      setCurrentIndex(result.position);
      setView("session");
      setLastResult(null);
      try {
        setIntervals(await previewReviewIntervals(card.card.id, Date.now()));
      } catch {
        setIntervals([]);
      }
      showToast("success", "Last review undone; the previous schedule was restored exactly.");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsUndoing(false);
    }
  }

  async function endSession(): Promise<void> {
    if (session !== null && view === "session") {
      try {
        await completePracticeSession(session.id);
      } catch (error: unknown) {
        showToast("error", getErrorMessage(error));
        return;
      }
    }
    setSession(null);
    setView("setup");
    setCurrentIndex(0);
    setAnswerRevealed(false);
    setIntervals([]);
    setLastResult(null);
  }

  function toggleDeck(deckId: string): void {
    setSelectedDeckIds((currentIds) =>
      currentIds.includes(deckId)
        ? currentIds.filter((id) => id !== deckId)
        : [...currentIds, deckId],
    );
  }

  if (view === "setup") {
    return (
      <div className="screen practice-screen">
        <header className="screen-header compact-screen-header">
          <div>
            <h1>Practice</h1>
            <p>Choose decks, then rate each revealed answer once from 0 to 3.</p>
          </div>
        </header>
        {errorMessage ? <div className="inline-alert error-alert" role="alert">{errorMessage}</div> : null}
        <div className="practice-setup-layout">
          <section className="practice-setup-card" aria-labelledby="select-practice-decks-heading">
            <div className="section-heading">
              <div>
                <h2 id="select-practice-decks-heading">Select decks</h2>
                <p>Daily limits are shared across all selected decks.</p>
              </div>
              <div className="header-actions">
                <button type="button" className="small-ghost-button" onClick={() => setSelectedDeckIds(decks.map((deck) => deck.id))}>Select all</button>
                <button type="button" className="small-ghost-button" onClick={() => setSelectedDeckIds([])}>Clear</button>
              </div>
            </div>
            {decks.length === 0 ? (
              <div className="compact-empty-state"><p>Create a deck and add cards before practicing.</p></div>
            ) : (
              <div className="practice-deck-grid">
                {decks.map((deck) => {
                  const selected = selectedDeckIds.includes(deck.id);
                  return (
                    <label key={deck.id} className={selected ? "practice-deck-option selected" : "practice-deck-option"}>
                      <input type="checkbox" checked={selected} onChange={() => toggleDeck(deck.id)} />
                      <span>
                        <strong>{deck.name}</strong>
                        <small>{deck.dueCount} due · {deck.cardCount} review cards</small>
                      </span>
                    </label>
                  );
                })}
              </div>
            )}
            <div className="practice-setup-controls">
              <label className="compact-checkbox practice-shuffle-checkbox">
                <input type="checkbox" checked={shuffle} onChange={(event) => setShuffle(event.currentTarget.checked)} />
                <span>Shuffle decks</span>
              </label>
              <span className="practice-selection-count">{selectedDeckIds.length} selected</span>
            </div>
            <div className="practice-limit-summary" aria-label="Daily practice limits">
              <span><strong>{settings.newCardsPerDay}</strong> new/day</span>
              <span><strong>{settings.reviewsPerDay}</strong> reviews/day</span>
              <span><strong>{settings.strugglingCardThreshold}</strong> misses before Struggling</span>
            </div>
            <div className="practice-setup-actions">
              <button type="button" className="primary-button" disabled={isStarting || selectedDeckIds.length === 0} onClick={() => void startSession(selectedDeckIds, shuffle)}>
                {isStarting ? "Starting…" : "Start practice"}
              </button>
            </div>
          </section>

          <section className="recent-practice-panel" aria-labelledby="recent-practice-heading">
            <div className="section-heading recent-practice-heading">
              <div>
                <h2 id="recent-practice-heading">Recently practiced</h2>
                <p>Your latest answer-confidence mix for each deck.</p>
              </div>
            </div>
            {isLoadingRecentPractice ? (
              <div className="inline-progress compact-progress">Loading recent practice…</div>
            ) : recentPractice.length === 0 ? (
              <div className="compact-empty-state">
                <p>Completed reviews will appear here for quick access.</p>
              </div>
            ) : (
              <div className="recent-practice-list">
                {recentPractice.map((recent) => (
                  <article key={`${recent.deckId}:${recent.sessionId}`} className="recent-practice-item">
                    <div className="recent-practice-title-row">
                      <div>
                        <strong>{recent.deckName}</strong>
                        <span>{formatRecentPracticeTime(recent.practicedAt)} · {recent.reviewCount} reviewed</span>
                      </div>
                      <button
                        type="button"
                        className="small-ghost-button"
                        onClick={() => setSelectedDeckIds([recent.deckId])}
                        aria-label={`Select ${recent.deckName} for practice`}
                      >
                        Select
                      </button>
                    </div>
                    <ConfidenceStrip recent={recent} />
                  </article>
                ))}
              </div>
            )}
          </section>
        </div>
      </div>
    );
  }

  if (view === "complete" && session) {
    const percent = ratedCount === 0 ? 0 : Math.round((correctCount / ratedCount) * 100);
    return (
      <div className="screen practice-screen">
        <div className="sr-only" aria-live="polite">{announcement}</div>
        {errorMessage ? <div className="inline-alert error-alert" role="alert">{errorMessage}</div> : null}
        <section className="practice-complete-card">
          <span className="completion-mark" aria-hidden="true">✓</span>
          <h1>Practice complete</h1>
          <p>You reviewed {ratedCount} cards. {correctCount} were rated Hard, OK, or Easy.</p>
          <div className="completion-metrics">
            <div><strong>{ratedCount}</strong><span>Reviewed</span></div>
            <div><strong>{correctCount}</strong><span>Not wrong</span></div>
            <div><strong>{percent}%</strong><span>Success rate</span></div>
          </div>
          {lastResult ? <p className="muted-copy">Last card returns {formatInterval(lastResult.intervalMs)} from now.</p> : null}
          <div className="header-actions">
            <button type="button" className="secondary-button" disabled={isUndoing || ratedCount === 0} onClick={() => void undo()}>{isUndoing ? "Undoing…" : "Undo last review"}</button>
            <button type="button" className="primary-button" onClick={() => void endSession()}>Choose decks</button>
          </div>
        </section>
      </div>
    );
  }

  if (session === null || current === null) {
    return <div className="screen"><div className="inline-progress">Loading practice card…</div></div>;
  }

  const progress = session.cards.length === 0 ? 0 : Math.round((ratedCount / session.cards.length) * 100);
  return (
    <div className="screen practice-screen active-practice-screen">
      <div className="sr-only" aria-live="polite">{announcement}</div>
      <header className="practice-session-header">
        <div>
          <button type="button" className="back-button" disabled={isSubmitting || isUndoing} onClick={() => void endSession()}>← End session</button>
          <h1>{current.deckName}</h1>
          <p>{current.isNew ? "New card" : "Scheduled review"} · Card {currentIndex + 1} of {session.cards.length}</p>
        </div>
        <div className="practice-header-actions">
          <button type="button" className="secondary-button" disabled={isUndoing || ratedCount === 0} onClick={() => void undo()}>{isUndoing ? "Undoing…" : "Undo last"} <kbd>U</kbd></button>
          <span>{ratedCount} rated · {correctCount} not wrong</span>
        </div>
      </header>

      <div className="practice-progress-track" aria-label={`${ratedCount} of ${session.cards.length} cards rated`}>
        <span style={{ width: `${progress}%` }} />
      </div>

      {errorMessage ? <div className="inline-alert error-alert" role="alert">{errorMessage}</div> : null}

      <section className="practice-card-shell">
        <div className="practice-card-label-row">
          <span className="eyebrow">{answerRevealed ? "ANSWER" : "QUESTION"}</span>
          <span>{current.card.displayName}</span>
        </div>
        <div className="practice-card-surface">
          <RichTextView
            document={answerRevealed ? current.card.backDoc : current.card.frontDoc}
            assets={current.card.assets}
            onLinkError={setErrorMessage}
          />
        </div>
      </section>

      {!answerRevealed ? (
        <div className="practice-reveal-area">
          <button type="button" className="primary-button reveal-answer-button" disabled={isRevealing} onClick={() => void revealAnswer()}>
            {isRevealing ? "Revealing…" : "Show answer"} <kbd>Space</kbd>
          </button>
        </div>
      ) : (
        <section className="direct-rating-panel" aria-labelledby="difficulty-heading">
          <div className="section-heading centered-heading">
            <div>
              <h2 id="difficulty-heading">How difficult was it to answer?</h2>
              <p>Choose one rating. The interval preview shows the next scheduled review.</p>
            </div>
          </div>
          <div className="direct-rating-grid">
            {SCORE_OPTIONS.map((option) => {
              const preview = intervalByScore.get(option.score);
              return (
                <button
                  type="button"
                  key={option.score}
                  className={`direct-rating-button rating-${option.className}`}
                  disabled={isSubmitting}
                  onClick={() => void rate(option.score)}
                  aria-label={`${option.score}, ${option.label}. ${option.description}. ${preview ? `Next review ${formatInterval(preview.intervalMs)}.` : ""}`}
                >
                  <span className="rating-key">{option.keyLabel}</span>
                  <strong>{option.label}</strong>
                  <small>{option.description}</small>
                  <em>{preview ? formatInterval(preview.intervalMs) : "Calculating…"}</em>
                </button>
              );
            })}
          </div>
        </section>
      )}

      <footer className="practice-session-footer">
        <span>{session.newCount} new · {session.reviewCount} review in this session</span>
        <span>{session.newRemainingToday} new and {session.reviewsRemainingToday} reviews remain after this queue</span>
      </footer>
    </div>
  );
}

function ConfidenceStrip({ recent }: { recent: RecentDeckPractice }) {
  const ratings = recent.ratingDistribution;
  const values = [
    { label: "Wrong", count: ratings.wrong, className: "wrong" },
    { label: "Hard", count: ratings.hard, className: "hard" },
    { label: "OK", count: ratings.ok, className: "ok" },
    { label: "Easy", count: ratings.easy, className: "easy" },
  ];
  const total = Math.max(1, recent.reviewCount);
  return (
    <div className="recent-confidence-summary" aria-label={`Answer confidence for ${recent.deckName}`}>
      <div className="recent-confidence-strip" aria-hidden="true">
        {values.map((rating) => (
          <span
            key={rating.label}
            className={`confidence-segment ${rating.className}`}
            style={{ width: `${(rating.count / total) * 100}%` }}
          />
        ))}
      </div>
      <div className="recent-confidence-counts">
        {values.map((rating) => (
          <span key={rating.label} className={`confidence-count ${rating.className}`}>
            <i aria-hidden="true" />{rating.label} {rating.count}
          </span>
        ))}
      </div>
    </div>
  );
}

function formatRecentPracticeTime(value: number): string {
  const elapsed = Date.now() - value;
  if (elapsed >= 0 && elapsed < 60_000) return "just now";
  if (elapsed >= 0 && elapsed < 3_600_000) {
    const minutes = Math.max(1, Math.round(elapsed / 60_000));
    return `${minutes} min ago`;
  }
  if (elapsed >= 0 && elapsed < 86_400_000) {
    const hours = Math.max(1, Math.round(elapsed / 3_600_000));
    return `${hours} hr ago`;
  }
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(value));
}

function localDayBoundaries(): { dayStartMs: number; nextDayStartMs: number } {
  const now = new Date();
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  const next = new Date(start);
  next.setDate(next.getDate() + 1);
  return { dayStartMs: start.getTime(), nextDayStartMs: next.getTime() };
}

function nextLocalDayStart(): number {
  return localDayBoundaries().nextDayStartMs;
}

function formatInterval(milliseconds: number): string {
  if (milliseconds <= 0) return "now";
  const minutes = Math.max(1, Math.round(milliseconds / 60_000));
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.round(milliseconds / 3_600_000);
  if (hours < 36) return `${hours} hr`;
  const days = Math.round(milliseconds / 86_400_000);
  if (days < 60) return `${days} day${days === 1 ? "" : "s"}`;
  const months = Math.round(days / 30.44);
  if (months < 24) return `${months} mo`;
  const years = Math.round((days / 365.25) * 10) / 10;
  return `${years} yr`;
}
