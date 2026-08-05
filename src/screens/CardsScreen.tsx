import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { MutableRefObject } from "react";

import {
  applyScheduleAction,
  createMutationId,
  getErrorCode,
  getErrorMessage,
  getOrganizedCard,
  listMatchingCards,
  listTags,
  searchCards,
  type AvailabilityFilter,
  type CardBrowserRow,
  type CardSearchInput,
  type CardSearchPage,
  type CardSort,
  type CardStatusFilter,
  type CardTemplate,
  type Deck,
  type GenerateTemplateCardsResult,
  type OrganizedCard,
  type ReviewedFilter,
  type ScheduleAction,
  type SelectionCard,
  type TagSummary,
} from "../ipc";
import { CardEditorScreen } from "./CardEditorScreen";
import { BulkEntryModal } from "./cards/BulkEntryModal";
import { BulkTagModal } from "./cards/BulkTagModal";
import { DeleteCardsModal } from "./cards/DeleteCardsModal";
import { MoveCopyModal } from "./cards/MoveCopyModal";
import { PracticeResultsModal } from "./cards/PracticeResultsModal";
import { ScheduleActionModal } from "./cards/ScheduleActionModal";
import { scheduleActionNotice } from "./cards/scheduleActionResult";

interface CardsScreenProps {
  decks: Deck[];
  refreshDecks: () => Promise<Deck[]>;
  onPracticeCards: (cardIds: string[]) => void;
  onDirtyChange: (dirty: boolean) => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

type SelectionAction =
  | "move_copy"
  | "add_tags"
  | "remove_tags"
  | "delete"
  | "suspend"
  | "resume"
  | "bury"
  | "unbury"
  | "reschedule"
  | "reset";

interface PendingScheduleMutation {
  key: string;
  mutationId: string;
}

const PAGE_SIZE = 100;

export function CardsScreen({
  decks,
  refreshDecks,
  onPracticeCards,
  onDirtyChange,
  showToast,
}: CardsScreenProps) {
  const [deckIds, setDeckIds] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState("");
  const [availabilityFilter, setAvailabilityFilter] =
    useState<AvailabilityFilter>("all");
  const [statusFilter, setStatusFilter] =
    useState<CardStatusFilter>("active");
  const [reviewedFilter, setReviewedFilter] =
    useState<ReviewedFilter>("all");
  const [sort, setSort] = useState<CardSort>("updated_desc");
  const [tagIds, setTagIds] = useState<Set<string>>(new Set());
  const [tags, setTags] = useState<TagSummary[]>([]);
  const [page, setPage] = useState<CardSearchPage>({
    cards: [],
    totalCount: 0,
  });
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [isSelectingAll, setIsSelectingAll] = useState(false);
  const [isResolvingPractice, setIsResolvingPractice] = useState(false);
  const [isApplyingSchedule, setIsApplyingSchedule] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [selectedCards, setSelectedCards] = useState<
    Map<string, SelectionCard>
  >(new Map());
  const [editingCard, setEditingCard] = useState<OrganizedCard | null>(null);
  const [creatingDeckId, setCreatingDeckId] = useState<string | null>(null);
  const [isOpeningCard, setIsOpeningCard] = useState(false);
  const [bulkOpen, setBulkOpen] = useState(false);
  const [selectionAction, setSelectionAction] =
    useState<SelectionAction | null>(null);
  const [moveCopyCards, setMoveCopyCards] = useState<SelectionCard[]>([]);
  const [practiceCards, setPracticeCards] =
    useState<SelectionCard[] | null>(null);
  const [scheduleActionCards, setScheduleActionCards] = useState<
    SelectionCard[]
  >([]);
  const [scheduleAction, setScheduleAction] = useState<
    "reset" | "reschedule" | null
  >(null);
  const pendingScheduleMutationRef =
    useRef<PendingScheduleMutation | null>(null);
  const searchRequestRef = useRef(0);

  const loadRequest = useMemo<CardSearchInput>(
    () => ({
      deckIds: [...deckIds].sort(),
      query,
      tagIds: [...tagIds].sort(),
      availabilityFilter,
      statusFilter,
      reviewedFilter,
      sort,
      limit: PAGE_SIZE,
      offset: 0,
    }),
    [
      availabilityFilter,
      deckIds,
      query,
      reviewedFilter,
      sort,
      statusFilter,
      tagIds,
    ],
  );

  const load = useCallback(async () => {
    const requestId = ++searchRequestRef.current;
    setIsLoading(true);
    setErrorMessage(null);
    try {
      const result = await searchCards(loadRequest);
      if (searchRequestRef.current !== requestId) return;
      setPage(result);
      setSelectedCards(new Map());
    } catch (error: unknown) {
      if (searchRequestRef.current === requestId) {
        setErrorMessage(getErrorMessage(error));
      }
    } finally {
      if (searchRequestRef.current === requestId) setIsLoading(false);
    }
  }, [loadRequest]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    let current = true;
    void listTags([...deckIds])
      .then((loaded) => {
        if (!current) return;
        setTags(loaded);
        const available = new Set(loaded.map((tag) => tag.id));
        setTagIds((selected) => {
          const next = new Set(
            [...selected].filter((id) => available.has(id)),
          );
          return sameSet(selected, next) ? selected : next;
        });
      })
      .catch((error: unknown) => {
        if (current) showToast("error", getErrorMessage(error));
      });
    return () => {
      current = false;
    };
  }, [deckIds, showToast]);

  async function loadMore(): Promise<void> {
    if (isLoadingMore || page.cards.length >= page.totalCount) return;
    const requestId = searchRequestRef.current;
    setIsLoadingMore(true);
    try {
      const next = await searchCards({
        ...loadRequest,
        offset: page.cards.length,
      });
      if (searchRequestRef.current !== requestId) return;
      setPage((current) => ({
        cards: [...current.cards, ...next.cards],
        totalCount: next.totalCount,
      }));
    } catch (error: unknown) {
      if (searchRequestRef.current === requestId) {
        showToast("error", getErrorMessage(error));
      }
    } finally {
      if (searchRequestRef.current === requestId) setIsLoadingMore(false);
    }
  }

  async function selectAllMatching(): Promise<void> {
    if (isSelectingAll) return;
    setIsSelectingAll(true);
    try {
      const matching = await listMatchingCards({
        ...loadRequest,
        limit: PAGE_SIZE,
        offset: 0,
      });
      setSelectedCards(
        new Map(matching.map((card) => [card.cardId, card])),
      );
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      setIsSelectingAll(false);
    }
  }

  function toggleVisible(): void {
    setSelectedCards((current) => {
      const next = new Map(current);
      const allVisibleSelected = page.cards.every((card) =>
        next.has(card.cardId),
      );
      for (const card of page.cards) {
        if (allVisibleSelected) next.delete(card.cardId);
        else next.set(card.cardId, selectionFromRow(card));
      }
      return next;
    });
  }

  async function openCard(noteId: string): Promise<void> {
    if (isOpeningCard) return;
    setIsOpeningCard(true);
    try {
      setEditingCard(await getOrganizedCard(noteId));
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      setIsOpeningCard(false);
    }
  }

  function beginCreate(): void {
    if (deckIds.size !== 1) return;
    setCreatingDeckId([...deckIds][0] ?? null);
  }

  async function resolveSelectedPractice(): Promise<void> {
    if (selectedCards.size === 0 || isResolvingPractice) return;
    setIsResolvingPractice(true);
    try {
      setPracticeCards([...selectedCards.values()]);
    } finally {
      setIsResolvingPractice(false);
    }
  }

  function closePracticeResults(): void {
    setPracticeCards(null);
  }

  function runSelectionAction(action: SelectionAction): void {
    const cards = [...selectedCards.values()];
    if (cards.length === 0) return;
    if (
      action === "suspend" ||
      action === "resume" ||
      action === "bury" ||
      action === "unbury"
    ) {
      void applySelectedScheduleAction(cards, action);
      return;
    }
    if (action === "reset" || action === "reschedule") {
      setScheduleActionCards(cards);
      setScheduleAction(action);
      return;
    }
    if (action === "move_copy") {
      setMoveCopyCards(cards);
      return;
    }
    setSelectionAction(action);
  }

  async function applySelectedScheduleAction(
    cards: SelectionCard[],
    action: Extract<
      ScheduleAction,
      "suspend" | "resume" | "bury" | "unbury"
    >,
  ): Promise<void> {
    if (isApplyingSchedule) return;
    const cardIds = [...new Set(cards.map((card) => card.cardId))].sort();
    const key = JSON.stringify({ action, cardIds });
    const mutationId = mutationIdForScheduleRequest(
      pendingScheduleMutationRef,
      key,
    );
    setIsApplyingSchedule(true);
    let result;
    try {
      result = await applyScheduleAction({ mutationId, cardIds, action });
      pendingScheduleMutationRef.current = null;
    } catch (error: unknown) {
      if (getErrorCode(error) !== null) {
        pendingScheduleMutationRef.current = null;
      }
      showToast(
        "error",
        getErrorCode(error) === null
          ? `${getErrorMessage(error)} Choose the same action again to safely retry the complete selection.`
          : getErrorMessage(error),
      );
      setIsApplyingSchedule(false);
      return;
    }
    const notice = scheduleActionNotice(result, cardIds.length);
    showToast(notice.kind, notice.text);
    try {
      await refreshAfterMutation();
    } finally {
      setIsApplyingSchedule(false);
    }
  }

  async function refreshAfterMutation(): Promise<void> {
    try {
      await Promise.all([load(), refreshDecks()]);
    } catch (error: unknown) {
      showToast(
        "info",
        `The change was saved, but the current view could not refresh: ${getErrorMessage(error)}`,
      );
    }
  }

  if (editingCard || creatingDeckId) {
    const cardDeckId = editingCard?.deckId ?? creatingDeckId;
    const deck = decks.find((item) => item.id === cardDeckId);
    if (deck) {
      return (
        <CardEditorScreen
          deck={deck}
          decks={decks}
          card={editingCard}
          onDirtyChange={onDirtyChange}
          onCancel={() => {
            onDirtyChange(false);
            setEditingCard(null);
            setCreatingDeckId(null);
          }}
          onSaved={async () => {
            onDirtyChange(false);
            setEditingCard(null);
            setCreatingDeckId(null);
            showToast(
              "success",
              editingCard ? "Flashcard updated." : "Flashcard saved.",
            );
            await refreshAfterMutation();
          }}
          onTemplateGenerated={async (
            result: GenerateTemplateCardsResult,
            continueAdding: boolean,
            template: CardTemplate,
          ) => {
            onDirtyChange(false);
            if (!continueAdding) {
              setEditingCard(null);
              setCreatingDeckId(null);
            }
            showToast(
              result.createdCount > 0 ? "success" : "info",
              `Generated ${result.createdCount} ${
                result.createdCount === 1 ? "card" : "cards"
              } from “${template.name}”.`,
            );
            await refreshAfterMutation();
          }}
        />
      );
    }
  }

  const allVisibleSelected =
    page.cards.length > 0 &&
    page.cards.every((card) => selectedCards.has(card.cardId));

  return (
    <div className="screen cards-screen">
      <header className="screen-header compact-screen-header">
        <div>
          <h1>Search / Filter</h1>
          <p>
            Find exact scheduled card presentations and manage them in bounded
            pages.
          </p>
        </div>
        <div className="header-actions">
          <button
            type="button"
            className="secondary-button"
            onClick={() => setBulkOpen(true)}
          >
            Bulk entry
          </button>
          <button
            type="button"
            className="primary-button"
            disabled={deckIds.size !== 1}
            onClick={beginCreate}
          >
            Add card
          </button>
        </div>
      </header>

      <section className="card-search-layout">
        <aside className="card-filter-sidebar">
          <label className="search-field">
            <span>Search</span>
            <input
              value={query}
              onChange={(event) => setQuery(event.currentTarget.value)}
              placeholder="Question, answer, deck, or tag"
            />
          </label>

          <details className="filter-disclosure">
            <summary>All decks</summary>
            <FilterChecklist
              label="Decks"
              options={decks.map((deck) => ({
                id: deck.id,
                name: deck.name,
                detail: `${deck.generatedCardCount} cards`,
              }))}
              selected={deckIds}
              onChange={setDeckIds}
            />
          </details>

          <label>
            <span>Availability</span>
            <select
              value={availabilityFilter}
              onChange={(event) =>
                setAvailabilityFilter(
                  event.currentTarget.value as AvailabilityFilter,
                )
              }
            >
              <option value="all">All cards</option>
              <option value="ready_now">Ready now</option>
              <option value="scheduled_later">Scheduled later</option>
            </select>
          </label>

          <label>
            <span>Status</span>
            <select
              value={statusFilter}
              onChange={(event) =>
                setStatusFilter(event.currentTarget.value as CardStatusFilter)
              }
            >
              <option value="all">All statuses</option>
              <option value="active">Active</option>
              <option value="suspended">Suspended</option>
              <option value="buried">Buried</option>
              <option value="needs_more_practice">Needs more practice</option>
            </select>
          </label>

          <label>
            <span>Studied</span>
            <select
              value={reviewedFilter}
              onChange={(event) =>
                setReviewedFilter(event.currentTarget.value as ReviewedFilter)
              }
            >
              <option value="all">Any time</option>
              <option value="never">Never</option>
              <option value="today">Today</option>
              <option value="past_7_dates">Past 7 local dates</option>
              <option value="past_30_dates">Past 30 local dates</option>
              <option value="before_30_dates">
                Before the past 30 local dates
              </option>
            </select>
          </label>

          <FilterChecklist
            label="Tags"
            options={tags.map((tag) => ({
              id: tag.id,
              name: tag.name,
              detail: `${tag.sourceNoteCount} notes`,
            }))}
            selected={tagIds}
            onChange={setTagIds}
          />
        </aside>

        <section className="card-results-panel">
          <div className="card-results-toolbar">
            <span>{page.totalCount} matching scheduled cards</span>
            <select
              aria-label="Sort cards"
              value={sort}
              onChange={(event) =>
                setSort(event.currentTarget.value as CardSort)
              }
            >
              <option value="updated_desc">Recently updated</option>
              <option value="question_asc">Question A-Z</option>
              <option value="due_asc">Next due</option>
              <option value="last_reviewed_desc">Recently studied</option>
              <option value="relevance">Relevance</option>
            </select>
          </div>
          {errorMessage ? (
            <div className="inline-alert error-alert">{errorMessage}</div>
          ) : null}
          {isLoading ? (
            <div className="inline-progress">Searching cards…</div>
          ) : null}
          {!isLoading && page.cards.length === 0 ? (
            <div className="compact-empty-state">
              <h2>No cards match</h2>
            </div>
          ) : null}
          {page.cards.length > 0 ? (
            <CardResultsTable
              cards={page.cards}
              selectedCards={selectedCards}
              onSelectedCardsChange={setSelectedCards}
              onOpenCard={openCard}
            />
          ) : null}
          {page.cards.length < page.totalCount ? (
            <button
              type="button"
              className="secondary-button"
              disabled={isLoadingMore}
              onClick={() => void loadMore()}
            >
              {isLoadingMore ? "Loading…" : "Load more"}
            </button>
          ) : null}
        </section>
      </section>

      {selectedCards.size > 0 ? (
        <section className="selection-action-bar">
          <strong>{selectedCards.size} selected</strong>
          <button type="button" onClick={toggleVisible}>
            {allVisibleSelected ? "Clear visible" : "Select visible"}
          </button>
          <button
            type="button"
            disabled={isSelectingAll}
            onClick={() => void selectAllMatching()}
          >
            {isSelectingAll
              ? "Selecting…"
              : `Select all (${page.totalCount})`}
          </button>
          <select
            aria-label="Actions for selected cards"
            defaultValue=""
            onChange={(event) => {
              const action = event.currentTarget.value as SelectionAction;
              if (action) runSelectionAction(action);
              event.currentTarget.value = "";
            }}
          >
            <option value="">Choose action</option>
            <option value="move_copy">Move / copy</option>
            <option value="add_tags">Add tags</option>
            <option value="remove_tags">Remove tags</option>
            <option value="suspend">Suspend</option>
            <option value="resume">Resume</option>
            <option value="bury">Bury until tomorrow</option>
            <option value="unbury">Unbury</option>
            <option value="reschedule">Reschedule</option>
            <option value="reset">Reset schedule</option>
            <option value="delete">Delete</option>
          </select>
          <button
            type="button"
            className="primary-button"
            disabled={isResolvingPractice}
            onClick={() => void resolveSelectedPractice()}
          >
            {isResolvingPractice ? "Preparing…" : "Practice selected"}
          </button>
        </section>
      ) : null}

      {bulkOpen ? (
        <BulkEntryModal
          decks={decks}
          defaultDeckId={deckIds.size === 1 ? [...deckIds][0] : undefined}
          onClose={() => setBulkOpen(false)}
          onSaved={async () => {
            setBulkOpen(false);
            await refreshAfterMutation();
          }}
          showToast={showToast}
        />
      ) : null}
      {moveCopyCards.length > 0 ? (
        <MoveCopyModal
          cards={moveCopyCards}
          decks={decks}
          onClose={() => setMoveCopyCards([])}
          onComplete={async () => {
            setMoveCopyCards([]);
            await refreshAfterMutation();
          }}
        />
      ) : null}
      {selectionAction === "add_tags" ||
      selectionAction === "remove_tags" ? (
        <BulkTagModal
          cards={[...selectedCards.values()]}
          mode={selectionAction === "add_tags" ? "add" : "remove"}
          suggestions={tags}
          onClose={() => setSelectionAction(null)}
          onComplete={async () => {
            setSelectionAction(null);
            await refreshAfterMutation();
          }}
          showToast={showToast}
        />
      ) : null}
      {selectionAction === "delete" ? (
        <DeleteCardsModal
          cards={[...selectedCards.values()]}
          onClose={() => setSelectionAction(null)}
          onComplete={refreshAfterMutation}
          showToast={showToast}
        />
      ) : null}
      {scheduleAction ? (
        <ScheduleActionModal
          cards={scheduleActionCards}
          action={scheduleAction}
          onClose={() => {
            setScheduleAction(null);
            setScheduleActionCards([]);
          }}
          onComplete={refreshAfterMutation}
          showToast={showToast}
        />
      ) : null}
      {practiceCards ? (
        <PracticeResultsModal
          cards={practiceCards}
          sourceLabel="the current filtered results"
          refreshDecks={refreshDecks}
          onPracticeCards={onPracticeCards}
          onClose={closePracticeResults}
          showToast={showToast}
        />
      ) : null}
    </div>
  );
}

interface CardResultsTableProps {
  cards: CardBrowserRow[];
  selectedCards: Map<string, SelectionCard>;
  onSelectedCardsChange: (cards: Map<string, SelectionCard>) => void;
  onOpenCard: (noteId: string) => void;
}

function CardResultsTable({
  cards,
  selectedCards,
  onSelectedCardsChange,
  onOpenCard,
}: CardResultsTableProps) {
  return (
    <div className="card-results-table">
      {cards.map((card) => (
        <article key={card.cardId} className="search-card-row">
          <input
            type="checkbox"
            aria-label={`Select ${card.frontText || "card"}`}
            checked={selectedCards.has(card.cardId)}
            onChange={(event) => {
              const next = new Map(selectedCards);
              if (event.currentTarget.checked) {
                next.set(card.cardId, selectionFromRow(card));
              } else {
                next.delete(card.cardId);
              }
              onSelectedCardsChange(next);
            }}
          />
          <button
            type="button"
            className="search-card-main"
            onClick={() => onOpenCard(card.noteId)}
          >
            <span>
              <strong>{card.frontText || "Image card"}</strong>
              <small>
                {card.cardLabel} · {card.deckName}
              </small>
            </span>
            <span>{card.backText}</span>
          </button>
          <span>
            {card.needsMorePractice ? "Needs practice" : formatDue(card.dueAt)}
          </span>
        </article>
      ))}
    </div>
  );
}

interface FilterChecklistProps {
  label: string;
  options: { id: string; name: string; detail: string }[];
  selected: Set<string>;
  onChange: (selected: Set<string>) => void;
}

function FilterChecklist({
  label,
  options,
  selected,
  onChange,
}: FilterChecklistProps) {
  return (
    <fieldset className="filter-checklist">
      <legend>{label}</legend>
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
            {option.name}
            <small>{option.detail}</small>
          </span>
        </label>
      ))}
    </fieldset>
  );
}

function selectionFromRow(card: CardBrowserRow): SelectionCard {
  return {
    cardId: card.cardId,
    noteId: card.noteId,
    deckId: card.deckId,
  };
}

function sameSet(left: Set<string>, right: Set<string>): boolean {
  return left.size === right.size && [...left].every((value) => right.has(value));
}

function formatDue(value: number): string {
  const delta = value - Date.now();
  if (delta <= 0) return "Ready now";
  const days = Math.max(1, Math.round(delta / 86_400_000));
  return `Due in ${days}d`;
}

function mutationIdForScheduleRequest(
  ref: MutableRefObject<PendingScheduleMutation | null>,
  key: string,
): string {
  if (ref.current?.key === key) return ref.current.mutationId;
  const mutationId = createMutationId();
  ref.current = { key, mutationId };
  return mutationId;
}
