import type {
  AnalyticsDeckOption,
  AnalyticsRange,
  AnalyticsTagOption,
} from "../../ipc/analytics";
import type { DeckSummary } from "../../ipc/decks";
import { SegmentedControl } from "../../ui/primitives/SegmentedControl";

interface AnalyticsFiltersProps {
  range: AnalyticsRange;
  customStartLocalDate: string;
  customEndLocalDate: string;
  selectedDeckIds: Set<string>;
  selectedTagIds: Set<string>;
  highlightedSessionId?: string;
  deckOptions: AnalyticsDeckOption[];
  tagOptions: AnalyticsTagOption[];
  decks: DeckSummary[];
  onRangeChange: (range: AnalyticsRange) => void;
  onCustomStartLocalDateChange: (value: string) => void;
  onCustomEndLocalDateChange: (value: string) => void;
  onDeckIdsChange: (next: Set<string>) => void;
  onTagIdsChange: (next: Set<string>) => void;
  onClearHighlightedSession: () => void;
  onClearScope: () => void;
}

const RANGE_OPTIONS: ReadonlyArray<{ value: AnalyticsRange; label: string }> = [
  { value: "7d", label: "7 days" },
  { value: "30d", label: "30 days" },
  { value: "90d", label: "90 days" },
  { value: "1y", label: "1 year" },
  { value: "all", label: "All time" },
  { value: "custom", label: "Custom" },
];

/** Pure analytics range/scope controls. Native reads and stale-response policy live in the controller. */
export function AnalyticsFilters({
  range,
  customStartLocalDate,
  customEndLocalDate,
  selectedDeckIds,
  selectedTagIds,
  highlightedSessionId,
  deckOptions,
  tagOptions,
  decks,
  onRangeChange,
  onCustomStartLocalDateChange,
  onCustomEndLocalDateChange,
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
      <SegmentedControl
        label="Time range"
        value={range}
        options={RANGE_OPTIONS}
        onChange={(value) => {
          onRangeChange(value);
          onClearHighlightedSession();
        }}
      />

      {range === "custom" ? (
        <div className="analytics-custom-range" aria-label="Custom analytics date range">
          <label>
            <span>Start date</span>
            <input
              type="date"
              value={customStartLocalDate}
              max={customEndLocalDate}
              onChange={(event) => onCustomStartLocalDateChange(event.currentTarget.value)}
            />
          </label>
          <label>
            <span>End date</span>
            <input
              type="date"
              value={customEndLocalDate}
              min={customStartLocalDate}
              max={localToday()}
              onChange={(event) => onCustomEndLocalDateChange(event.currentTarget.value)}
            />
          </label>
        </div>
      ) : null}

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

function localToday(): string {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return year + "-" + month + "-" + day;
}
