import type {
  AnalyticsDeckOption,
  AnalyticsRange,
  AnalyticsTagOption,
} from "../../ipc/analytics";
import type { DeckSummary } from "../../ipc/decks";

interface AnalyticsFiltersProps {
  range: AnalyticsRange;
  selectedDeckIds: Set<string>;
  selectedTagIds: Set<string>;
  highlightedSessionId?: string;
  deckOptions: AnalyticsDeckOption[];
  tagOptions: AnalyticsTagOption[];
  decks: DeckSummary[];
  onRangeChange: (range: AnalyticsRange) => void;
  onDeckIdsChange: (next: Set<string>) => void;
  onTagIdsChange: (next: Set<string>) => void;
  onClearHighlightedSession: () => void;
  onClearScope: () => void;
}

/** Pure analytics range/scope controls. Native reads and stale-response policy live in the controller. */
export function AnalyticsFilters({
  range,
  selectedDeckIds,
  selectedTagIds,
  highlightedSessionId,
  deckOptions,
  tagOptions,
  decks,
  onRangeChange,
  onDeckIdsChange,
  onTagIdsChange,
  onClearHighlightedSession,
  onClearScope,
}: AnalyticsFiltersProps) {
  const deckIds = [...selectedDeckIds].sort();
  const tagIds = [...selectedTagIds].sort();
  const selectedDeckNames = deckIds
    .map(
      (id) =>
        deckOptions.find((deck) => deck.id === id)?.name ??
        decks.find((deck) => deck.id === id)?.name,
    )
    .filter((name): name is string => Boolean(name));
  const selectedTagNames = tagIds
    .map((id) => tagOptions.find((tag) => tag.id === id)?.name)
    .filter((name): name is string => Boolean(name));

  return (
    <section className="analytics-filter-panel" aria-label="Analytics filters">
      <div className="analytics-range-control" role="group" aria-label="Time range">
        {([
          ["7d", "7 days"],
          ["30d", "30 days"],
          ["90d", "90 days"],
          ["1y", "1 year"],
          ["all", "All time"],
        ] as const).map(([value, label]) => (
          <button
            key={value}
            type="button"
            className={range === value ? "analytics-filter-button active" : "analytics-filter-button"}
            aria-pressed={range === value}
            onClick={() => {
              onRangeChange(value);
              onClearHighlightedSession();
            }}
          >
            {label}
          </button>
        ))}
      </div>

      <details className="analytics-scope-details">
        <summary>
          Scope: {selectedDeckNames.length === 0 ? "All decks" : `${selectedDeckNames.length} deck${selectedDeckNames.length === 1 ? "" : "s"}`}
          {selectedTagNames.length > 0
            ? ` · ${selectedTagNames.length} tag${selectedTagNames.length === 1 ? "" : "s"}`
            : ""}
        </summary>
        <div className="analytics-scope-grid">
          <FilterChecklist
            label="Decks"
            emptyLabel="No decks available"
            options={deckOptions.map((deck) => ({
              id: deck.id,
              name: deck.name,
              detail: deck.isArchived
                ? `Archived · ${deck.hasHistory ? "history retained" : "no reviews"}`
                : deck.hasCurrentCards
                  ? "Active inventory"
                  : deck.hasHistory
                    ? "Historical evidence"
                    : "Empty",
            }))}
            selected={selectedDeckIds}
            onChange={(next) => {
              onDeckIdsChange(next);
              onClearHighlightedSession();
            }}
          />
          <FilterChecklist
            label="Tags"
            emptyLabel="No current or historical tags"
            options={tagOptions.map((tag) => ({
              id: tag.id,
              name: tag.name,
              detail: tag.isCurrent
                ? tag.hasHistory
                  ? "Current · reviewed"
                  : "Current"
                : "Historical evidence",
            }))}
            selected={selectedTagIds}
            onChange={(next) => {
              onTagIdsChange(next);
              onClearHighlightedSession();
            }}
          />
        </div>
      </details>

      {selectedDeckNames.length > 0 || selectedTagNames.length > 0 || highlightedSessionId ? (
        <div className="analytics-filter-chips" aria-label="Active analytics filters">
          {selectedDeckNames.map((name) => (
            <span key={`deck-${name}`}>Deck: {name}</span>
          ))}
          {selectedTagNames.map((name) => (
            <span key={`tag-${name}`}>Tag: {name}</span>
          ))}
          {highlightedSessionId ? <span>Completed session highlighted</span> : null}
          <button type="button" className="text-button" onClick={onClearScope}>
            Clear scope
          </button>
        </div>
      ) : null}
    </section>
  );
}

interface FilterOption {
  id: string;
  name: string;
  detail: string;
}

interface FilterChecklistProps {
  label: string;
  emptyLabel: string;
  options: FilterOption[];
  selected: Set<string>;
  onChange: (next: Set<string>) => void;
}

function FilterChecklist({
  label,
  emptyLabel,
  options,
  selected,
  onChange,
}: FilterChecklistProps) {
  return (
    <fieldset className="analytics-filter-checklist">
      <legend>{label}</legend>
      {options.length === 0 ? (
        <p>{emptyLabel}</p>
      ) : (
        <div>
          {options.map((option) => (
            <label key={option.id}>
              <input
                type="checkbox"
                checked={selected.has(option.id)}
                onChange={(event) => {
                  const next = new Set(selected);
                  if (event.currentTarget.checked) next.add(option.id);
                  else next.delete(option.id);
                  onChange(next);
                }}
              />
              <span>
                <strong>{option.name}</strong>
                <small>{option.detail}</small>
              </span>
            </label>
          ))}
        </div>
      )}
    </fieldset>
  );
}
