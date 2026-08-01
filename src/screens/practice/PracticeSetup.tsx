import type { Deck } from "../../ipc";
import { formatConfidence, type RecentDeckSummary } from "./confidence";

interface PracticeSetupProps {
  decks: Deck[];
  selectedDeckIds: Set<string>;
  shuffle: boolean;
  isStarting: boolean;
  recentDecks: RecentDeckSummary[];
  onSelectedDeckIdsChange: (deckIds: Set<string>) => void;
  onShuffleChange: (shuffle: boolean) => void;
  onStart: (deckIds: string[], shuffle: boolean) => void;
}

const reviewedDateFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
});

export function PracticeSetup({
  decks,
  selectedDeckIds,
  shuffle,
  isStarting,
  recentDecks,
  onSelectedDeckIdsChange,
  onShuffleChange,
  onStart,
}: PracticeSetupProps) {
  const decksWithCards = decks.filter((deck) => deck.cardCount > 0);
  const dueCount = selectedDueCount(decks, selectedDeckIds);

  if (decksWithCards.length === 0) {
    return (
      <section className="compact-empty-state large-empty-state">
        <h2>No flashcards yet</h2>
        <p>Add a card to a deck before starting a session.</p>
      </section>
    );
  }

  return (
    <>
      <section className="practice-setup-panel">
        <div className="panel-heading-row">
          <div>
            <h2>Choose decks</h2>
            <span>{selectedDeckIds.size} selected</span>
          </div>
          <div className="inline-actions">
            <button
              type="button"
              className="text-button"
              onClick={() =>
                onSelectedDeckIdsChange(
                  new Set(
                    decksWithCards
                      .filter((deck) => deck.dueCount > 0)
                      .map((deck) => deck.id),
                  ),
                )
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
                className={[
                  "practice-deck-choice",
                  checked ? "selected" : "",
                  deck.dueCount === 0 ? "disabled" : "",
                ]
                  .filter(Boolean)
                  .join(" ")}
              >
                <input
                  type="checkbox"
                  checked={checked}
                  disabled={deck.dueCount === 0}
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
                    {deck.dueCount} due · {deck.cardCount} total
                  </small>
                </span>
              </label>
            );
          })}
        </div>

        <div className="practice-setup-actions">
          <label className="compact-shuffle-control">
            <input
              type="checkbox"
              checked={shuffle}
              onChange={(event) => onShuffleChange(event.currentTarget.checked)}
            />
            <span>Shuffle decks</span>
          </label>
          <div className="practice-due-inline" aria-live="polite">
            <strong>{dueCount}</strong>
            <span>cards due</span>
          </div>
          <button
            type="button"
            className="primary-button practice-start-button"
            disabled={selectedDeckIds.size === 0 || dueCount === 0 || isStarting}
            onClick={() => onStart([...selectedDeckIds], shuffle)}
          >
            {isStarting ? "Starting…" : "Start practice"}
          </button>
        </div>
      </section>

      <section className="recent-practice-section" aria-labelledby="recent-practice-heading">
        <div className="recent-practice-heading-row">
          <div>
            <h2 id="recent-practice-heading">Recently practiced</h2>
            <p>Latest results by deck, scored from 0 (Wrong) to 3 (Easy).</p>
          </div>
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

function selectedDueCount(decks: Deck[], selectedDeckIds: Set<string>): number {
  return decks.reduce(
    (total, deck) => total + (selectedDeckIds.has(deck.id) ? deck.dueCount : 0),
    0,
  );
}
