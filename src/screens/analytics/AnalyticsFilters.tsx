import { useState } from "react";

import type { AnalyticsDeckOption, AnalyticsRange, AnalyticsTagOption } from "../../ipc/analytics";
import type { DeckSummary } from "../../ipc/decks";
import { Icon } from "../../ui/icons/Icon";
import { SegmentedControl, type SegmentedOptions } from "../../ui/primitives/SegmentedControl";
import { Button } from "../../ui/primitives/Button";
import { TextField } from "../../ui/primitives/TextField";
import { Checkbox, Disclosure } from "../../ui/primitives/FormControls";
import { Chip } from "../../ui/primitives/Chip";
import { Surface } from "../../ui/primitives/Surface";
import { styles } from "./presentation";

interface AnalyticsFiltersProps {
  range: AnalyticsRange; customStartLocalDate: string; customEndLocalDate: string;
  selectedDeckIds: Set<string>; selectedTagIds: Set<string>; highlightedSessionId?: string;
  deckOptions: AnalyticsDeckOption[]; tagOptions: AnalyticsTagOption[]; decks: DeckSummary[];
  onRangeChange: (range: AnalyticsRange) => void;
  onCustomStartLocalDateChange: (value: string) => void; onCustomEndLocalDateChange: (value: string) => void;
  onDeckIdsChange: (next: Set<string>) => void; onTagIdsChange: (next: Set<string>) => void;
  onClearHighlightedSession: () => void; onClearScope: () => void;
}
type PresetRange = Exclude<AnalyticsRange, "custom">;
const RANGE_OPTIONS = [
  { value: "7d", label: "7 days" }, { value: "30d", label: "30 days" }, { value: "90d", label: "90 days" },
  { value: "1y", label: "1 year" }, { value: "all", label: "All time" },
] as const satisfies SegmentedOptions<PresetRange>;
/** Pure scope controls: native reads and stale-response policy stay in the controller. */
export function AnalyticsFilters({ range, customStartLocalDate, customEndLocalDate, selectedDeckIds, selectedTagIds,
  highlightedSessionId, deckOptions, tagOptions, decks, onRangeChange, onCustomStartLocalDateChange,
  onCustomEndLocalDateChange, onDeckIdsChange, onTagIdsChange, onClearHighlightedSession, onClearScope }: AnalyticsFiltersProps) {
  const deckIds = [...selectedDeckIds].sort();
  const tagIds = [...selectedTagIds].sort();
  const selectedDeckNames = deckIds.map((id) => deckOptions.find((deck) => deck.id === id)?.name ?? decks.find((deck) => deck.id === id)?.name)
    .filter((name): name is string => Boolean(name));
  const selectedTagNames = tagIds.map((id) => tagOptions.find((tag) => tag.id === id)?.name).filter((name): name is string => Boolean(name));
  // Custom dates sit beside the presets as a toggle; turning it off returns to the last preset.
  const [lastPreset, setLastPreset] = useState<PresetRange>(range === "custom" ? "7d" : range);
  if (range !== "custom" && range !== lastPreset) setLastPreset(range);
  function changeRange(next: AnalyticsRange) { onRangeChange(next); onClearHighlightedSession(); }
  return <Surface as="section" className={styles.analyticsFilterPanel} aria-label="Analytics filters">
    <div className={styles.analyticsRangeRow}>
      <SegmentedControl label="Time range" className="tw:compact:w-full" value={range === "custom" ? null : range}
        options={RANGE_OPTIONS} onChange={changeRange} />
      <Button aria-pressed={range === "custom"} className={styles.analyticsCustomToggle}
        onClick={() => changeRange(range === "custom" ? lastPreset : "custom")}>
        <Icon name="calendar" size={16} />Custom dates
      </Button>
    </div>
    {range === "custom" ? <div className={styles.analyticsCustomRange} aria-label="Custom analytics date range">
      <label><span>Start date</span><TextField type="date" value={customStartLocalDate} max={customEndLocalDate} onChange={(event) => onCustomStartLocalDateChange(event.currentTarget.value)} /></label>
      <label><span>End date</span><TextField type="date" value={customEndLocalDate} min={customStartLocalDate} max={localToday()} onChange={(event) => onCustomEndLocalDateChange(event.currentTarget.value)} /></label>
    </div> : null}
    <Disclosure className={styles.analyticsScopeDetails} title={<>
      Scope: {selectedDeckNames.length === 0 ? "All decks" : `${selectedDeckNames.length} deck${selectedDeckNames.length === 1 ? "" : "s"}`}
      {selectedTagNames.length > 0 ? ` · ${selectedTagNames.length} tag${selectedTagNames.length === 1 ? "" : "s"}` : ""}
    </>}>
      <div className={styles.analyticsScopeGrid}>
        <FilterChecklist label="Decks" emptyLabel="No decks available" options={deckOptions.map((deck) => ({
          id: deck.id, name: deck.name, detail: deck.isArchived ? `Archived · ${deck.hasHistory ? "history retained" : "no reviews"}`
            : deck.hasCurrentCards ? "Active inventory" : deck.hasHistory ? "Historical evidence" : "Empty",
        }))} selected={selectedDeckIds} onChange={(next) => { onDeckIdsChange(next); onClearHighlightedSession(); }} />
        <FilterChecklist label="Tags" emptyLabel="No current or historical tags" options={tagOptions.map((tag) => ({
          id: tag.id, name: tag.name, detail: tag.isCurrent ? tag.hasHistory ? "Current · reviewed" : "Current" : "Historical evidence",
        }))} selected={selectedTagIds} onChange={(next) => { onTagIdsChange(next); onClearHighlightedSession(); }} />
      </div>
    </Disclosure>
    {selectedDeckNames.length > 0 || selectedTagNames.length > 0 || highlightedSessionId ? <div className={styles.analyticsFilterChips} aria-label="Active analytics filters">
      {selectedDeckNames.map((name) => <Chip key={`deck-${name}`}>Deck: {name}</Chip>)}
      {selectedTagNames.map((name) => <Chip key={`tag-${name}`}>Tag: {name}</Chip>)}
      {highlightedSessionId ? <Chip tone="accent">Completed session highlighted</Chip> : null}
      <Button variant="text" onClick={onClearScope}>Clear scope</Button>
    </div> : null}
  </Surface>;
}
interface FilterOption { id: string; name: string; detail: string; }
interface FilterChecklistProps { label: string; emptyLabel: string; options: FilterOption[]; selected: Set<string>; onChange: (next: Set<string>) => void; }
function FilterChecklist({ label, emptyLabel, options, selected, onChange }: FilterChecklistProps) {
  return <fieldset className={styles.analyticsFilterChecklist}>
    <legend>{label}</legend>
    {options.length === 0 ? <p>{emptyLabel}</p> : <div>{options.map((option) => <label key={option.id}>
      <Checkbox checked={selected.has(option.id)} onCheckedChange={(checked) => {
        const next = new Set(selected); if (checked) next.add(option.id); else next.delete(option.id); onChange(next);
      }} /><span><strong>{option.name}</strong><small>{option.detail}</small></span>
    </label>)}</div>}
  </fieldset>;
}
function localToday(): string {
  const now = new Date();
  return now.getFullYear() + "-" + String(now.getMonth() + 1).padStart(2, "0") + "-" + String(now.getDate()).padStart(2, "0");
}
