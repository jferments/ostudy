import { useEffect, useMemo, useRef, useState } from "react";

import type { AnalyticsLaunchRequest } from "../../App";
import type { AnalyticsRange, TagAnalytics } from "../../ipc/analytics";
import type { DeckSummary } from "../../ipc/decks";
import type { ProgressQuery } from "../../ipc/progress";
import { IconButton } from "../../ui/icons/IconButton";
import { Icon } from "../../ui/icons/Icon";
import { Sheet } from "../../ui/overlays/Dialog";
import { FilterSummary, PageHeader, PageScaffold, type ActiveFilter } from "../../ui/patterns/WorkspacePatterns";
import { Button } from "../../ui/primitives/Button";
import { Checkbox } from "../../ui/primitives/FormControls";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { NativeSelect } from "../../ui/primitives/NativeSelect";
import { SegmentedControl, type SegmentedOptions } from "../../ui/primitives/SegmentedControl";
import { TextField } from "../../ui/primitives/TextField";
import { ActivityPanel } from "./ActivityPanel";
import { CoachBanner, slippingTopic } from "./CoachBanner";
import { AnswersPanel, QuestionTypesPanel, SessionsPanel } from "./DetailPanels";
import { ForecastPanel } from "./ForecastPanel";
import { ReviewsChart } from "./ReviewsChart";
import { StrengthPanel } from "./StrengthPanel";
import { SubjectsPanel } from "./SubjectsPanel";
import { SummaryTiles } from "./SummaryTiles";
import { TopicsPanel } from "./TopicsPanel";
import { formatDay, formatSpan, shiftLocalDate } from "./progressFormat";
import { subjectBadgeColors } from "./subjectBadges";
import { useProgressData } from "./useProgressData";

type PresetRange = Exclude<AnalyticsRange, "custom">;
const RANGE_OPTIONS = [
  { value: "7d", label: "7 days" },
  { value: "30d", label: "30 days" },
  { value: "90d", label: "90 days" },
  { value: "1y", label: "Year" },
  { value: "all", label: "All time" },
] as const satisfies SegmentedOptions<PresetRange>;
const RANGE_LABELS: Record<AnalyticsRange, string> = { "7d": "7 days", "30d": "30 days", "90d": "90 days", "1y": "Year", all: "All time", custom: "Custom dates" };

interface ProgressScreenProps {
  decks: DeckSummary[];
  launchRequest: AnalyticsLaunchRequest | null;
  onLaunchConsumed: () => void;
  onPracticeScope: (deckIds: string[], tagIds: string[]) => void | Promise<void>;
  onSearchTag: (tagId: string) => void | Promise<void>;
}

/** Today's date on this device, the first guess for custom dates until native reports the study date. */
function deviceToday(): string {
  const now = new Date();
  return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}-${String(now.getDate()).padStart(2, "0")}`;
}

/**
 * The Progress page: how much the learner studied, how well it stuck, and what to do next.
 * Each section reads its own native command, so a filter change redraws sections as they arrive.
 */
export function ProgressScreen({ decks, launchRequest, onLaunchConsumed, onPracticeScope, onSearchTag }: ProgressScreenProps) {
  const [range, setRange] = useState<AnalyticsRange>("30d");
  const [lastPreset, setLastPreset] = useState<PresetRange>("30d");
  const [customStart, setCustomStart] = useState(() => shiftLocalDate(deviceToday(), -29));
  const [customEnd, setCustomEnd] = useState(deviceToday);
  const [subjectId, setSubjectId] = useState<string | null>(null);
  const [deckIds, setDeckIds] = useState<string[]>([]);
  const [tagIds, setTagIds] = useState<string[]>([]);
  const [highlightedSessionId, setHighlightedSessionId] = useState<string | undefined>();
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [practicingTagId, setPracticingTagId] = useState<string | null>(null);
  const [startingReview, setStartingReview] = useState(false);
  const handledLaunch = useRef<number | null>(null);

  const query = useMemo<ProgressQuery>(() => ({
    range,
    subjectIds: subjectId ? [subjectId] : [],
    deckIds,
    tagIds,
    ...(range === "custom" ? { customStartLocalDate: customStart, customEndLocalDate: customEnd } : {}),
  }), [customEnd, customStart, deckIds, range, subjectId, tagIds]);
  const data = useProgressData(query);

  useEffect(() => {
    if (!launchRequest || handledLaunch.current === launchRequest.requestId) return;
    handledLaunch.current = launchRequest.requestId;
    setRange(launchRequest.range);
    if (launchRequest.range !== "custom") setLastPreset(launchRequest.range);
    setDeckIds([...launchRequest.deckIds].sort());
    setTagIds([...launchRequest.tagIds].sort());
    setHighlightedSessionId(launchRequest.highlightedSessionId);
    onLaunchConsumed();
  }, [launchRequest, onLaunchConsumed]);

  const library = data.library.data;
  const subjects = library?.subjects ?? [];
  const subjectColors = useMemo(() => subjectBadgeColors(library?.subjects ?? []), [library]);
  const subjectName = subjects.find((subject) => subject.id === subjectId)?.name ?? null;
  const overview = data.overview.data;
  const today = overview?.scope.todayLocalDate ?? data.activity.data?.todayLocalDate ?? deviceToday();
  const goal = overview?.desiredRetention ?? null;
  const topic = slippingTopic(data.topics.data);
  const dueToday = data.forecast.data?.days[0]?.dueCount ?? 0;
  const deckNames = new Map((data.options.data?.decks ?? decks).map((deck) => [deck.id, deck.name]));
  const tagNames = new Map((data.options.data?.tags ?? []).map((tag) => [tag.id, tag.name]));

  // The decks a Practice button studies: the chosen decks, narrowed to the subject when both are set.
  const scopeDeckIds = useMemo(() => {
    if (!subjectId) return deckIds;
    const inSubject = (library?.decks ?? []).filter((placement) => placement.subjectId === subjectId).map((placement) => placement.deckId);
    return deckIds.length === 0 ? inSubject : deckIds.filter((id) => inSubject.includes(id));
  }, [deckIds, library, subjectId]);
  const dueDeckIds = (scopeDeckIds.length > 0 || subjectId ? decks.filter((deck) => scopeDeckIds.includes(deck.id)) : decks)
    .filter((deck) => deck.archivedAt === null && deck.studyableDueCount > 0)
    .map((deck) => deck.id);
  const busy = practicingTagId !== null || startingReview;

  function practiceTopic(next: TagAnalytics): void {
    if (busy) return;
    setPracticingTagId(next.tagId);
    void Promise.resolve(onPracticeScope(scopeDeckIds, [...new Set([...tagIds, next.tagId])])).finally(() => setPracticingTagId(null));
  }
  function startReview(): void {
    if (busy || dueDeckIds.length === 0) return;
    setStartingReview(true);
    void Promise.resolve(onPracticeScope(dueDeckIds, tagIds)).finally(() => setStartingReview(false));
  }
  function chooseRange(next: AnalyticsRange): void {
    if (next !== "custom") setLastPreset(next);
    setRange(next);
    setHighlightedSessionId(undefined);
  }
  function clearFilters(): void {
    setDeckIds([]);
    setTagIds([]);
    setHighlightedSessionId(undefined);
    if (range === "custom") setRange(lastPreset);
  }

  const activeFilters: ActiveFilter[] = [
    ...(range === "custom" ? [{ id: "range", label: `${formatDay(customStart)} to ${formatDay(customEnd)}` }] : []),
    ...deckIds.map((id) => ({ id: `deck:${id}`, label: `Deck: ${deckNames.get(id) ?? "Deck"}` })),
    ...tagIds.map((id) => ({ id: `tag:${id}`, label: `Tag: ${tagNames.get(id) ?? "Tag"}` })),
    ...(highlightedSessionId ? [{ id: "session", label: "Finished session highlighted" }] : []),
  ];
  function removeFilter(id: string): void {
    if (id === "range") setRange(lastPreset);
    else if (id === "session") setHighlightedSessionId(undefined);
    else if (id.startsWith("deck:")) setDeckIds((current) => current.filter((value) => `deck:${value}` !== id));
    else setTagIds((current) => current.filter((value) => `tag:${value}` !== id));
  }

  const errors = [...new Set(Object.values(data).map((section) => section.error).filter((error): error is string => error !== null))];
  const span = overview ? formatSpan(overview.scope.startLocalDate, overview.scope.endLocalDate, today) : null;
  const periodLabel = overview ? (overview.scope.range === "all" ? "All time" : range === "custom" ? span ?? "" : `Last ${RANGE_LABELS[range].toLowerCase()}`) : "This range";
  const filterCount = deckIds.length + tagIds.length + (range === "custom" ? 1 : 0);
  const showSubjects = !subjectId && (data.subjects.data?.length ?? 0) > 0 && subjects.length > 0;

  return (
    <PageScaffold width="wide">
      <PageHeader
        title="Progress"
        description={`${subjectName ?? "All subjects"}${span ? ` · ${span}` : ""}`}
        actions={
          <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2">
            <NativeSelect aria-label="Subject" value={subjectId ?? ""} onChange={(event) => setSubjectId(event.currentTarget.value || null)} className="tw:w-auto tw:max-w-56">
              <option value="">All subjects</option>
              {subjects.map((subject) => <option key={subject.id} value={subject.id}>{subject.name}</option>)}
            </NativeSelect>
            <span className="tw:compact:hidden">
              <SegmentedControl label="Time range" value={range === "custom" ? null : range} options={RANGE_OPTIONS} onChange={chooseRange} />
            </span>
            <NativeSelect aria-label="Time range" value={range} onChange={(event) => chooseRange(event.currentTarget.value as AnalyticsRange)} className="tw:hidden tw:w-auto tw:compact:block">
              {RANGE_OPTIONS.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
              {range === "custom" ? <option value="custom">Custom dates</option> : null}
            </NativeSelect>
            <span className="tw:compact:hidden">
              <Button variant="ghost" aria-haspopup="dialog" onClick={() => setFiltersOpen(true)}>
                <Icon name="filter" size={16} />Decks and tags{filterCount > 0 ? ` (${filterCount})` : ""}
              </Button>
            </span>
            <span className="tw:hidden tw:compact:inline-flex">
              <IconButton icon="filter" label={filterCount > 0 ? `Decks, tags and dates (${filterCount} chosen)` : "Decks, tags and dates"} aria-haspopup="dialog" onClick={() => setFiltersOpen(true)} />
            </span>
          </div>
        }
      />
      {activeFilters.length > 0 ? <div className="tw:mb-4"><FilterSummary filters={activeFilters} onRemove={removeFilter} onClear={clearFilters} /></div> : null}
      {errors.length > 0 ? <div className="tw:mb-4"><InlineNotice kind="error">{errors.join(" ")}</InlineNotice></div> : null}

      <CoachBanner overview={overview} topic={topic} activity={data.activity.data} dueToday={dueToday} busy={busy}
        onPracticeTopic={practiceTopic} onStartReview={dueDeckIds.length > 0 ? startReview : null} />
      <SummaryTiles overview={overview} strength={data.strength.data} activity={data.activity.data} loading={data.overview.loading} />

      <div className="tw:grid tw:min-w-0 tw:gap-4">
        <div className="tw:grid tw:min-w-0 tw:gap-4 tw:wide:grid-cols-[minmax(0,1.75fr)_minmax(0,1fr)]">
          <ReviewsChart overview={overview} loading={data.overview.loading} />
          <ForecastPanel forecast={data.forecast.data} loading={data.forecast.loading} />
        </div>
        <div className="tw:grid tw:min-w-0 tw:gap-4 tw:wide:grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)]">
          <TopicsPanel topics={data.topics.data} goal={goal} loading={data.topics.loading} practicingTagId={practicingTagId} onPractice={practiceTopic} />
          <StrengthPanel strength={data.strength.data} goal={goal} loading={data.strength.loading} />
        </div>
        {showSubjects ? (
          <SubjectsPanel subjects={data.subjects.data} colors={subjectColors} periodLabel={periodLabel} loading={data.subjects.loading}
            onOpenSubject={setSubjectId} />
        ) : null}
        <ActivityPanel activity={data.activity.data} loading={data.activity.loading} />
        <div className="tw:grid tw:min-w-0 tw:gap-4 tw:medium:grid-cols-2 tw:wide:grid-cols-3">
          <QuestionTypesPanel rows={data.questionTypes.data} loading={data.questionTypes.loading} />
          <SessionsPanel sessions={data.sessions.data} highlightedSessionId={highlightedSessionId} loading={data.sessions.loading} />
          <AnswersPanel query={query} firstPage={data.answers.data} loading={data.answers.loading} onSearchTag={(tagId) => void onSearchTag(tagId)} />
        </div>
      </div>

      {filtersOpen ? (
        <ScopeSheet
          range={range} customStart={customStart} customEnd={customEnd} today={today}
          deckOptions={(data.options.data?.decks ?? decks.map((deck) => ({ id: deck.id, name: deck.name, isArchived: deck.archivedAt !== null, hasCurrentCards: true, hasHistory: false })))}
          tagOptions={data.options.data?.tags ?? []}
          deckIds={deckIds} tagIds={tagIds}
          onCustomRange={(start, end) => { setCustomStart(start); setCustomEnd(end); chooseRange("custom"); }}
          onPresetRange={() => chooseRange(lastPreset)}
          onDeckIds={setDeckIds} onTagIds={setTagIds} onClear={clearFilters}
          onClose={() => setFiltersOpen(false)}
        />
      ) : null}
    </PageScaffold>
  );
}

interface ScopeOption { id: string; name: string; isArchived?: boolean }

interface ScopeSheetProps {
  range: AnalyticsRange;
  customStart: string;
  customEnd: string;
  today: string;
  deckOptions: ScopeOption[];
  tagOptions: ScopeOption[];
  deckIds: string[];
  tagIds: string[];
  onCustomRange: (start: string, end: string) => void;
  onPresetRange: () => void;
  onDeckIds: (ids: string[]) => void;
  onTagIds: (ids: string[]) => void;
  onClear: () => void;
  onClose: () => void;
}

/** Decks, tags and custom dates: the narrower filters, kept off the page until asked for. */
function ScopeSheet({ range, customStart, customEnd, today, deckOptions, tagOptions, deckIds, tagIds, onCustomRange, onPresetRange, onDeckIds, onTagIds, onClear, onClose }: ScopeSheetProps) {
  const custom = range === "custom";
  return (
    <Sheet title="Decks, tags and dates" description="Each choice narrows every section except Activity, which always shows all your study." onClose={onClose}
      actions={<>
        <Button variant="ghost" onClick={onClear}>Clear all</Button>
        <Button variant="primary" onClick={onClose}>Done</Button>
      </>}>
      <fieldset className="tw:m-0 tw:grid tw:gap-3 tw:border-0 tw:p-0">
        <legend className="tw:mb-2 tw:p-0 tw:font-semibold">Dates</legend>
        <label className="tw:flex tw:items-center tw:gap-2.5 tw:text-small">
          <Checkbox checked={custom} onCheckedChange={(checked) => (checked ? onCustomRange(customStart, customEnd) : onPresetRange())} />
          Choose my own dates
        </label>
        {custom ? (
          <div className="tw:grid tw:gap-3 tw:sm:grid-cols-2">
            <label className="tw:grid tw:gap-1.5 tw:text-small tw:font-medium">Start date
              <TextField type="date" value={customStart} max={customEnd} onChange={(event) => onCustomRange(event.currentTarget.value, customEnd)} />
            </label>
            <label className="tw:grid tw:gap-1.5 tw:text-small tw:font-medium">End date
              <TextField type="date" value={customEnd} min={customStart} max={today} onChange={(event) => onCustomRange(customStart, event.currentTarget.value)} />
            </label>
          </div>
        ) : null}
      </fieldset>
      <Checklist legend="Decks" empty="No decks yet" options={deckOptions} selected={deckIds} onChange={onDeckIds} />
      <Checklist legend="Tags" empty="No tags yet" options={tagOptions} selected={tagIds} onChange={onTagIds} />
    </Sheet>
  );
}

function Checklist({ legend, empty, options, selected, onChange }: { legend: string; empty: string; options: ScopeOption[]; selected: string[]; onChange: (ids: string[]) => void }) {
  return (
    <fieldset className="tw:m-0 tw:grid tw:min-w-0 tw:gap-0.5 tw:border-0 tw:p-0">
      <legend className="tw:mb-2 tw:p-0 tw:font-semibold">{legend}</legend>
      {options.length === 0 ? <p className="tw:m-0 tw:text-small tw:text-muted-foreground">{empty}</p> : options.map((option) => (
        <label key={option.id} className="tw:flex tw:min-h-9 tw:items-center tw:gap-2.5 tw:rounded-md tw:px-2 tw:text-small tw:hover:bg-muted/60 tw:coarse:min-h-touch">
          <Checkbox checked={selected.includes(option.id)} onCheckedChange={(checked) => onChange(
            checked ? [...selected, option.id].sort() : selected.filter((id) => id !== option.id),
          )} />
          <span className="tw:min-w-0 tw:break-words">{option.name}{option.isArchived ? <span className="tw:text-muted-foreground"> (archived)</span> : null}</span>
        </label>
      ))}
    </fieldset>
  );
}
