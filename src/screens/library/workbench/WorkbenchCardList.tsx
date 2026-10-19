import { useRef, type ReactNode } from "react";

import type { CardBrowserRow, CardSort } from "../../../ipc/cards";
import { Icon } from "../../../ui/icons/Icon";
import { IconButton } from "../../../ui/icons/IconButton";
import { cn } from "../../../ui/internal/cn";
import { SelectionToolbar } from "../../../ui/patterns/SelectionToolbar";
import { VirtualList } from "../../../ui/patterns/VirtualList";
import type { ResponsiveAction } from "../../../ui/patterns/ResponsiveActionGroup";
import { Button } from "../../../ui/primitives/Button";
import { EmptyState } from "../../../ui/primitives/EmptyState";
import { Checkbox } from "../../../ui/primitives/FormControls";
import { InlineNotice, InlineProgress } from "../../../ui/primitives/InlineNotice";
import { ItemTrigger } from "../../../ui/primitives/Item";
import { NativeSelect } from "../../../ui/primitives/NativeSelect";
import { TextField } from "../../../ui/primitives/TextField";
import type { WorkbenchCardsState } from "./useWorkbenchCards";
import {
  CARD_FILTERS,
  dueLabel,
  intervalLabel,
  SORT_OPTIONS,
  sourceColumnLabel,
  sourceUnitLabel,
  type CardFilter,
  type DueTone,
} from "./workbenchModel";

const ROW_HEIGHT = 44;
const COMPACT_ROW_HEIGHT = 68;

/** Checkbox, source unit, front, type, due and interval: the mockup's columns. */
const COLUMNS = "tw:grid tw:grid-cols-[2rem_3.25rem_minmax(0,1fr)_6.5rem_5.5rem_3.5rem] tw:items-center tw:gap-x-2";

interface WorkbenchCardListProps {
  cards: WorkbenchCardsState;
  /** Where the list is: "Cardiac physiology / Deck". */
  eyebrow?: string;
  title: string;
  /** The deck or subject actions beside the title. */
  actions?: ReactNode;
  filter: CardFilter;
  onFilterChange: (filter: CardFilter) => void;
  query: string;
  onQueryChange: (query: string) => void;
  sort: CardSort;
  onSortChange: (sort: CardSort) => void;
  searchRef?: React.Ref<HTMLInputElement>;
  openCardId: string | null;
  onOpenCard: (row: CardBrowserRow) => void;
  selectionActions: { primary: Omit<ResponsiveAction, "placement" | "presentation">; secondary: ResponsiveAction[] };
  emptyTitle: string;
  emptyDescription: string;
  emptyAction?: ReactNode;
  compact: boolean;
  /** Phones: the way back to the tree. */
  onBack?: () => void;
}

/**
 * The Workbench's card list: a deck, subject, view or tag as dense rows that
 * load as they scroll, with search, a Show filter, sort and a selection that
 * bulk actions work on. Choosing a row opens it in the editor beside the list.
 */
export function WorkbenchCardList({
  cards,
  eyebrow,
  title,
  actions,
  filter,
  onFilterChange,
  query,
  onQueryChange,
  sort,
  onSortChange,
  searchRef,
  openCardId,
  onOpenCard,
  selectionActions,
  emptyTitle,
  emptyDescription,
  emptyAction,
  compact,
  onBack,
}: WorkbenchCardListProps) {
  const shiftRef = useRef(false);
  const sourceColumn = sourceColumnLabel(cards.rows) ?? "Source";
  const sortOptions = SORT_OPTIONS.filter((option) => option.value !== "relevance" || query.trim() !== "");
  const allVisibleSelected = cards.rows.length > 0 && cards.rows.every((row) => cards.selected.has(row.cardId));

  function onCheck(row: CardBrowserRow): void {
    if (shiftRef.current) cards.selectRange(row);
    else cards.toggle(row);
    shiftRef.current = false;
  }

  return (
    <section aria-label="Cards" className="tw:flex tw:h-full tw:min-h-0 tw:min-w-0 tw:flex-col tw:gap-3">
      <header className="tw:grid tw:min-w-0 tw:gap-3">
        <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-x-3 tw:gap-y-2">
          <div className="tw:flex tw:min-w-0 tw:flex-[1_1_14rem] tw:items-start tw:gap-1">
            {onBack ? <IconButton icon="back" label="Library" onClick={onBack} className="tw:shrink-0" /> : null}
            <div className="tw:grid tw:min-w-0 tw:gap-0.5">
              {eyebrow ? <p className="tw:m-0 tw:text-caption tw:text-muted-foreground tw:break-words">{eyebrow}</p> : null}
              <h1 className="tw:m-0 tw:text-title-small tw:break-words" data-testid="workbench-title">{title}</h1>
            </div>
          </div>
          {actions ? <div className="tw:flex tw:shrink-0 tw:flex-wrap tw:items-center tw:gap-2">{actions}</div> : null}
        </div>
        <div data-tour-anchor="search.filters" className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2">
          <label className="tw:relative tw:flex tw:min-w-0 tw:flex-[1_1_14rem] tw:items-center">
            <span className="sr-only">Search cards</span>
            <Icon name="search" size={16} className="tw:pointer-events-none tw:absolute tw:left-3 tw:text-muted-foreground" />
            <TextField
              ref={searchRef}
              type="search"
              value={query}
              placeholder="Search questions, answers and tags"
              className="tw:w-full tw:pl-9"
              onChange={(event) => onQueryChange(event.currentTarget.value)}
            />
          </label>
          <label className="tw:flex tw:items-center tw:gap-2 tw:text-small tw:text-muted-foreground">
            <span className={compact ? "sr-only" : undefined}>Show</span>
            <NativeSelect
              aria-label="Show cards"
              data-testid="workbench-filter"
              value={filter}
              onChange={(event) => onFilterChange(event.currentTarget.value as CardFilter)}
            >
              {CARD_FILTERS.map((option) => {
                const count = cards.filterCounts[option.value];
                return (
                  <option key={option.value} value={option.value}>
                    {count === undefined ? option.label : `${option.label} · ${count.toLocaleString()}`}
                  </option>
                );
              })}
            </NativeSelect>
          </label>
          <label className="tw:flex tw:items-center tw:gap-2 tw:text-small tw:text-muted-foreground">
            <span className={compact ? "sr-only" : undefined}>Sort</span>
            <NativeSelect data-testid="workbench-sort" value={sort} onChange={(event) => onSortChange(event.currentTarget.value as CardSort)}>
              {sortOptions.map((option) => <option key={option.value} value={option.value}>{option.label}</option>)}
            </NativeSelect>
          </label>
        </div>
      </header>

      {cards.selected.size > 0 ? (
        <SelectionToolbar
          selectedCount={cards.selected.size}
          primaryAction={selectionActions.primary}
          secondaryActions={selectionActions.secondary}
          clearLabel="Deselect all"
          onClearSelection={cards.clearSelection}
          className="tw:mb-0"
        />
      ) : null}
      {cards.error ? <InlineNotice kind="error">{cards.error}</InlineNotice> : null}

      <div className="tw:flex tw:min-h-0 tw:min-w-0 tw:flex-1 tw:flex-col tw:overflow-hidden tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card">
        <div className="tw:flex tw:min-h-10 tw:flex-wrap tw:items-center tw:justify-between tw:gap-2 tw:border-b tw:border-solid tw:border-border tw:px-3 tw:py-1.5 tw:text-small tw:text-muted-foreground">
          <span aria-live="polite" className="tw:tabular-nums">
            {cards.loading ? "Loading cards…" : `${cards.rows.length.toLocaleString()} of ${cards.totalCount.toLocaleString()} shown`}
          </span>
          {cards.totalCount > 0 ? (
            <span className="tw:flex tw:flex-wrap tw:gap-1">
              <Button
                variant="ghost"
                size="small"
                onClick={() => (allVisibleSelected ? cards.clearSelection() : cards.rows.forEach((row) => {
                  if (!cards.selected.has(row.cardId)) cards.toggle(row);
                }))}
              >
                {allVisibleSelected ? "Deselect shown" : "Select shown"}
              </Button>
              {cards.totalCount > cards.rows.length || !allVisibleSelected ? (
                <Button variant="ghost" size="small" loading={cards.selectingAll} onClick={() => void cards.selectAllMatching()}>
                  Select all {cards.totalCount.toLocaleString()}
                </Button>
              ) : null}
            </span>
          ) : null}
        </div>
        {!compact && cards.rows.length > 0 ? (
          <div aria-hidden="true" className={cn(COLUMNS, "tw:border-b tw:border-solid tw:border-border tw:px-3 tw:py-1.5 tw:text-caption tw:font-semibold tw:text-muted-foreground")}>
            <span />
            <span>{sourceColumn}</span>
            <span>Front</span>
            <span>Type</span>
            <span>Due</span>
            <span className="tw:text-right">Interval</span>
          </div>
        ) : null}
        {cards.loading && cards.rows.length === 0 ? <div className="tw:p-4"><InlineProgress>Loading cards…</InlineProgress></div> : null}
        {!cards.loading && cards.totalCount === 0 && !cards.error ? (
          <div className="tw:p-4">
            <EmptyState compact title={emptyTitle} description={emptyDescription} actions={emptyAction} />
          </div>
        ) : null}
        {cards.rows.length > 0 ? (
          <VirtualList
            className="tw:flex-1"
            label="Cards"
            items={cards.rows}
            itemKey={(row) => row.cardId}
            estimateItemSize={compact ? COMPACT_ROW_HEIGHT : ROW_HEIGHT}
            onEndReached={cards.loadMore}
            footer={cards.loadingMore ? <div className="tw:p-3"><InlineProgress>Loading more cards…</InlineProgress></div> : null}
            renderItem={(row) => (
              <CardRow
                row={row}
                todayStartAt={cards.todayStartAt}
                selected={cards.selected.has(row.cardId)}
                open={row.cardId === openCardId}
                compact={compact}
                onShift={(shift) => { shiftRef.current = shift; }}
                onCheck={() => onCheck(row)}
                onOpen={() => onOpenCard(row)}
              />
            )}
          />
        ) : null}
      </div>
    </section>
  );
}

const DUE_TONES: Readonly<Record<DueTone, string>> = {
  due: "tw:font-semibold tw:text-warning",
  new: "tw:text-primary",
  suspended: "tw:text-muted-foreground tw:italic",
  later: "tw:text-muted-foreground",
};

interface CardRowProps {
  row: CardBrowserRow;
  todayStartAt: number;
  selected: boolean;
  open: boolean;
  compact: boolean;
  onShift: (shift: boolean) => void;
  onCheck: () => void;
  onOpen: () => void;
}

function CardRow({ row, todayStartAt, selected, open, compact, onShift, onCheck, onOpen }: CardRowProps) {
  const front = summarize(row.frontText);
  const due = dueLabel(row, todayStartAt);
  const source = row.sourceUnit === null ? null : `${sourceUnitLabel(row.sourceUnitKind)} ${row.sourceUnit}`;
  const checkbox = (
    <span
      className="tw:flex tw:items-center tw:justify-center"
      onPointerDown={(event) => onShift(event.shiftKey)}
      onKeyDown={(event) => onShift(event.shiftKey)}
    >
      <Checkbox checked={selected} aria-label={`Select ${front}`} onCheckedChange={onCheck} />
    </span>
  );
  const rowClass = cn(
    "tw:border-b tw:border-solid tw:border-border tw:px-3",
    selected && "tw:bg-accent/50",
    open && "tw:bg-accent tw:shadow-[inset_3px_0_0_var(--ostudy-color-accent-base)]",
  );
  if (compact) {
    return (
      <div className={cn(rowClass, "tw:flex tw:min-w-0 tw:items-start tw:gap-2 tw:py-2")} data-card-id={row.cardId}>
        <span className="tw:pt-0.5">{checkbox}</span>
        <ItemTrigger
          aria-current={open ? "true" : undefined}
          onClick={onOpen}
          className="tw:grid tw:min-h-11 tw:min-w-0 tw:flex-1 tw:cursor-pointer tw:gap-1 tw:border-0 tw:bg-transparent tw:p-0 tw:text-left tw:text-foreground"
        >
          <span className="tw:line-clamp-2 tw:text-body tw:break-words">{front}</span>
          <span className="tw:flex tw:flex-wrap tw:gap-x-2 tw:text-caption tw:text-muted-foreground">
            <span>{row.cardLabel}</span>
            {source ? <span>{source}</span> : null}
            <span className={DUE_TONES[due.tone]}>{due.tone === "due" ? "due today" : due.text}</span>
          </span>
        </ItemTrigger>
      </div>
    );
  }
  return (
    <div className={cn(rowClass, COLUMNS, "tw:min-h-11 tw:py-1")} data-card-id={row.cardId}>
      {checkbox}
      <span className="tw:text-small tw:text-muted-foreground tw:tabular-nums" title={source ?? undefined}>
        {row.sourceUnit ?? ""}
      </span>
      <ItemTrigger
        aria-current={open ? "true" : undefined}
        aria-label={`${front}. ${row.cardLabel}, ${due.text}`}
        onClick={onOpen}
        className="tw:min-h-9 tw:min-w-0 tw:text-small tw:text-foreground tw:hover:underline"
        title={front}
      >
        <span className="tw:truncate">{front}</span>
      </ItemTrigger>
      <span className="tw:truncate tw:text-small tw:text-muted-foreground">{row.cardLabel}</span>
      <span className={cn("tw:text-small", DUE_TONES[due.tone])}>{due.text}</span>
      <span className="tw:text-right tw:text-small tw:text-muted-foreground tw:tabular-nums">{intervalLabel(row)}</span>
    </div>
  );
}

export function summarize(value: string): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (normalized === "") return "Image card";
  return normalized.length > 300 ? `${normalized.slice(0, 297)}…` : normalized;
}
