import type { DeckSummary } from "../../ipc/decks";
import type { PracticeWorkloadOverview } from "../../ipc/practice";
import { formatConfidence, type RecentDeckSummary } from "./confidence";

interface PracticeSetupProps {
  decks: DeckSummary[];
  selectedDeckIds: Set<string>;
  shuffle: boolean;
  isStarting: boolean;
  isLoadingWorkload: boolean;
  workload: PracticeWorkloadOverview | null;
  recentDecks: RecentDeckSummary[];
  onSelectedDeckIdsChange: (deckIds: Set<string>) => void;
  onShuffleChange: (shuffle: boolean) => void;
  onStart: (deckIds: string[], shuffle: boolean, mode: "scheduled" | "extra") => void;
}

const reviewedDateFormatter = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

export function PracticeSetup({
  decks,
  selectedDeckIds,
  shuffle,
  isStarting,
  isLoadingWorkload,
  workload,
  recentDecks,
  onSelectedDeckIdsChange,
  onShuffleChange,
  onStart,
}: PracticeSetupProps) {
  const decksWithCards = decks.filter(
    (deck) => deck.archivedAt === null && deck.generatedCardCount > 0,
  );
  const scheduledCount = (workload?.scheduledReviews ?? 0) + (workload?.scheduledNew ?? 0);

  if (decksWithCards.length === 0) {
    return (
      <section className="compact-empty-state large-empty-state">
        <h2>No active flashcards yet</h2>
        <p>Add a card to an active deck or unarchive a deck before starting a session.</p>
      </section>
    );
  }

  return (
    <>
      <section className="practice-setup-panel">
        <div className="panel-heading-row">
          <div>
            <h2>Select decks to study</h2>
            <span>{selectedDeckIds.size} selected</span>
          </div>
          <div className="inline-actions">
            <button
              type="button"
              className="text-button"
              onClick={() =>
                onSelectedDeckIdsChange(new Set(decksWithCards.map((deck) => deck.id)))
              }
            >
              Select all
            </button>
            {selectedDeckIds.size > 0 ? (
              <button
                type="button"
                className="text-button"
                onClick={() => onSelectedDeckIdsChange(new Set())}
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
              <label
                key={deck.id}
                className={["practice-deck-choice", checked ? "selected" : ""]
                  .filter(Boolean)
                  .join(" ")}
              >
                <input
                  type="checkbox"
                  checked={checked}
                  onChange={(event) => {
                    const next = new Set(selectedDeckIds);
                    if (event.currentTarget.checked) next.add(deck.id);
                    else next.delete(deck.id);
                    onSelectedDeckIdsChange(next);
                  }}
                />
                <span>
                  <strong>{deck.name}</strong>
                  <small>
                    {deck.studyableDueCount > 0
                      ? `${deck.studyableDueCount} ready now · ${deck.generatedCardCount} generated total`
                      : `Nothing ready now · ${deck.generatedCardCount} generated total`}
                  </small>
                </span>
              </label>
            );
          })}
        </div>

        {workload && selectedDeckIds.size > 0 ? (
          <div className="session-plan-copy">
            <strong>{scheduledCount} cards in the scheduled session</strong>
            <span>
              {workload.scheduledReviews} reviews · {workload.scheduledNew} new
            </span>
            {workload.omittedDueReviews > 0 ? (
              <small>
                {workload.omittedDueReviews} ready reviews remain outside today’s limit.
              </small>
            ) : null}
            {workload.omittedDueNew > 0 ? (
              <small>
                {workload.omittedDueNew} ready new cards are paused by the workload policy.
              </small>
            ) : null}
          </div>
        ) : null}

        <div className="practice-setup-actions">
          <label className="compact-shuffle-control">
            <input
              type="checkbox"
              checked={shuffle}
              onChange={(event) => onShuffleChange(event.currentTarget.checked)}
            />
            <span>Shuffle within priority groups</span>
          </label>
          <div className="practice-due-inline">
            <strong>{scheduledCount}</strong>
            <span>scheduled</span>
          </div>
          {scheduledCount > 0 ? (
            <button
              type="button"
              className="primary-button practice-start-button"
              disabled={selectedDeckIds.size === 0 || isStarting || isLoadingWorkload}
              onClick={() => onStart([...selectedDeckIds], shuffle, "scheduled")}
            >
              {isStarting ? "Starting…" : "Start scheduled practice"}
            </button>
          ) : workload && workload.extraAvailable > 0 && selectedDeckIds.size > 0 ? (
            <button
              type="button"
              className="secondary-button practice-start-button"
              disabled={isStarting || isLoadingWorkload}
              onClick={() => onStart([...selectedDeckIds], shuffle, "extra")}
            >
              {isStarting ? "Starting…" : "Study this selection anyway"}
            </button>
          ) : (
            <button type="button" className="primary-button practice-start-button" disabled>
              Start scheduled practice
            </button>
          )}
        </div>
      </section>

      <section className="practice-workload-panel" aria-label="Practice workload" aria-live="polite">
        <div>
          <span>Today</span>
          <strong>{workload?.reviewsRemaining ?? 0}</strong>
          <small>reviews remaining</small>
        </div>
        <div>
          <span>New</span>
          <strong>{workload?.newRemaining ?? 0}</strong>
          <small>cards remaining</small>
        </div>
        <div>
          <span>Ready now</span>
          <strong>
            {(workload?.studyableDueReviews ?? 0) + (workload?.studyableDueNew ?? 0)}
          </strong>
          <small>studyable cards</small>
        </div>
        <div>
          <span>Priority</span>
          <strong>{workload?.needsMorePractice ?? 0}</strong>
          <small>need more practice</small>
        </div>
        {isLoadingWorkload ? <span className="workload-loading">Updating…</span> : null}
      </section>

      <section className="recent-practice-section" aria-labelledby="recent-practice-heading">
        <div className="recent-practice-heading-row">
          <h2 id="recent-practice-heading">Recently practiced</h2>
        </div>
        {recentDecks.length === 0 ? (
          <p className="recent-practice-empty">
            Complete a practice session and its confidence result will appear here.
          </p>
        ) : (
          <div className="recent-practice-list">
            {recentDecks.map((deck) => (
              <article key={deck.deckName} className="recent-practice-row">
                <div>
                  <strong>{deck.deckName}</strong>
                  <span>
                    {deck.reviewCount} recent reviews ·{" "}
                    {reviewedDateFormatter.format(new Date(deck.lastReviewedAt))}
                  </span>
                </div>
                <div className="recent-confidence-copy">
                  <strong>{formatConfidence(deck.averageConfidence)}</strong>
                  <span>average confidence</span>
                </div>
              </article>
            ))}
          </div>
        )}
      </section>
    </>
  );
}
