import type {
  AvailabilityFilter,
  CardSort,
  CardStatusFilter,
  ReviewedFilter,
  TagSummary,
} from "../../ipc/cards";
import type { DeckSummary } from "../../ipc/decks";

interface CardFiltersProps {
  activeDecks: DeckSummary[];
  deckIds: Set<string>;
  query: string;
  availabilityFilter: AvailabilityFilter;
  statusFilter: CardStatusFilter;
  reviewedFilter: ReviewedFilter;
  sort: CardSort;
  tags: TagSummary[];
  tagIds: Set<string>;
  onDeckIdsChange: (next: Set<string>) => void;
  onQueryChange: (value: string) => void;
  onAvailabilityFilterChange: (value: AvailabilityFilter) => void;
  onStatusFilterChange: (value: CardStatusFilter) => void;
  onReviewedFilterChange: (value: ReviewedFilter) => void;
  onSortChange: (value: CardSort) => void;
  onTagIdsChange: (next: Set<string>) => void;
}

/** Pure Search/Filter controls; data loading and mutation orchestration stay in the controller. */
export function CardFilters({
  activeDecks,
  deckIds,
  query,
  availabilityFilter,
  statusFilter,
  reviewedFilter,
  sort,
  tags,
  tagIds,
  onDeckIdsChange,
  onQueryChange,
  onAvailabilityFilterChange,
  onStatusFilterChange,
  onReviewedFilterChange,
  onSortChange,
  onTagIdsChange,
}: CardFiltersProps) {
  return (
    <section className="card-library-toolbar" aria-label="Card filters">
      <details className="filter-menu deck-filter-menu">
        <summary>{deckFilterSummary(deckIds, activeDecks)}</summary>
        <div className="filter-checklist">
          <label>
            <input
              type="checkbox"
              checked={deckIds.size === 0}
              onChange={() => onDeckIdsChange(new Set())}
            />
            <span>All active decks</span>
            <small>{activeDecks.reduce((sum, deck) => sum + deck.generatedCardCount, 0)}</small>
          </label>
          {activeDecks.map((deck) => (
            <label key={deck.id}>
              <input
                type="checkbox"
                checked={deckIds.has(deck.id)}
                onChange={(event) => {
                  const next = new Set(deckIds);
                  if (event.currentTarget.checked) next.add(deck.id);
                  else next.delete(deck.id);
                  onDeckIdsChange(next);
                }}
              />
              <span>{deck.name}</span>
              <small>{deck.generatedCardCount}</small>
            </label>
          ))}
        </div>
      </details>

      <label className="card-library-search">
        <span>Search cards</span>
        <input
          type="search"
          value={query}
          placeholder="Question, answer, or tag"
          onChange={(event) => onQueryChange(event.currentTarget.value)}
        />
      </label>

      <label>
        <span>Availability</span>
        <select
          value={availabilityFilter}
          disabled={statusFilter === "suspended" || statusFilter === "buried"}
          onChange={(event) =>
            onAvailabilityFilterChange(event.currentTarget.value as AvailabilityFilter)
          }
        >
          <option value="all">Any schedule</option>
          <option value="ready_now">Ready now</option>
          <option value="scheduled_later">Scheduled later</option>
        </select>
      </label>

      <label>
        <span>Status</span>
        <select
          value={statusFilter}
          onChange={(event) => onStatusFilterChange(event.currentTarget.value as CardStatusFilter)}
        >
          <option value="active">Studyable</option>
          <option value="all">Any current status</option>
          <option value="suspended">Suspended</option>
          <option value="buried">Buried</option>
          <option value="needs_more_practice">Needs more practice</option>
        </select>
      </label>

      <label>
        <span>Last studied</span>
        <select
          value={reviewedFilter}
          onChange={(event) => onReviewedFilterChange(event.currentTarget.value as ReviewedFilter)}
        >
          <option value="all">Any local date</option>
          <option value="never">Never studied</option>
          <option value="today">Today</option>
          <option value="past_7_local_dates">Past 7 local dates</option>
          <option value="past_30_local_dates">Past 30 local dates</option>
          <option value="before_past_30_local_dates">Before the past 30 local dates</option>
        </select>
      </label>

      <label>
        <span>Sort</span>
        <select
          value={sort}
          onChange={(event) => onSortChange(event.currentTarget.value as CardSort)}
        >
          <option value="relevance" disabled={query.trim() === ""}>
            Relevance
          </option>
          <option value="updated_desc">Recently updated</option>
          <option value="last_reviewed_desc">Recently studied</option>
          <option value="question_asc">Question A–Z</option>
          <option value="due_asc">Next scheduled</option>
        </select>
      </label>

      <details className="filter-menu tag-filter-menu">
        <summary>Tags{tagIds.size > 0 ? ` (${tagIds.size})` : ""}</summary>
        <div className="filter-checklist">
          {tags.length === 0 ? (
            <p>No tags in the selected active decks.</p>
          ) : (
            tags.map((tag) => (
              <label key={tag.id}>
                <input
                  type="checkbox"
                  checked={tagIds.has(tag.id)}
                  onChange={(event) => {
                    const next = new Set(tagIds);
                    if (event.currentTarget.checked) next.add(tag.id);
                    else next.delete(tag.id);
                    onTagIdsChange(next);
                  }}
                />
                <span>{tag.name}</span>
                <small>
                  {tag.generatedCardCount} cards · {tag.studyableDueCount} ready
                </small>
              </label>
            ))
          )}
          {tagIds.size > 0 ? (
            <button type="button" className="text-button" onClick={() => onTagIdsChange(new Set())}>
              Clear tag filters
            </button>
          ) : null}
        </div>
      </details>
    </section>
  );
}

function deckFilterSummary(deckIds: Set<string>, decks: DeckSummary[]): string {
  if (deckIds.size === 0) return "All active decks";
  const selectedNames = decks.filter((deck) => deckIds.has(deck.id)).map((deck) => deck.name);
  return selectedNames.length === 1
    ? (selectedNames[0] ?? "1 deck")
    : `${selectedNames.length} decks`;
}
