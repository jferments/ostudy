import { useEffect, useMemo, useState, type ReactNode } from "react";

import type {
  AvailabilityFilter,
  CardSort,
  CardStatusFilter,
  ReviewedFilter,
  TagSummary,
} from "../../ipc/cards";
import type { DeckSummary } from "../../ipc/decks";
import {
  getDeckLibraryOrganization,
  type DeckFolder,
} from "../../ipc/deckFolders";
import { HelpTrigger } from "../../ui/help/HelpProvider";
import { Icon } from "../../ui/icons/Icon";
import {
  AdaptiveSheet,
  FilterSummary,
  type ActiveFilter,
} from "../../ui/patterns/WorkspacePatterns";
import { Button } from "../../ui/primitives/Button";
import { Surface } from "../../ui/primitives/Surface";
import styles from "./CardFilters.module.css";

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

interface FolderScope {
  folder: DeckFolder;
  deckIds: string[];
}

/**
 * Search keeps the high-frequency controls visible and moves the complete
 * filter model into one adaptive overlay. All changes still apply immediately;
 * the sheet is disclosure, not a second form state.
 */
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
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [folderScopes, setFolderScopes] = useState<FolderScope[]>([]);

  useEffect(() => {
    let active = true;
    void getDeckLibraryOrganization()
      .then((organization) => {
        if (!active) return;
        const activeIds = new Set(activeDecks.map((deck) => deck.id));
        setFolderScopes(
          organization.folders
            .map((folder) => ({
              folder,
              deckIds: organization.placements
                .filter(
                  (placement) =>
                    placement.folderId === folder.id && activeIds.has(placement.deckId),
                )
                .sort((left, right) => left.sortOrder - right.sortOrder)
                .map((placement) => placement.deckId),
            }))
            .filter((scope) => scope.deckIds.length > 0),
        );
      })
      .catch(() => {
        if (active) setFolderScopes([]);
      });
    return () => {
      active = false;
    };
  }, [activeDecks]);

  const activeFilters = useMemo<ActiveFilter[]>(() => {
    const output: ActiveFilter[] = [];
    if (deckIds.size > 0) {
      const selectedNames = activeDecks
        .filter((deck) => deckIds.has(deck.id))
        .map((deck) => deck.name);
      output.push({
        id: "decks",
        label:
          selectedNames.length === 1
            ? `Deck: ${selectedNames[0]}`
            : `${selectedNames.length} decks`,
      });
    }
    if (availabilityFilter !== "all") {
      output.push({
        id: "availability",
        label: availabilityFilter === "ready_now" ? "Ready now" : "Scheduled later",
      });
    }
    if (statusFilter !== "active") {
      output.push({ id: "status", label: statusLabel(statusFilter) });
    }
    if (reviewedFilter !== "all") {
      output.push({ id: "reviewed", label: reviewedLabel(reviewedFilter) });
    }
    if (tagIds.size > 0) {
      const selected = tags.filter((tag) => tagIds.has(tag.id)).map((tag) => tag.name);
      output.push({
        id: "tags",
        label: selected.length === 1 ? `Tag: ${selected[0]}` : `${selected.length} tags`,
      });
    }
    return output;
  }, [activeDecks, availabilityFilter, deckIds, reviewedFilter, statusFilter, tagIds, tags]);

  function removeFilter(id: string): void {
    if (id === "decks") onDeckIdsChange(new Set());
    if (id === "availability") onAvailabilityFilterChange("all");
    if (id === "status") onStatusFilterChange("active");
    if (id === "reviewed") onReviewedFilterChange("all");
    if (id === "tags") onTagIdsChange(new Set());
  }

  function clearFilters(): void {
    onDeckIdsChange(new Set());
    onAvailabilityFilterChange("all");
    onStatusFilterChange("active");
    onReviewedFilterChange("all");
    onTagIdsChange(new Set());
  }

  return (
    <section className={styles.root} aria-label="Card filters">
      <Surface level={2} material="solid" className={styles.primaryBar}>
        <label className={styles.searchField}>
          <span>Search cards</span>
          <span className={styles.searchControl}>
            <Icon name="search" size={18} />
            <input
              type="search"
              value={query}
              placeholder="Question, answer, or tag"
              onChange={(event) => onQueryChange(event.currentTarget.value)}
            />
          </span>
        </label>

        <button
          type="button"
          className={styles.scopeButton}
          onClick={() => setFiltersOpen(true)}
        >
          <Icon name="folder" size={17} />
          <span>{deckFilterSummary(deckIds, activeDecks)}</span>
        </button>

        <label className={styles.sortField}>
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
            <option value="due_asc">Next scheduled</option>
          </select>
        </label>

        <Button
          className={styles.filterButton}
          data-tour-anchor="search.filters"
          onClick={() => setFiltersOpen(true)}
        >
          <Icon name="filter" size={18} />
          All filters
          {activeFilters.length > 0 ? (
            <span className={styles.filterCount}>{activeFilters.length}</span>
          ) : null}
        </Button>
      </Surface>

      <FilterSummary filters={activeFilters} onRemove={removeFilter} onClear={clearFilters} />

      <AdaptiveSheet
        open={filtersOpen}
        title="Filter flashcards"
        description="Narrow the current library without changing cards or schedules."
        onClose={() => setFiltersOpen(false)}
      >
        <div className={styles.filterSheet}>
          <FilterGroup
            title="Scope"
            description="Choose all active decks, a folder, or individual decks."
          >
            <button
              type="button"
              className={styles.scopeChoice}
              aria-pressed={deckIds.size === 0}
              onClick={() => onDeckIdsChange(new Set())}
            >
              <span>All active decks</span>
              <small>
                {activeDecks.reduce((sum, deck) => sum + deck.generatedCardCount, 0)} cards
              </small>
            </button>
            {folderScopes.length > 0 ? (
              <div className={styles.folderChoices}>
                {folderScopes.map((scope) => {
                  const selected =
                    deckIds.size === scope.deckIds.length &&
                    scope.deckIds.every((id) => deckIds.has(id));
                  return (
                    <button
                      key={scope.folder.id}
                      type="button"
                      className={styles.scopeChoice}
                      aria-pressed={selected}
                      onClick={() => onDeckIdsChange(new Set(scope.deckIds))}
                    >
                      <Icon name="folder" size={17} />
                      <span>{scope.folder.name}</span>
                      <small>{scope.deckIds.length} decks</small>
                    </button>
                  );
                })}
              </div>
            ) : null}
            <div className={styles.checklist}>
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
          </FilterGroup>

          <FilterGroup
            title="Study state"
            description="Filter by availability, scheduling status, or review history."
            help
          >
            <div className={styles.controlGrid}>
              <label>
                <span>Availability</span>
                <select
                  value={availabilityFilter}
                  disabled={statusFilter === "suspended" || statusFilter === "buried"}
                  onChange={(event) =>
                    onAvailabilityFilterChange(
                      event.currentTarget.value as AvailabilityFilter,
                    )
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
                  onChange={(event) =>
                    onStatusFilterChange(event.currentTarget.value as CardStatusFilter)
                  }
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
                  onChange={(event) =>
                    onReviewedFilterChange(event.currentTarget.value as ReviewedFilter)
                  }
                >
                  <option value="all">Any local date</option>
                  <option value="never">Never studied</option>
                  <option value="today">Today</option>
                  <option value="past_7_local_dates">Past 7 local dates</option>
                  <option value="past_30_local_dates">Past 30 local dates</option>
                  <option value="before_past_30_local_dates">
                    Before the past 30 local dates
                  </option>
                </select>
              </label>
            </div>
          </FilterGroup>

          <FilterGroup
            title="Tags"
            description="A card can match one or more selected tags."
          >
            {tags.length === 0 ? (
              <p className={styles.emptyCopy}>No tags in the selected active decks.</p>
            ) : (
              <div className={styles.checklist}>
                {tags.map((tag) => (
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
                ))}
              </div>
            )}
          </FilterGroup>

          <div className={styles.sheetActions}>
            {activeFilters.length > 0 ? (
              <Button onClick={clearFilters}>Clear filters</Button>
            ) : null}
            <Button variant="primary" onClick={() => setFiltersOpen(false)}>
              View results
            </Button>
          </div>
        </div>
      </AdaptiveSheet>
    </section>
  );
}

function FilterGroup({
  title,
  description,
  help = false,
  children,
}: {
  title: string;
  description: string;
  help?: boolean;
  children: ReactNode;
}) {
  return (
    <fieldset className={styles.group}>
      <legend>
        <span>
          <strong>{title}</strong>
          <small>{description}</small>
        </span>
        {help ? (
          <HelpTrigger
            topicId="flashcards.search.studyState"
            label="Help with study-state filters"
          />
        ) : null}
      </legend>
      {children}
    </fieldset>
  );
}

function deckFilterSummary(deckIds: Set<string>, decks: DeckSummary[]): string {
  if (deckIds.size === 0) return "All active decks";
  const selectedNames = decks.filter((deck) => deckIds.has(deck.id)).map((deck) => deck.name);
  return selectedNames.length === 1
    ? (selectedNames[0] ?? "1 deck")
    : `${selectedNames.length} decks`;
}

function statusLabel(value: CardStatusFilter): string {
  switch (value) {
    case "active":
      return "Studyable";
    case "all":
      return "Any status";
    case "suspended":
      return "Suspended";
    case "buried":
      return "Buried";
    case "needs_more_practice":
      return "Needs more practice";
  }
}

function reviewedLabel(value: ReviewedFilter): string {
  switch (value) {
    case "all":
      return "Any study date";
    case "never":
      return "Never studied";
    case "today":
      return "Studied today";
    case "past_7_local_dates":
      return "Past 7 days";
    case "past_30_local_dates":
      return "Past 30 days";
    case "before_past_30_local_dates":
      return "Studied earlier";
  }
}
