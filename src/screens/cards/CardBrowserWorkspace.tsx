import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type MutableRefObject } from "react";
import type { ShowToast } from "../../components/Toast";
import { CardsController } from "../../controllers/cardsController";
import { reconcileCommitted } from "../../controllers/reconciliation";
import { isQuestionNote, type AuthoringNote } from "../../ipc/authoringNotes";
import {
  type AvailabilityFilter, type CardBrowserRow, type CardSearchInput,
  type CardSearchPage, type CardSort, type CardStatusFilter, type ReviewedFilter,
  type SelectionCard, type TagSummary,
} from "../../ipc/cards";
import { createMutationId, getErrorCode, getErrorMessage } from "../../ipc/common";
import type { DeckSummary } from "../../ipc/decks";
import type { AnalyticsSearchLaunch } from "../../navigation/flashcardsRoutes";
import type { ScheduleAction } from "../../ipc/scheduling";
import type { CardTemplate, GenerateTemplateCardsResult } from "../../ipc/templates";
import { IconButton } from "../../ui/icons/IconButton";
import { SelectionToolbar } from "../../ui/patterns/SelectionToolbar";
import { InlineNotice, InlineProgress } from "../../ui/primitives/InlineNotice";
import { EmptyState } from "../../ui/primitives/EmptyState";
import { Badge } from "../../ui/primitives/Badge";
import { Chip } from "../../ui/primitives/Chip";
import { Checkbox } from "../../ui/primitives/FormControls";
import { TextField } from "../../ui/primitives/TextField";
import { Button } from "../../ui/primitives/Button";
import { Card } from "../../ui/primitives/Card";
import { cn } from "../../ui/internal/cn";
import { CardEditorScreen } from "../CardEditorScreen";
import { BulkEntryModal } from "./BulkEntryModal";
import { BulkTagModal } from "./BulkTagModal";
import { CardFilters } from "./CardFilters";
import { DeleteCardsModal } from "./DeleteCardsModal";
import { MoveCopyModal, type TransferableCard } from "./MoveCopyModal";
import { PracticeResultsModal } from "./PracticeResultsModal";
import { ScheduleActionModal } from "./ScheduleActionModal";
import { scheduleActionNotice } from "./scheduleActionResult";
import styles from "./browserPresentation";

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
  analyticsSearchLaunch?: AnalyticsSearchLaunch;
}
type SelectionAction = "move_copy" | "add_tags" | "remove_tags" | "delete" | "suspend" | "resume" | "bury" | "unbury" | "reschedule" | "reset";
interface PendingScheduleMutation { key: string; mutationId: string; }
const PAGE_SIZE = 100;

/**
 * Deck detail and global search share native queries, pagination and exact-card
 * selection. Presentation varies only the controls above that shared model.
 * UI-library components do not own requests, mutation identities or editor state.
 */
export function CardBrowserWorkspace({
  decks, refreshDecks, onPracticeCards, onDirtyChange, showToast, fixedDeckId,
  controls, showFilters, showPageHeader = fixedDeckId === undefined, onViewChange,
  analyticsSearchLaunch,
}: CardBrowserWorkspaceProps) {
  const controlMode: CardBrowserControls = controls ??
    (showFilters === undefined ? (fixedDeckId === undefined ? "search" : "deck") : showFilters ? "search" : "deck");
  const activeDecks = useMemo(() => decks.filter((deck) => deck.archivedAt === null), [decks]);
  const [filterDeckIds, setFilterDeckIds] = useState<Set<string>>(new Set());
  const [query, setQuery] = useState("");
  const [deckSearchOpen, setDeckSearchOpen] = useState(false);
  const [availabilityFilter, setAvailabilityFilter] = useState<AvailabilityFilter>("all");
  const [statusFilter, setStatusFilter] = useState<CardStatusFilter>(() => fixedDeckId ? "all" : "active");
  const [reviewedFilter, setReviewedFilter] = useState<ReviewedFilter>("all");
  const [sort, setSort] = useState<CardSort>("updated_desc");
  const [tagIds, setTagIds] = useState<Set<string>>(() => new Set(analyticsSearchLaunch?.tagIds ?? []));
  const [searchGate, setSearchGate] = useState<"open" | "pending" | "blocked">(analyticsSearchLaunch ? "pending" : "open");
  const [tags, setTags] = useState<TagSummary[]>([]);
  const [page, setPage] = useState<CardSearchPage>({ cards: [], totalCount: 0 });
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [isSelectingAll, setIsSelectingAll] = useState(false);
  const [isApplyingSchedule, setIsApplyingSchedule] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [selectedCards, setSelectedCards] = useState<Map<string, SelectionCard>>(new Map());
  const [editingCard, setEditingCard] = useState<AuthoringNote | null>(null);
  const [creatingDeckId, setCreatingDeckId] = useState<string | null>(null);
  const [isOpeningCard, setIsOpeningCard] = useState(false);
  const [bulkEntryOpen, setBulkEntryOpen] = useState(false);
  const [selectionAction, setSelectionAction] = useState<SelectionAction | null>(null);
  const [moveCopyCards, setMoveCopyCards] = useState<TransferableCard[]>([]);
  const [practiceCards, setPracticeCards] = useState<SelectionCard[] | null>(null);
  const pendingScheduleMutationRef = useRef<PendingScheduleMutation | null>(null);
  const requestedTagClearedRef = useRef(false);
  const controllerRef = useRef<CardsController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new CardsController();
  const controller = controllerRef.current;

  const scopedDeckIds = useMemo(() => fixedDeckId ? [fixedDeckId] : [...filterDeckIds], [filterDeckIds, fixedDeckId]);
  const deckIds = useMemo(() => new Set(scopedDeckIds), [scopedDeckIds]);
  const normalizedSort: CardSort = query.trim() === "" && sort === "relevance" ? "updated_desc" : sort;
  const searchInput = useMemo<CardSearchInput>(() => ({
    deckIds: scopedDeckIds, query, tagIds: [...tagIds], availabilityFilter,
    statusFilter, reviewedFilter, sort: normalizedSort, limit: PAGE_SIZE, offset: 0,
  }), [availabilityFilter, normalizedSort, query, reviewedFilter, scopedDeckIds, statusFilter, tagIds]);
  const selected = useMemo(() => [...selectedCards.values()], [selectedCards]);
  const selectedNotes = useMemo(() => uniqueNotes(selected), [selected]);
  const selectedDeck = scopedDeckIds.length === 1
    ? (activeDecks.find((deck) => deck.id === scopedDeckIds[0]) ?? null) : null;
  const browserView: CardBrowserView = editingCard || creatingDeckId ? "editor" : "list";
  const practiceSourceLabel = fixedDeckId ? `the “${selectedDeck?.name ?? "current"}” deck` : "Search/Filter";
  const deckQueryActive = controlMode === "deck" && query.trim() !== "";
  const deckSearchExpanded = controlMode === "deck" && (deckSearchOpen || deckQueryActive);

  const loadTags = useCallback(async (): Promise<void> => {
    try {
      const loaded = await controller.tags(scopedDeckIds);
      if (!loaded) return;
      setTags(loaded);
      setTagIds((current) => {
        const valid = new Set(loaded.map((tag) => tag.id));
        return new Set([...current].filter((id) => valid.has(id)));
      });
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
  }, [controller, scopedDeckIds]);

  const loadCards = useCallback(async (offset = 0, append = false): Promise<void> => {
    if (searchGate !== "open") return;
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
      setPage((current) => ({ totalCount: result.totalCount, cards: append ? [...current.cards, ...result.cards] : result.cards }));
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { setIsLoading(false); setIsLoadingMore(false); }
  }, [controller, searchGate, searchInput]);

  useEffect(() => () => controller.invalidate(), [controller]);
  useEffect(() => { onViewChange?.(browserView); }, [browserView, onViewChange]);
  useEffect(() => {
    if (fixedDeckId) return;
    setFilterDeckIds((current) => {
      const activeIds = new Set(activeDecks.map((deck) => deck.id));
      return new Set([...current].filter((id) => activeIds.has(id)));
    });
  }, [activeDecks, fixedDeckId]);
  useEffect(() => {
    if (!analyticsSearchLaunch) return;
    let active = true;
    void controller.tags([]).then((loaded) => {
      if (!active || !loaded || requestedTagClearedRef.current) return;
      setTags(loaded);
      const availableIds = new Set(loaded.map((tag) => tag.id));
      if (analyticsSearchLaunch.tagIds.every((tagId) => availableIds.has(tagId))) {
        setSearchGate("open");
        return;
      }
      setIsLoading(false);
      setSearchGate("blocked");
      setErrorMessage("The tag chosen in Progress is no longer available. Clear the unavailable tag filter to search all cards.");
    }).catch((error: unknown) => {
      if (!active || requestedTagClearedRef.current) return;
      setIsLoading(false);
      setSearchGate("blocked");
      setErrorMessage(`The tag chosen in Progress could not be verified: ${getErrorMessage(error)} Clear the unavailable tag filter to search all cards.`);
    });
    return () => { active = false; };
  }, [analyticsSearchLaunch, controller]);
  useEffect(() => {
    if (searchGate !== "open") return;
    if (analyticsSearchLaunch && !requestedTagClearedRef.current) return;
    void loadTags();
  }, [analyticsSearchLaunch, loadTags, searchGate]);
  useEffect(() => { if (query.trim() === "" && sort === "relevance") setSort("updated_desc"); }, [query, sort]);
  useEffect(() => {
    if (statusFilter === "suspended" || statusFilter === "buried") setAvailabilityFilter("all");
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
  function openDeckSearch(): void { setDeckSearchOpen(true); }
  function closeDeckSearch(): void { changeQuery(""); setDeckSearchOpen(false); }
  function handleDeckSearchKeyDown(event: KeyboardEvent<HTMLInputElement>): void {
    if (event.key !== "Escape") return;
    event.preventDefault();
    closeDeckSearch();
  }
  function changeTagIds(next: Set<string>): void {
    if (analyticsSearchLaunch && next.size === 0) {
      requestedTagClearedRef.current = true;
      setSearchGate("open");
    }
    setTagIds(next);
  }
  async function refreshAfterMutation(): Promise<void> {
    await Promise.all([loadCards(), loadTags(), refreshDecks()]);
    setSelectedCards(new Map());
  }
  async function reconcileAfterCommit(subject: string): Promise<void> {
    const outcome = await reconcileCommitted(undefined, refreshAfterMutation);
    if (outcome.reconciliationError) showToast("info", `${subject} was saved, but the current card list could not refresh: ${outcome.reconciliationError}`);
  }
  async function completeSelectionMutation(): Promise<void> {
    if (editingCard || creatingDeckId) {
      onDirtyChange(false);
      setEditingCard(null);
      setCreatingDeckId(null);
    }
    await refreshAfterMutation();
  }
  async function openCard(noteId: string, cardType: string): Promise<void> {
    setIsOpeningCard(true);
    setErrorMessage(null);
    try {
      const card = await controller.note(noteId, cardType);
      if (!card) return;
      setEditingCard(card);
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { setIsOpeningCard(false); }
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
    if (searchGate !== "open" || page.totalCount === 0 || isSelectingAll) return;
    setIsSelectingAll(true);
    setErrorMessage(null);
    try {
      const matches = await controller.matching(searchInput);
      if (!matches) return;
      setSelectedCards(new Map(matches.map((card) => [card.cardId, card])));
    } catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { setIsSelectingAll(false); }
  }
  function beginMoveCopy(cards: TransferableCard[]): void {
    const notes = [...new Map(cards.map((card) => [card.noteId, card])).values()];
    if (notes.length === 0) return;
    setMoveCopyCards(notes);
    setSelectionAction("move_copy");
  }
  async function applySimpleScheduleAction(cards: SelectionCard[], action: Exclude<ScheduleAction, "reset" | "reschedule">): Promise<void> {
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
        setErrorMessage(`${getErrorMessage(error)} Choose the same schedule action again to safely retry the complete selection.`);
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
      void applySimpleScheduleAction(selected, action as "suspend" | "resume" | "bury" | "unbury");
    } else setSelectionAction(action);
  }
  function renderSelectionModal() {
    if (selectionAction === "move_copy") return (
      <MoveCopyModal cards={moveCopyCards} decks={activeDecks}
        onClose={() => { setSelectionAction(null); setMoveCopyCards([]); }}
        onComplete={completeSelectionMutation} showToast={showToast} />
    );
    if (selectionAction === "add_tags" || selectionAction === "remove_tags") return (
      <BulkTagModal cards={selectedNotes} mode={selectionAction === "add_tags" ? "add" : "remove"}
        suggestions={tags} onClose={() => setSelectionAction(null)} onComplete={completeSelectionMutation} showToast={showToast} />
    );
    if (selectionAction === "delete") return (
      <DeleteCardsModal cards={selectedNotes} onClose={() => setSelectionAction(null)} onComplete={completeSelectionMutation} showToast={showToast} />
    );
    if (selectionAction === "reset" || selectionAction === "reschedule") return (
      <ScheduleActionModal cards={selected} action={selectionAction} onClose={() => setSelectionAction(null)} onComplete={refreshAfterMutation} showToast={showToast} />
    );
    return null;
  }

  if (browserView === "editor") {
    const editorDeckId = editingCard?.deckId ?? creatingDeckId;
    const deck = activeDecks.find((candidate) => candidate.id === editorDeckId);
    if (!deck) return (
      <div className={styles.screen}>
        <InlineNotice kind="error">The card’s active deck could not be found. Return to the card list and refresh.</InlineNotice>
        <Button variant="secondary" onClick={() => { setEditingCard(null); setCreatingDeckId(null); }}>Back to cards</Button>
      </div>
    );
    return (
      <>
        <CardEditorScreen deck={deck} decks={activeDecks} card={editingCard} onDirtyChange={onDirtyChange}
          onCancel={() => { onDirtyChange(false); setEditingCard(null); setCreatingDeckId(null); }}
          onMoveCopy={editingCard && !isQuestionNote(editingCard)
            ? () => beginMoveCopy([{ noteId: editingCard.noteId, deckId: editingCard.deckId }])
            : undefined}
          onSaved={async (saved, continueAdding) => {
            onDirtyChange(false);
            const wasEditing = editingCard !== null;
            if (!continueAdding) { setEditingCard(null); setCreatingDeckId(null); }
            showToast("success", wasEditing ? "Flashcard updated." : "Flashcard saved.");
            if (saved.deckId !== deck.id) showToast("info", "The card is now in a different deck.");
            await reconcileAfterCommit("The flashcard");
          }}
          onQuestionSaved={async (_noteId, continueAdding) => {
            onDirtyChange(false);
            const wasEditing = editingCard !== null;
            if (!continueAdding) { setEditingCard(null); setCreatingDeckId(null); }
            showToast("success", wasEditing ? "Interactive question updated." : "Interactive question saved.");
            await reconcileAfterCommit("The interactive question");
          }}
          onTemplateGenerated={async (result: GenerateTemplateCardsResult, continueAdding: boolean, template: CardTemplate) => {
            onDirtyChange(false);
            if (!continueAdding) { setEditingCard(null); setCreatingDeckId(null); }
            const duplicateMessage = result.skippedDuplicates > 0
              ? ` ${result.skippedDuplicates} exact ${result.skippedDuplicates === 1 ? "duplicate was" : "duplicates were"} skipped.` : "";
            showToast(result.createdCount > 0 ? "success" : "info",
              `Generated ${result.createdCount} ${result.createdCount === 1 ? "card" : "cards"} from “${template.name}”.${duplicateMessage}`);
            await reconcileAfterCommit("The generated cards");
          }} />
        {renderSelectionModal()}
      </>
    );
  }

  const listContent = (
    <>
      {controlMode === "search" ? (
        <CardFilters activeDecks={activeDecks} deckIds={deckIds} query={query}
          availabilityFilter={availabilityFilter} statusFilter={statusFilter} reviewedFilter={reviewedFilter}
          sort={sort} tags={tags} tagIds={tagIds} onDeckIdsChange={setFilterDeckIds} onQueryChange={changeQuery}
          onAvailabilityFilterChange={setAvailabilityFilter} onStatusFilterChange={setStatusFilter}
          onReviewedFilterChange={setReviewedFilter} onSortChange={setSort} onTagIdsChange={changeTagIds} />
      ) : null}
      {selectedCards.size > 0 ? (
        <SelectionToolbar selectedCount={selectedCards.size}
          primaryAction={{ id: "practice-selected", label: "Practice selected", onSelect: () => setPracticeCards(selected) }}
          secondaryActions={([
            ["move_copy", "Move / copy cards"], ["add_tags", "Add tags"], ["remove_tags", "Remove tags"],
            ["suspend", "Suspend cards"], ["resume", "Resume cards"], ["bury", "Bury until tomorrow"],
            ["unbury", "Unbury cards"], ["reschedule", "Reschedule cards"], ["reset", "Reset schedules"], ["delete", "Delete cards"],
          ] as const).map(([id, label]) => ({ id, label, placement: "overflow-only" as const, disabled: isApplyingSchedule,
            destructive: id === "delete" || id === "reset", onSelect: () => chooseSelectionAction(id as SelectionAction) }))}
          clearLabel="Deselect all" onClearSelection={() => setSelectedCards(new Map())} />
      ) : null}
      {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}
      {searchGate === "blocked" ? <Button variant="secondary" onClick={() => changeTagIds(new Set())}>Clear unavailable tag filter</Button> : null}
      {isOpeningCard ? <InlineProgress>Opening card…</InlineProgress> : null}
      {isLoading ? <InlineProgress>Searching cards…</InlineProgress> : null}

      <Card as="section" className={styles.cardLibraryPanel} aria-label="Card browser">
        <div className={styles.cardBrowserHeading}>
          <div><h2>{fixedDeckId ? "Cards" : "Results"}</h2>
            {!fixedDeckId ? <span className={styles.cardCountSummary}>{page.totalCount} {page.totalCount === 1 ? "card" : "cards"}</span> : null}
          </div>
          {controlMode === "deck" ? (
            <div className={styles.cardBrowserActions}>
              <Button variant="secondary" disabled={!selectedDeck}
                title={selectedDeck ? undefined : "This deck is unavailable for bulk card entry"}
                onClick={() => { if (selectedDeck) setBulkEntryOpen(true); }}>Bulk add</Button>
              <Button variant="primary" disabled={!selectedDeck}
                title={selectedDeck ? undefined : "This deck is unavailable for adding a card"} onClick={beginCreateCard}>Add flashcard</Button>
            </div>
          ) : null}
        </div>
        {page.totalCount > 0 || controlMode === "deck" ? (
          <div className={styles.cardLibrarySummary}>
            <div className={styles.resultCount}><strong>{page.cards.length} of {page.totalCount}</strong>
              <span>{deckQueryActive ? " matching cards shown" : " cards shown"}</span>
            </div>
            <div className={styles.selectionButtons} role="group" aria-label="Card table controls">
              {page.totalCount > 0 ? (
                <>
                  <Button variant="secondary" onClick={selectVisible}>Select visible</Button>
                  <Button variant="secondary" disabled={isSelectingAll} onClick={() => void selectAllMatching()}>
                    {isSelectingAll ? "Selecting…" : `Select all (${page.totalCount})`}
                  </Button>
                </>
              ) : null}
              {controlMode === "deck" ? (
                <div className={styles.deckCardSearch} aria-label="Search this deck" data-expanded={deckSearchExpanded || undefined}>
                  {deckSearchExpanded ? (
                    <>
                      <label><span className="sr-only">Search cards in this deck</span>
                        <TextField autoFocus type="search" value={query} placeholder="Question, answer, or tag"
                          aria-label="Search cards in this deck" onChange={(event) => changeQuery(event.currentTarget.value)} onKeyDown={handleDeckSearchKeyDown} />
                      </label>
                      <IconButton icon="close" label="Close deck search" tooltip={false} onClick={closeDeckSearch} />
                    </>
                  ) : <IconButton icon="search" label="Search cards in this deck" tooltip="Search cards" onClick={openDeckSearch} />}
                </div>
              ) : null}
            </div>
          </div>
        ) : null}
        {!isLoading && page.totalCount === 0 ? (
          <EmptyState compact title={fixedDeckId && !deckQueryActive ? "This deck is empty" : "No matching cards"}
            description={fixedDeckId && !deckQueryActive ? "Add a flashcard or use Bulk add to start studying this deck." : "Try changing or clearing the search."} />
        ) : null}
        {page.totalCount > 0 ? (
          <div className={styles.organizedCardList} role="table" aria-label="Matching cards">
            {page.cards.map((card) => <CardBrowserListRow key={card.cardId} card={card}
              selected={selectedCards.has(card.cardId)} onToggleSelected={() => toggleSelected(selectionFromRow(card))}
              onOpen={() => void openCard(card.noteId, card.cardType)} />)}
          </div>
        ) : null}
        {page.cards.length < page.totalCount ? (
          <Button variant="secondary" className={styles.loadMoreCards} disabled={isLoadingMore} onClick={() => void loadCards(page.cards.length, true)}>
            {isLoadingMore ? "Loading…" : "Load more"}
          </Button>
        ) : null}
      </Card>
      {renderSelectionModal()}
      {practiceCards ? <PracticeResultsModal cards={practiceCards} sourceLabel={practiceSourceLabel}
        refreshDecks={refreshDecks} onPracticeCards={onPracticeCards} onClose={() => setPracticeCards(null)} showToast={showToast} /> : null}
      {bulkEntryOpen && controlMode === "deck" && selectedDeck ? (
        <BulkEntryModal decks={activeDecks} fixedDeckId={selectedDeck.id} onClose={() => setBulkEntryOpen(false)}
          onSaved={refreshAfterMutation} showToast={showToast} />
      ) : null}
    </>
  );
  if (!showPageHeader) return <div className={styles.cardBrowserWorkspace}>{listContent}</div>;
  return <div className={styles.screen}>
    <header className={styles.screenHeader}><div><h1>Search/Filter</h1>
      <p>Find independently scheduled cards and manage exact schedules without changing sibling presentations.</p>
    </div></header>
    {listContent}
  </div>;
}

function CardBrowserListRow({ card, selected, onToggleSelected, onOpen }: {
  card: CardBrowserRow; selected: boolean; onToggleSelected: () => void; onOpen: () => void;
}) {
  const dueLabel = formatDue(card.dueAt);
  return (
    <article className={cn(styles.organizedCardRow, selected && "tw:bg-accent/60")} role="row">
      <label className={styles.cardRowSelector} onClick={(event) => event.stopPropagation()}>
        <Checkbox checked={selected} aria-label={`Select ${summarize(card.frontText)}`} onCheckedChange={onToggleSelected} />
      </label>
      <Button variant="secondary" className={styles.organizedCardMain} onClick={onOpen}>
        <span className={styles.organizedCardDeck}>{card.deckName}<Badge tone="neutral">{card.cardLabel}</Badge></span>
        <span className={styles.organizedCardStatuses}>
          {card.suspendedAt ? <Badge tone="danger">Suspended</Badge> : null}
          {card.buriedUntil && card.buriedUntil > Date.now() ? <Badge tone="accent">Buried</Badge> : null}
          {card.needsMorePractice ? <Badge tone="warning">Needs more practice</Badge> : null}
        </span>
        <span className={styles.organizedCardSides}>
          <span className={styles.cardListingLine}><span className={styles.cardListingLabel} aria-hidden="true">Q:</span>
            <span className="sr-only">Question: </span><strong>{summarize(card.frontText)}</strong></span>
          <span className={styles.cardListingLine}><span className={styles.cardListingLabel} aria-hidden="true">A:</span>
            <span className="sr-only">Answer: </span><strong>{summarize(card.backText)}</strong></span>
        </span>
        <span className={styles.organizedCardTags}>
          {card.tags.length === 0 ? <small>No tags</small> : card.tags.map((tag) => <Chip key={tag}>{tag}</Chip>)}
        </span>
      </Button>
      <div className={styles.organizedCardMeta}>{dueLabel ? <span>{dueLabel}</span> : null}<small>{formatLastStudied(card.lastReviewedAt)}</small></div>
    </article>
  );
}
function mutationIdForSchedule(pendingRef: MutableRefObject<PendingScheduleMutation | null>, key: string): string {
  if (pendingRef.current?.key === key) return pendingRef.current.mutationId;
  const mutationId = createMutationId();
  pendingRef.current = { key, mutationId };
  return mutationId;
}
function uniqueNotes(cards: SelectionCard[]): SelectionCard[] {
  const byNoteId = new Map<string, SelectionCard>();
  for (const card of cards) if (!byNoteId.has(card.noteId)) byNoteId.set(card.noteId, card);
  return [...byNoteId.values()];
}
function selectionFromRow(card: CardBrowserRow): SelectionCard { return { cardId: card.cardId, noteId: card.noteId, deckId: card.deckId }; }
function summarize(value: string): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (normalized === "") return "Image card";
  return normalized.length > 512 ? `${normalized.slice(0, 509)}…` : normalized;
}
function formatDue(value: number): string | null {
  if (value <= Date.now()) return null;
  return `Scheduled ${new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(value))}`;
}
function formatLastStudied(value: number | null): string {
  if (value === null) return "Never studied";
  const reviewed = new Date(value);
  const today = new Date();
  if (reviewed.toDateString() === today.toDateString()) return "Studied today";
  return `Studied ${new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(reviewed)}`;
}
