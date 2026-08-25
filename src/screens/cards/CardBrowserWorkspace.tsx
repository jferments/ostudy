import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { MutableRefObject } from "react";

import type { ShowToast } from "../../components/Toast";
import { CardsController } from "../../controllers/cardsController";
import { reconcileCommitted } from "../../controllers/reconciliation";
import {
  type AvailabilityFilter,
  type CardBrowserRow,
  type CardNote,
  type CardSearchInput,
  type CardSearchPage,
  type CardSort,
  type CardStatusFilter,
  type ReviewedFilter,
  type SelectionCard,
  type TagSummary,
} from "../../ipc/cards";
import { createMutationId, getErrorCode, getErrorMessage } from "../../ipc/common";
import type { DeckSummary } from "../../ipc/decks";
import type { ScheduleAction } from "../../ipc/scheduling";
import type { CardTemplate, GenerateTemplateCardsResult } from "../../ipc/templates";
import { CardEditorScreen } from "../CardEditorScreen";
import { BulkEntryModal } from "./BulkEntryModal";
import { BulkTagModal } from "./BulkTagModal";
import { CardFilters } from "./CardFilters";
import { DeleteCardsModal } from "./DeleteCardsModal";
import { MoveCopyModal, type TransferableCard } from "./MoveCopyModal";
import { PracticeResultsModal } from "./PracticeResultsModal";
import { ScheduleActionModal } from "./ScheduleActionModal";
import { scheduleActionNotice } from "./scheduleActionResult";
import { ownedClassNames, sharedClassMaps } from "../../ui/styles/classOwnership";
import ownershipStyles from "./CardBrowserWorkspace.module.css";

const ownedStyleMaps = [...sharedClassMaps, ownershipStyles] as const;


export type CardBrowserView = "list" | "editor";
export type CardBrowserControls = "deck" | "search";

interface CardBrowserWorkspaceProps {
  decks: DeckSummary[];
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeCards: (cardIds: string[]) => void | Promise<void>;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
  fixedDeckId?: string;
  controls?: CardBrowserControls;
  showFilters?: boolean;
  showPageHeader?: boolean;
  onViewChange?: (view: CardBrowserView) => void;
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

/**
 * The single card-listing workspace used by both global Search/Filter and an
 * open deck. Both callers use the same native search contract, pagination,
 * row markup, selection semantics, card editing, and bulk operations. The
 * control mode intentionally varies only the screen-specific controls above
 * that shared behavior: Search/Filter gets the complete filter model, while a
 * deck gets a fixed scope, one lightweight search field, and authoring actions.
 */
export function CardBrowserWorkspace({
  decks,
  refreshDecks,
  onPracticeCards,
  onDirtyChange,
  showToast,
  fixedDeckId,
  controls,
  showFilters,
  showPageHeader = fixedDeckId === undefined,
  onViewChange,
}: CardBrowserWorkspaceProps) {
  const controlMode: CardBrowserControls =
    controls ??
    (showFilters === undefined
      ? fixedDeckId === undefined
        ? "search"
        : "deck"
      : showFilters
        ? "search"
        : "deck");
  const activeDecks = useMemo(
    () => decks.filter((deck) => deck.archivedAt === null),
    [decks],
  );
  const [filterDeckIds, setFilterDeckIds] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState("");
  const [availabilityFilter, setAvailabilityFilter] =
    useState<AvailabilityFilter>("all");
  const [statusFilter, setStatusFilter] = useState<CardStatusFilter>(() =>
    fixedDeckId ? "all" : "active",
  );
  const [reviewedFilter, setReviewedFilter] = useState<ReviewedFilter>("all");
  const [sort, setSort] = useState<CardSort>("updated_desc");
  const [tagIds, setTagIds] = useState<Set<string>>(new Set());
  const [tags, setTags] = useState<TagSummary[]>([]);
  const [page, setPage] = useState<CardSearchPage>({ cards: [], totalCount: 0 });
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [isSelectingAll, setIsSelectingAll] = useState(false);
  const [isApplyingSchedule, setIsApplyingSchedule] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [selectedCards, setSelectedCards] = useState<Map<string, SelectionCard>>(new Map());
  const [editingCard, setEditingCard] = useState<CardNote | null>(null);
  const [creatingDeckId, setCreatingDeckId] = useState<string | null>(null);
  const [isOpeningCard, setIsOpeningCard] = useState(false);
  const [bulkEntryOpen, setBulkEntryOpen] = useState(false);
  const [selectionAction, setSelectionAction] = useState<SelectionAction | null>(null);
  const [moveCopyCards, setMoveCopyCards] = useState<TransferableCard[]>([]);
  const [practiceCards, setPracticeCards] = useState<SelectionCard[] | null>(null);
  const pendingScheduleMutationRef = useRef<PendingScheduleMutation | null>(null);
  const controllerRef = useRef<CardsController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new CardsController();
  const controller = controllerRef.current;

  const scopedDeckIds = useMemo(
    () => (fixedDeckId ? [fixedDeckId] : [...filterDeckIds]),
    [filterDeckIds, fixedDeckId],
  );
  const deckIds = useMemo(() => new Set(scopedDeckIds), [scopedDeckIds]);
  const normalizedSort: CardSort =
    query.trim() === "" && sort === "relevance" ? "updated_desc" : sort;
  const searchInput = useMemo<CardSearchInput>(
    () => ({
      deckIds: scopedDeckIds,
      query,
      tagIds: [...tagIds],
      availabilityFilter,
      statusFilter,
      reviewedFilter,
      sort: normalizedSort,
      limit: PAGE_SIZE,
      offset: 0,
    }),
    [
      availabilityFilter,
      normalizedSort,
      query,
      reviewedFilter,
      scopedDeckIds,
      statusFilter,
      tagIds,
    ],
  );
  const selected = useMemo(() => [...selectedCards.values()], [selectedCards]);
  const selectedNotes = useMemo(() => uniqueNotes(selected), [selected]);
  const selectedDeck =
    scopedDeckIds.length === 1
      ? (activeDecks.find((deck) => deck.id === scopedDeckIds[0]) ?? null)
      : null;
  const browserView: CardBrowserView = editingCard || creatingDeckId ? "editor" : "list";
  const practiceSourceLabel = fixedDeckId
    ? `the “${selectedDeck?.name ?? "current"}” deck`
    : "Search/Filter";

  const loadTags = useCallback(async (): Promise<void> => {
    try {
      const loaded = await controller.tags(scopedDeckIds);
      if (!loaded) return;
      setTags(loaded);
      setTagIds((current) => {
        const valid = new Set(loaded.map((tag) => tag.id));
        return new Set([...current].filter((id) => valid.has(id)));
      });
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    }
  }, [controller, scopedDeckIds]);

  const loadCards = useCallback(
    async (offset = 0, append = false): Promise<void> => {
      if (append) setIsLoadingMore(true);
      else {
        setIsLoading(true);
        setIsLoadingMore(false);
        setPage({ cards: [], totalCount: 0 });
      }
      setErrorMessage(null);
      try {
        const result = await controller.search({ ...searchInput, offset });
        if (!result) return;
        setPage((current) => ({
          totalCount: result.totalCount,
          cards: append ? [...current.cards, ...result.cards] : result.cards,
        }));
      } catch (error: unknown) {
        setErrorMessage(getErrorMessage(error));
      } finally {
        setIsLoading(false);
        setIsLoadingMore(false);
      }
    },
    [controller, searchInput],
  );

  useEffect(() => () => controller.invalidate(), [controller]);

  useEffect(() => {
    onViewChange?.(browserView);
  }, [browserView, onViewChange]);

  useEffect(() => {
    if (fixedDeckId) return;
    setFilterDeckIds((current) => {
      const activeIds = new Set(activeDecks.map((deck) => deck.id));
      return new Set([...current].filter((id) => activeIds.has(id)));
    });
  }, [activeDecks, fixedDeckId]);

  useEffect(() => {
    void loadTags();
  }, [loadTags]);

  useEffect(() => {
    if (query.trim() === "" && sort === "relevance") setSort("updated_desc");
  }, [query, sort]);

  useEffect(() => {
    if (statusFilter === "suspended" || statusFilter === "buried") {
      setAvailabilityFilter("all");
    }
  }, [statusFilter]);

  useEffect(() => {
    setSelectedCards(new Map());
    const timeout = window.setTimeout(() => void loadCards(), 180);
    return () => window.clearTimeout(timeout);
  }, [loadCards]);

  function changeQuery(value: string): void {
    setQuery(value);
    if (value.trim() !== "" && sort === "updated_desc") setSort("relevance");
  }

  async function refreshAfterMutation(): Promise<void> {
    await Promise.all([loadCards(), loadTags(), refreshDecks()]);
    setSelectedCards(new Map());
  }

  async function reconcileAfterCommit(subject: string): Promise<void> {
    const outcome = await reconcileCommitted(undefined, refreshAfterMutation);
    if (outcome.reconciliationError) {
      showToast(
        "info",
        `${subject} was saved, but the current card list could not refresh: ${outcome.reconciliationError}`,
      );
    }
  }

  async function completeSelectionMutation(): Promise<void> {
    if (editingCard || creatingDeckId) {
      onDirtyChange(false);
      setEditingCard(null);
      setCreatingDeckId(null);
    }
    await refreshAfterMutation();
  }

  async function openCard(noteId: string): Promise<void> {
    setIsOpeningCard(true);
    setErrorMessage(null);
    try {
      const card = await controller.note(noteId);
      if (!card) return;
      setEditingCard(card);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsOpeningCard(false);
    }
  }

  function beginCreateCard(): void {
    if (!selectedDeck) return;
    setEditingCard(null);
    setCreatingDeckId(selectedDeck.id);
  }

  function toggleSelected(card: SelectionCard): void {
    setSelectedCards((current) => {
      const next = new Map(current);
      if (next.has(card.cardId)) next.delete(card.cardId);
      else next.set(card.cardId, card);
      return next;
    });
  }

  function selectVisible(): void {
    setSelectedCards((current) => {
      const next = new Map(current);
      for (const card of page.cards) next.set(card.cardId, selectionFromRow(card));
      return next;
    });
  }

  async function selectAllMatching(): Promise<void> {
    if (page.totalCount === 0 || isSelectingAll) return;
    setIsSelectingAll(true);
    setErrorMessage(null);
    try {
      const matches = await controller.matching(searchInput);
      if (!matches) return;
      setSelectedCards(new Map(matches.map((card) => [card.cardId, card])));
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsSelectingAll(false);
    }
  }

  function beginMoveCopy(cards: TransferableCard[]): void {
    const notes = [...new Map(cards.map((card) => [card.noteId, card])).values()];
    if (notes.length === 0) return;
    setMoveCopyCards(notes);
    setSelectionAction("move_copy");
  }

  async function applySimpleScheduleAction(
    cards: SelectionCard[],
    action: Exclude<ScheduleAction, "reset" | "reschedule">,
  ): Promise<void> {
    if (cards.length === 0 || isApplyingSchedule) return;
    const cardIds = [...new Set(cards.map((card) => card.cardId))].sort();
    const key = JSON.stringify({ action, cardIds });
    const mutationId = mutationIdForSchedule(pendingScheduleMutationRef, key);
    setIsApplyingSchedule(true);
    setErrorMessage(null);

    let result;
    try {
      result = await controller.schedule({ mutationId, cardIds, action });
      pendingScheduleMutationRef.current = null;
    } catch (error: unknown) {
      if (getErrorCode(error) !== null) {
        pendingScheduleMutationRef.current = null;
        setErrorMessage(getErrorMessage(error));
      } else {
        setErrorMessage(
          `${getErrorMessage(error)} Choose the same schedule action again to safely retry the complete selection.`,
        );
      }
      setIsApplyingSchedule(false);
      return;
    }

    const notice = scheduleActionNotice(result, cardIds.length);
    showToast(notice.kind, notice.text);
    await reconcileAfterCommit("The schedule change");
    setIsApplyingSchedule(false);
  }

  function chooseSelectionAction(action: SelectionAction): void {
    if (action === "move_copy") beginMoveCopy(selected);
    else if (["suspend", "resume", "bury", "unbury"].includes(action)) {
      void applySimpleScheduleAction(
        selected,
        action as "suspend" | "resume" | "bury" | "unbury",
      );
    } else setSelectionAction(action);
  }

  function renderSelectionModal() {
    if (selectionAction === "move_copy") {
      return (
        <MoveCopyModal
          cards={moveCopyCards}
          decks={activeDecks}
          onClose={() => {
            setSelectionAction(null);
            setMoveCopyCards([]);
          }}
          onComplete={completeSelectionMutation}
          showToast={showToast}
        />
      );
    }
    if (selectionAction === "add_tags" || selectionAction === "remove_tags") {
      return (
        <BulkTagModal
          cards={selectedNotes}
          mode={selectionAction === "add_tags" ? "add" : "remove"}
          suggestions={tags}
          onClose={() => setSelectionAction(null)}
          onComplete={completeSelectionMutation}
          showToast={showToast}
        />
      );
    }
    if (selectionAction === "delete") {
      return (
        <DeleteCardsModal
          cards={selectedNotes}
          onClose={() => setSelectionAction(null)}
          onComplete={completeSelectionMutation}
          showToast={showToast}
        />
      );
    }
    if (selectionAction === "reset" || selectionAction === "reschedule") {
      return (
        <ScheduleActionModal
          cards={selected}
          action={selectionAction}
          onClose={() => setSelectionAction(null)}
          onComplete={refreshAfterMutation}
          showToast={showToast}
        />
      );
    }
    return null;
  }

  if (browserView === "editor") {
    const editorDeckId = editingCard?.deckId ?? creatingDeckId;
    const deck = activeDecks.find((candidate) => candidate.id === editorDeckId);
    if (!deck) {
      return (
        <div className={ownedClassNames(ownedStyleMaps, "screen cards-screen")}>
          <div className={ownedClassNames(ownedStyleMaps, "inline-alert error-alert")}>
            The card’s active deck could not be found. Return to the card list and refresh.
          </div>
          <button
            type="button"
            className={ownedClassNames(ownedStyleMaps, "secondary-button")}
            onClick={() => {
              setEditingCard(null);
              setCreatingDeckId(null);
            }}
          >
            Back to cards
          </button>
        </div>
      );
    }
    return (
      <>
        <CardEditorScreen
          deck={deck}
          decks={activeDecks}
          card={editingCard}
          onDirtyChange={onDirtyChange}
          onCancel={() => {
            onDirtyChange(false);
            setEditingCard(null);
            setCreatingDeckId(null);
          }}
          onMoveCopy={
            editingCard
              ? () =>
                  beginMoveCopy([
                    {
                      noteId: editingCard.noteId,
                      deckId: editingCard.deckId,
                    },
                  ])
              : undefined
          }
          onSaved={async (saved, continueAdding) => {
            onDirtyChange(false);
            const wasEditing = editingCard !== null;
            if (!continueAdding) {
              setEditingCard(null);
              setCreatingDeckId(null);
            }
            showToast("success", wasEditing ? "Flashcard updated." : "Flashcard saved.");
            if (saved.deckId !== deck.id) showToast("info", "The card is now in a different deck.");
            await reconcileAfterCommit("The flashcard");
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
            const duplicateMessage =
              result.skippedDuplicates > 0
                ? ` ${result.skippedDuplicates} exact ${
                    result.skippedDuplicates === 1 ? "duplicate was" : "duplicates were"
                  } skipped.`
                : "";
            showToast(
              result.createdCount > 0 ? "success" : "info",
              `Generated ${result.createdCount} ${
                result.createdCount === 1 ? "card" : "cards"
              } from “${template.name}”.${duplicateMessage}`,
            );
            await reconcileAfterCommit("The generated cards");
          }}
        />
        {renderSelectionModal()}
      </>
    );
  }

  const listContent = (
    <>
      {controlMode === "search" ? (
        <CardFilters
          activeDecks={activeDecks}
          deckIds={deckIds}
          query={query}
          availabilityFilter={availabilityFilter}
          statusFilter={statusFilter}
          reviewedFilter={reviewedFilter}
          sort={sort}
          tags={tags}
          tagIds={tagIds}
          onDeckIdsChange={setFilterDeckIds}
          onQueryChange={changeQuery}
          onAvailabilityFilterChange={setAvailabilityFilter}
          onStatusFilterChange={setStatusFilter}
          onReviewedFilterChange={setReviewedFilter}
          onSortChange={setSort}
          onTagIdsChange={setTagIds}
        />
      ) : (
        <section className={ownedClassNames(ownedStyleMaps, "deck-card-search")} aria-label="Search this deck">
          <label>
            <span>Search cards</span>
            <input
              type="search"
              value={query}
              placeholder="Question, answer, or tag"
              onChange={(event) => changeQuery(event.currentTarget.value)}
            />
          </label>
        </section>
      )}

      {selectedCards.size > 0 ? (
        <section className={ownedClassNames(ownedStyleMaps, "card-selection-bar")} aria-live="polite">
          <strong>{selectedCards.size} selected</strong>
          <select
            aria-label="Actions for selected cards"
            value=""
            disabled={isApplyingSchedule}
            onChange={(event) => {
              const action = event.currentTarget.value as SelectionAction | "";
              if (action !== "") chooseSelectionAction(action);
            }}
          >
            <option value="">Actions…</option>
            <option value="move_copy">Move / copy cards</option>
            <option value="add_tags">Add tags</option>
            <option value="remove_tags">Remove tags</option>
            <option value="suspend">Suspend cards</option>
            <option value="resume">Resume cards</option>
            <option value="bury">Bury until tomorrow</option>
            <option value="unbury">Unbury cards</option>
            <option value="reschedule">Reschedule cards</option>
            <option value="reset">Reset schedules</option>
            <option value="delete">Delete cards</option>
          </select>
          <button type="button" className={ownedClassNames(ownedStyleMaps, "text-button")} onClick={() => setSelectedCards(new Map())}>
            Deselect all
          </button>
        </section>
      ) : null}

      {errorMessage ? <div className={ownedClassNames(ownedStyleMaps, "inline-alert error-alert")}>{errorMessage}</div> : null}
      {isOpeningCard ? <div className={ownedClassNames(ownedStyleMaps, "inline-progress")}>Opening card…</div> : null}
      {isLoading ? <div className={ownedClassNames(ownedStyleMaps, "inline-progress")}>Searching cards…</div> : null}

      <section className={ownedClassNames(ownedStyleMaps, "card-library-panel shared-card-browser")} aria-label="Card browser">
        <div className={ownedClassNames(ownedStyleMaps, "card-browser-heading")}>
          <div>
            <h2>{fixedDeckId ? "Cards" : "Results"}</h2>
            <span className={ownedClassNames(ownedStyleMaps, "card-count-summary")}>
              {page.totalCount} {page.totalCount === 1 ? "card" : "cards"}
            </span>
          </div>
          {controlMode === "deck" ? (
            <div className={ownedClassNames(ownedStyleMaps, "card-browser-actions")}>
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "secondary-button")}
                disabled={!selectedDeck}
                title={selectedDeck ? undefined : "This deck is unavailable for bulk card entry"}
                onClick={() => {
                  if (selectedDeck) setBulkEntryOpen(true);
                }}
              >
                Bulk add
              </button>
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "primary-button")}
                disabled={!selectedDeck}
                title={selectedDeck ? undefined : "This deck is unavailable for adding a card"}
                onClick={beginCreateCard}
              >
                Add flashcard
              </button>
            </div>
          ) : null}
        </div>

        {page.totalCount > 0 ? (
          <div className={ownedClassNames(ownedStyleMaps, "card-library-summary")}>
            <div className={ownedClassNames(ownedStyleMaps, "result-count")}>
              <strong>
                {page.cards.length} of {page.totalCount}
              </strong>
              <span> cards shown</span>
            </div>
            <div className={ownedClassNames(ownedStyleMaps, "selection-buttons")} aria-label="Selection controls">
              <button type="button" className={ownedClassNames(ownedStyleMaps, "secondary-button")} onClick={selectVisible}>
                Select visible
              </button>
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "secondary-button")}
                disabled={isSelectingAll}
                onClick={() => void selectAllMatching()}
              >
                {isSelectingAll ? "Selecting…" : `Select all (${page.totalCount})`}
              </button>
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "secondary-button")}
                disabled={selectedCards.size === 0}
                onClick={() => setSelectedCards(new Map())}
              >
                Deselect all
              </button>
            </div>
            <button
              type="button"
              className={ownedClassNames(ownedStyleMaps, "primary-button")}
              disabled={selectedCards.size === 0}
              onClick={() => setPracticeCards(selected)}
            >
              Practice selected
            </button>
          </div>
        ) : null}

        {!isLoading && page.totalCount === 0 ? (
          <section className={ownedClassNames(ownedStyleMaps, "compact-empty-state large-empty-state")}>
            <h2>{fixedDeckId ? "This deck is empty" : "No matching cards"}</h2>
            <p>
              {fixedDeckId
                ? "Add a flashcard or use Bulk add to start studying this deck."
                : "Try removing a filter or changing the search."}
            </p>
          </section>
        ) : null}

        {page.totalCount > 0 ? (
          <div className={ownedClassNames(ownedStyleMaps, "organized-card-list")} role="table" aria-label="Matching cards">
            {page.cards.map((card) => {
              const selectedRow = selectedCards.has(card.cardId);
              return (
                <CardBrowserListRow
                  key={card.cardId}
                  card={card}
                  selected={selectedRow}
                  onToggleSelected={() => toggleSelected(selectionFromRow(card))}
                  onOpen={() => void openCard(card.noteId)}
                />
              );
            })}
          </div>
        ) : null}

        {page.cards.length < page.totalCount ? (
          <button
            type="button"
            className={ownedClassNames(ownedStyleMaps, "secondary-button load-more-cards")}
            disabled={isLoadingMore}
            onClick={() => void loadCards(page.cards.length, true)}
          >
            {isLoadingMore ? "Loading…" : "Load more"}
          </button>
        ) : null}
      </section>

      {renderSelectionModal()}
      {practiceCards ? (
        <PracticeResultsModal
          cards={practiceCards}
          sourceLabel={practiceSourceLabel}
          refreshDecks={refreshDecks}
          onPracticeCards={onPracticeCards}
          onClose={() => setPracticeCards(null)}
          showToast={showToast}
        />
      ) : null}
      {bulkEntryOpen && controlMode === "deck" && selectedDeck ? (
        <BulkEntryModal
          decks={activeDecks}
          fixedDeckId={selectedDeck.id}
          onClose={() => setBulkEntryOpen(false)}
          onSaved={refreshAfterMutation}
          showToast={showToast}
        />
      ) : null}
    </>
  );

  if (!showPageHeader) {
    return <div className={ownedClassNames(ownedStyleMaps, "card-browser-workspace")}>{listContent}</div>;
  }

  return (
    <div className={ownedClassNames(ownedStyleMaps, "screen cards-screen")}>
      <header className={ownedClassNames(ownedStyleMaps, "screen-header compact-screen-header")}>
        <div>
          <h1>Search/Filter</h1>
          <p>
            Find independently scheduled cards and manage exact schedules without changing sibling
            presentations.
          </p>
        </div>
      </header>
      {listContent}
    </div>
  );
}

function CardBrowserListRow({
  card,
  selected,
  onToggleSelected,
  onOpen,
}: {
  card: CardBrowserRow;
  selected: boolean;
  onToggleSelected: () => void;
  onOpen: () => void;
}) {
  const dueLabel = formatDue(card.dueAt);
  return (
    <article
      className={ownedClassNames(ownedStyleMaps, selected ? "organized-card-row selected" : "organized-card-row")}
      role="row"
    >
      <label className={ownedClassNames(ownedStyleMaps, "card-row-selector")} onClick={(event) => event.stopPropagation()}>
        <input
          type="checkbox"
          checked={selected}
          aria-label={`Select ${summarize(card.frontText)}`}
          onChange={onToggleSelected}
        />
      </label>
      <button type="button" className={ownedClassNames(ownedStyleMaps, "organized-card-main")} onClick={onOpen}>
        <span className={ownedClassNames(ownedStyleMaps, "organized-card-deck")}>
          {card.deckName}
          <span className={ownedClassNames(ownedStyleMaps, "card-type-badge")}>{card.cardLabel}</span>
        </span>
        <span className={ownedClassNames(ownedStyleMaps, "organized-card-statuses")}>
          {card.suspendedAt ? <span className={ownedClassNames(ownedStyleMaps, "schedule-status-badge suspended")}>Suspended</span> : null}
          {card.buriedUntil && card.buriedUntil > Date.now() ? (
            <span className={ownedClassNames(ownedStyleMaps, "schedule-status-badge buried")}>Buried</span>
          ) : null}
          {card.needsMorePractice ? (
            <span className={ownedClassNames(ownedStyleMaps, "schedule-status-badge needs-practice")}>Needs more practice</span>
          ) : null}
        </span>
        <span className={ownedClassNames(ownedStyleMaps, "organized-card-sides card-listing-sides")}>
          <span className={ownedClassNames(ownedStyleMaps, "card-listing-line")}>
            <span className={ownedClassNames(ownedStyleMaps, "card-listing-label")} aria-hidden="true">
              Q:
            </span>
            <span className="sr-only">Question: </span>
            <strong>{summarize(card.frontText)}</strong>
          </span>
          <span className={ownedClassNames(ownedStyleMaps, "card-listing-line")}>
            <span className={ownedClassNames(ownedStyleMaps, "card-listing-label")} aria-hidden="true">
              A:
            </span>
            <span className="sr-only">Answer: </span>
            <strong>{summarize(card.backText)}</strong>
          </span>
        </span>
        <span className={ownedClassNames(ownedStyleMaps, "organized-card-tags")}>
          {card.tags.length === 0 ? (
            <small>No tags</small>
          ) : (
            card.tags.map((tag) => (
              <span className={ownedClassNames(ownedStyleMaps, "tag-chip static")} key={tag}>
                {tag}
              </span>
            ))
          )}
        </span>
      </button>
      <div className={ownedClassNames(ownedStyleMaps, "organized-card-meta")}>
        {dueLabel ? <span>{dueLabel}</span> : null}
        <small>{formatLastStudied(card.lastReviewedAt)}</small>
      </div>
    </article>
  );
}

function mutationIdForSchedule(
  pendingRef: MutableRefObject<PendingScheduleMutation | null>,
  key: string,
): string {
  if (pendingRef.current?.key === key) return pendingRef.current.mutationId;
  const mutationId = createMutationId();
  pendingRef.current = { key, mutationId };
  return mutationId;
}

function uniqueNotes(cards: SelectionCard[]): SelectionCard[] {
  const byNoteId = new Map<string, SelectionCard>();
  for (const card of cards) {
    if (!byNoteId.has(card.noteId)) byNoteId.set(card.noteId, card);
  }
  return [...byNoteId.values()];
}

function selectionFromRow(card: CardBrowserRow): SelectionCard {
  return { cardId: card.cardId, noteId: card.noteId, deckId: card.deckId };
}

function summarize(value: string): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (normalized === "") return "Image card";
  return normalized.length > 512 ? `${normalized.slice(0, 509)}…` : normalized;
}

function formatDue(value: number): string | null {
  if (value <= Date.now()) return null;
  return `Scheduled ${new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(
    new Date(value),
  )}`;
}

function formatLastStudied(value: number | null): string {
  if (value === null) return "Never studied";
  const reviewed = new Date(value);
  const today = new Date();
  if (reviewed.toDateString() === today.toDateString()) return "Studied today";
  return `Studied ${new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(
    reviewed,
  )}`;
}
