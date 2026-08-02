import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ChangeEvent } from "react";

import {
  applyScheduleAction,
  createMutationId,
  getErrorMessage,
  getOrganizedCard,
  listMatchingCards,
  listTags,
  searchCards,
  type CardBrowserRow,
  type CardSearchInput,
  type CardSearchPage,
  type CardSort,
  type CardStatusFilter,
  type CardTemplate,
  type Deck,
  type DueFilter,
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
  const [dueFilter, setDueFilter] = useState<DueFilter>("all");
  const [statusFilter, setStatusFilter] = useState<CardStatusFilter>("active");
  const [reviewedFilter, setReviewedFilter] = useState<ReviewedFilter>("all");
  const [sort, setSort] = useState<CardSort>("updated_desc");
  const [tagIds, setTagIds] = useState<Set<string>>(new Set());
  const [tags, setTags] = useState<TagSummary[]>([]);
  const [page, setPage] = useState<CardSearchPage>({ cards: [], totalCount: 0 });
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [isSelectingAll, setIsSelectingAll] = useState(false);
  const [isResolvingPractice, setIsResolvingPractice] = useState(false);
  const [isApplyingSchedule, setIsApplyingSchedule] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [selectedCards, setSelectedCards] = useState<Map<string, SelectionCard>>(new Map());
  const [editingCard, setEditingCard] = useState<OrganizedCard | null>(null);
  const [creatingDeckId, setCreatingDeckId] = useState<string | null>(null);
  const [isOpeningCard, setIsOpeningCard] = useState(false);
  const [bulkOpen, setBulkOpen] = useState(false);
  const [selectionAction, setSelectionAction] = useState<SelectionAction | null>(null);
  const [moveCopyCards, setMoveCopyCards] = useState<SelectionCard[]>([]);
  const [practiceCards, setPracticeCards] = useState<SelectionCard[] | null>(null);
  const [practiceSourceLabel, setPracticeSourceLabel] = useState("the current results");
  const requestSequence = useRef(0);

  const normalizedSort: CardSort = query.trim() === "" && sort === "relevance" ? "updated_desc" : sort;
  const searchInput = useMemo<CardSearchInput>(
    () => ({
      deckIds: [...deckIds],
      query,
      tagIds: [...tagIds],
      dueFilter,
      statusFilter,
      reviewedFilter,
      sort: normalizedSort,
      limit: PAGE_SIZE,
      offset: 0,
    }),
    [deckIds, dueFilter, normalizedSort, query, reviewedFilter, statusFilter, tagIds],
  );
  const selected = useMemo(() => [...selectedCards.values()], [selectedCards]);
  const selectedNotes = useMemo(() => uniqueNotes(selected), [selected]);
  const selectedDeck = deckIds.size === 1 ? decks.find((deck) => deck.id === [...deckIds][0]) ?? null : null;

  const loadTags = useCallback(async (): Promise<void> => {
    try {
      const loaded = await listTags([...deckIds]);
      setTags(loaded);
      setTagIds((current) => {
        const valid = new Set(loaded.map((tag) => tag.id));
        return new Set([...current].filter((id) => valid.has(id)));
      });
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    }
  }, [deckIds]);

  const loadCards = useCallback(async (offset = 0, append = false): Promise<void> => {
    const requestId = requestSequence.current + 1;
    requestSequence.current = requestId;
    if (append) setIsLoadingMore(true);
    else {
      setIsLoading(true);
      setPage({ cards: [], totalCount: 0 });
    }
    setErrorMessage(null);
    try {
      const result = await searchCards({ ...searchInput, offset });
      if (requestSequence.current !== requestId) return;
      setPage((current) => ({
        totalCount: result.totalCount,
        cards: append ? [...current.cards, ...result.cards] : result.cards,
      }));
    } catch (error: unknown) {
      if (requestSequence.current === requestId) setErrorMessage(getErrorMessage(error));
    } finally {
      if (requestSequence.current === requestId) {
        setIsLoading(false);
        setIsLoadingMore(false);
      }
    }
  }, [searchInput]);

  useEffect(() => { void loadTags(); }, [loadTags]);
  useEffect(() => { if (query.trim() === "" && sort === "relevance") setSort("updated_desc"); }, [query, sort]);
  useEffect(() => {
    setSelectedCards(new Map());
    const timeout = window.setTimeout(() => void loadCards(), 180);
    return () => window.clearTimeout(timeout);
  }, [loadCards]);

  async function refreshAfterMutation(): Promise<void> {
    await Promise.all([loadCards(), loadTags(), refreshDecks()]);
    setSelectedCards(new Map());
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
    if (isOpeningCard) return;
    setIsOpeningCard(true);
    setErrorMessage(null);
    try { setEditingCard(await getOrganizedCard(noteId)); }
    catch (error: unknown) { setErrorMessage(getErrorMessage(error)); }
    finally { setIsOpeningCard(false); }
  }

  function toggleSelected(card: SelectionCard): void {
    setSelectedCards((current) => {
      const next = new Map(current);
      if (next.has(card.cardId)) next.delete(card.cardId); else next.set(card.cardId, card);
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
      const matches = await listMatchingCards(searchInput);
      setSelectedCards(new Map(matches.map((card) => [card.cardId, card])));
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsSelectingAll(false);
    }
  }

  function beginMoveCopy(cards: SelectionCard[]): void {
    const notes = uniqueNotes(cards);
    if (notes.length === 0) return;
    setMoveCopyCards(notes);
    setSelectionAction("move_copy");
  }

  async function practiceAllResults(): Promise<void> {
    if (page.totalCount === 0 || isResolvingPractice) return;
    setIsResolvingPractice(true);
    setErrorMessage(null);
    try {
      const matches = await listMatchingCards(searchInput);
      setPracticeSourceLabel("the current filtered results");
      setPracticeCards(matches);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsResolvingPractice(false);
    }
  }

  async function applySimpleScheduleAction(cards: SelectionCard[], action: Exclude<ScheduleAction, "reset" | "reschedule">): Promise<void> {
    if (cards.length === 0 || isApplyingSchedule) return;
    setIsApplyingSchedule(true);
    setErrorMessage(null);
    try {
      for (const card of cards) {
        await applyScheduleAction({ mutationId: createMutationId(), cardIds: [card.cardId], action });
      }
      await refreshAfterMutation();
      const labels: Record<typeof action, string> = {
        suspend: "Suspended",
        resume: "Resumed",
        bury: "Buried until tomorrow",
        unbury: "Unburied",
      };
      showToast("success", `${labels[action]} ${cards.length} ${cards.length === 1 ? "card" : "cards"}.`);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsApplyingSchedule(false);
    }
  }

  function chooseSelectionAction(action: SelectionAction): void {
    if (action === "move_copy") beginMoveCopy(selected);
    else if (["suspend", "resume", "bury", "unbury"].includes(action)) {
      void applySimpleScheduleAction(selected, action as "suspend" | "resume" | "bury" | "unbury");
    } else setSelectionAction(action);
  }

  function renderSelectionModal() {
    if (selectionAction === "move_copy") {
      return <MoveCopyModal cards={moveCopyCards} decks={decks} onClose={() => { setSelectionAction(null); setMoveCopyCards([]); }} onComplete={completeSelectionMutation} showToast={showToast} />;
    }
    if (selectionAction === "add_tags" || selectionAction === "remove_tags") {
      return <BulkTagModal cards={selectedNotes} mode={selectionAction === "add_tags" ? "add" : "remove"} suggestions={tags} onClose={() => setSelectionAction(null)} onComplete={completeSelectionMutation} showToast={showToast} />;
    }
    if (selectionAction === "delete") {
      return <DeleteCardsModal cards={selectedNotes} onClose={() => setSelectionAction(null)} onComplete={completeSelectionMutation} showToast={showToast} />;
    }
    if (selectionAction === "reset" || selectionAction === "reschedule") {
      return <ScheduleActionModal cards={selected} action={selectionAction} onClose={() => setSelectionAction(null)} onComplete={refreshAfterMutation} showToast={showToast} />;
    }
    return null;
  }

  if (editingCard || creatingDeckId) {
    const editorDeckId = editingCard?.deckId ?? creatingDeckId;
    const deck = decks.find((candidate) => candidate.id === editorDeckId);
    if (!deck) {
      return <div className="screen cards-screen"><div className="inline-alert error-alert">The card’s deck could not be found. Return to Search/Filter and refresh.</div><button type="button" className="secondary-button" onClick={() => { setEditingCard(null); setCreatingDeckId(null); }}>Back to Search/Filter</button></div>;
    }
    return (
      <>
        <CardEditorScreen
          deck={deck}
          decks={decks}
          card={editingCard}
          onDirtyChange={onDirtyChange}
          onCancel={() => { onDirtyChange(false); setEditingCard(null); setCreatingDeckId(null); }}
          onMoveCopy={editingCard ? () => beginMoveCopy([{ cardId: editingCard.id, noteId: editingCard.noteId, deckId: editingCard.deckId }]) : undefined}
          onSaved={async (saved, continueAdding) => {
            onDirtyChange(false);
            const wasEditing = editingCard !== null;
            if (!continueAdding) { setEditingCard(null); setCreatingDeckId(null); }
            await refreshAfterMutation();
            showToast("success", wasEditing ? "Flashcard updated." : "Flashcard saved.");
            if (saved.deckId !== deck.id) showToast("info", "The card is now in a different deck.");
          }}
          onTemplateGenerated={async (result: GenerateTemplateCardsResult, continueAdding: boolean, template: CardTemplate) => {
            onDirtyChange(false);
            if (!continueAdding) { setEditingCard(null); setCreatingDeckId(null); }
            await refreshAfterMutation();
            const duplicateMessage = result.skippedDuplicates > 0 ? ` ${result.skippedDuplicates} exact ${result.skippedDuplicates === 1 ? "duplicate was" : "duplicates were"} skipped.` : "";
            showToast(result.createdCount > 0 ? "success" : "info", `Generated ${result.createdCount} ${result.createdCount === 1 ? "card" : "cards"} from “${template.name}”.${duplicateMessage}`);
          }}
        />
        {renderSelectionModal()}
      </>
    );
  }

  return (
    <div className="screen cards-screen">
      <header className="screen-header compact-screen-header">
        <div><h1>Search/Filter</h1><p>Find independently scheduled cards and manage exact card schedules without changing sibling presentations.</p></div>
        <div className="header-actions wrap-actions">
          <button type="button" className="secondary-button" disabled={decks.length === 0} onClick={() => setBulkOpen(true)}>Bulk add</button>
          <button type="button" className="primary-button" disabled={!selectedDeck} title={selectedDeck ? undefined : "Select exactly one deck before adding an individual card"} onClick={() => { if (!selectedDeck) return; setEditingCard(null); setCreatingDeckId(selectedDeck.id); }}>Add card</button>
        </div>
      </header>

      <section className="card-library-toolbar" aria-label="Card filters">
        <details className="filter-menu deck-filter-menu"><summary>{deckFilterSummary(deckIds, decks)}</summary><div className="filter-checklist">
          <label><input type="checkbox" checked={deckIds.size === 0} onChange={() => setDeckIds(new Set())} /><span>All decks</span><small>{decks.reduce((sum, deck) => sum + deck.cardCount, 0)}</small></label>
          {decks.map((deck) => <label key={deck.id}><input type="checkbox" checked={deckIds.has(deck.id)} onChange={(event) => { const next = new Set(deckIds); if (event.currentTarget.checked) next.add(deck.id); else next.delete(deck.id); setDeckIds(next); }} /><span>{deck.name}</span><small>{deck.cardCount}</small></label>)}
        </div></details>
        <label className="card-library-search"><span>Search cards</span><input type="search" value={query} placeholder="Question, answer, or tag" onChange={(event) => { const value = event.currentTarget.value; setQuery(value); if (value.trim() !== "" && sort === "updated_desc") setSort("relevance"); }} /></label>
        <label><span>Due</span><select value={dueFilter} onChange={(event: ChangeEvent<HTMLSelectElement>) => setDueFilter(event.currentTarget.value as DueFilter)}><option value="all">Any due date</option><option value="due">Due now</option><option value="not_due">Not due</option></select></label>
        <label><span>Status</span><select value={statusFilter} onChange={(event: ChangeEvent<HTMLSelectElement>) => setStatusFilter(event.currentTarget.value as CardStatusFilter)}><option value="active">Active</option><option value="all">Any status</option><option value="suspended">Suspended</option><option value="buried">Buried</option><option value="needs_more_practice">Needs more practice</option></select></label>
        <label><span>Last studied</span><select value={reviewedFilter} onChange={(event: ChangeEvent<HTMLSelectElement>) => setReviewedFilter(event.currentTarget.value as ReviewedFilter)}><option value="all">Any time</option><option value="never">Never studied</option><option value="last_1_day">Within 24 hours</option><option value="last_7_days">Within 7 days</option><option value="last_30_days">Within 30 days</option><option value="older_30_days">More than 30 days ago</option></select></label>
        <label><span>Sort</span><select value={sort} onChange={(event: ChangeEvent<HTMLSelectElement>) => setSort(event.currentTarget.value as CardSort)}><option value="relevance" disabled={query.trim() === ""}>Relevance</option><option value="updated_desc">Recently updated</option><option value="last_reviewed_desc">Recently studied</option><option value="question_asc">Question A–Z</option><option value="due_asc">Next due</option></select></label>
        <details className="filter-menu tag-filter-menu"><summary>Tags{tagIds.size > 0 ? ` (${tagIds.size})` : ""}</summary><div className="filter-checklist">{tags.length === 0 ? <p>No tags in the selected decks.</p> : tags.map((tag) => <label key={tag.id}><input type="checkbox" checked={tagIds.has(tag.id)} onChange={(event) => { const next = new Set(tagIds); if (event.currentTarget.checked) next.add(tag.id); else next.delete(tag.id); setTagIds(next); }} /><span>{tag.name}</span><small>{tag.cardCount}</small></label>)}{tagIds.size > 0 ? <button type="button" className="text-button" onClick={() => setTagIds(new Set())}>Clear tag filters</button> : null}</div></details>
      </section>

      {selectedCards.size > 0 ? (
        <section className="card-selection-bar" aria-live="polite">
          <strong>{selectedCards.size} selected</strong>
          <select aria-label="Actions for selected cards" value="" disabled={isApplyingSchedule} onChange={(event) => { const action = event.currentTarget.value as SelectionAction | ""; if (action !== "") chooseSelectionAction(action); }}>
            <option value="">Actions…</option><option value="move_copy">Move / copy source notes</option><option value="add_tags">Add tags</option><option value="remove_tags">Remove tags</option><option value="suspend">Suspend cards</option><option value="resume">Resume cards</option><option value="bury">Bury until tomorrow</option><option value="unbury">Unbury cards</option><option value="reschedule">Reschedule cards</option><option value="reset">Reset schedules</option><option value="delete">Delete source notes</option>
          </select>
          <button type="button" className="secondary-button" onClick={() => { setPracticeSourceLabel("the selected cards"); setPracticeCards(selected); }}>Practice selected</button>
          <button type="button" className="text-button" onClick={() => setSelectedCards(new Map())}>Deselect all</button>
        </section>
      ) : null}

      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      {isLoading ? <div className="inline-progress">Searching cards…</div> : null}
      {!isLoading && page.totalCount === 0 ? <section className="compact-empty-state large-empty-state"><h2>No matching cards</h2><p>Try removing a filter, changing the search, or adding cards.</p></section> : null}

      {page.totalCount > 0 ? (
        <section className="card-library-panel">
          <div className="card-library-summary"><div className="result-count"><strong>{page.cards.length} of {page.totalCount}</strong><span> cards shown</span></div><div className="selection-buttons" aria-label="Selection controls"><button type="button" className="secondary-button" onClick={selectVisible}>Select visible</button><button type="button" className="secondary-button" disabled={isSelectingAll} onClick={() => void selectAllMatching()}>{isSelectingAll ? "Selecting…" : `Select all (${page.totalCount})`}</button><button type="button" className="secondary-button" disabled={selectedCards.size === 0} onClick={() => setSelectedCards(new Map())}>Deselect all</button></div><button type="button" className="primary-button" disabled={isResolvingPractice || statusFilter === "suspended" || statusFilter === "buried"} onClick={() => void practiceAllResults()}>{isResolvingPractice ? "Preparing…" : "Practice results"}</button></div>
          <div className="organized-card-list" role="table" aria-label="Matching cards">
            {page.cards.map((card) => {
              const selectedRow = selectedCards.has(card.cardId);
              return (
                <article key={card.cardId} className={selectedRow ? "organized-card-row selected" : "organized-card-row"} role="row">
                  <label className="card-row-selector" onClick={(event) => event.stopPropagation()}><input type="checkbox" checked={selectedRow} aria-label={`Select ${summarize(card.frontText)}`} onChange={() => toggleSelected(selectionFromRow(card))} /></label>
                  <button type="button" className="organized-card-main" onClick={() => void openCard(card.noteId)}>
                    <span className="organized-card-deck">{card.deckName}<span className="card-type-badge">{card.cardLabel}</span></span>
                    <span className="organized-card-statuses">{card.suspendedAt ? <span className="schedule-status-badge suspended">Suspended</span> : null}{card.buriedUntil && card.buriedUntil > Date.now() ? <span className="schedule-status-badge buried">Buried</span> : null}{card.needsMorePractice ? <span className="schedule-status-badge needs-practice">Needs more practice</span> : null}</span>
                    <span className="organized-card-sides"><span><small>Question</small><strong>{summarize(card.frontText)}</strong></span><span aria-hidden="true">→</span><span><small>Answer</small><strong>{summarize(card.backText)}</strong></span></span>
                    <span className="organized-card-tags">{card.tags.length === 0 ? <small>No tags</small> : card.tags.map((tag) => <span className="tag-chip static" key={tag}>{tag}</span>)}</span>
                  </button>
                  <div className="organized-card-meta"><span>{formatDue(card.dueAt)}</span><small>{formatLastStudied(card.lastReviewedAt)}</small></div>
                </article>
              );
            })}
          </div>
          {page.cards.length < page.totalCount ? <button type="button" className="secondary-button load-more-cards" disabled={isLoadingMore} onClick={() => void loadCards(page.cards.length, true)}>{isLoadingMore ? "Loading…" : "Load more"}</button> : null}
        </section>
      ) : null}

      {bulkOpen ? <BulkEntryModal decks={decks} defaultDeckId={selectedDeck?.id} onClose={() => setBulkOpen(false)} onSaved={refreshAfterMutation} showToast={showToast} /> : null}
      {renderSelectionModal()}
      {practiceCards ? <PracticeResultsModal cards={practiceCards} sourceLabel={practiceSourceLabel} refreshDecks={refreshDecks} onPracticeCards={onPracticeCards} onClose={() => setPracticeCards(null)} showToast={showToast} /> : null}
    </div>
  );
}

function uniqueNotes(cards: SelectionCard[]): SelectionCard[] {
  const byNoteId = new Map<string, SelectionCard>();
  for (const card of cards) if (!byNoteId.has(card.noteId)) byNoteId.set(card.noteId, card);
  return [...byNoteId.values()];
}

function selectionFromRow(card: CardBrowserRow): SelectionCard {
  return { cardId: card.cardId, noteId: card.noteId, deckId: card.deckId };
}

function deckFilterSummary(deckIds: Set<string>, decks: Deck[]): string {
  if (deckIds.size === 0) return "All decks";
  const selectedNames = decks.filter((deck) => deckIds.has(deck.id)).map((deck) => deck.name);
  return selectedNames.length === 1 ? selectedNames[0] ?? "1 deck" : `${selectedNames.length} decks`;
}

function summarize(value: string): string {
  const normalized = value.replace(/\s+/gu, " ").trim();
  if (normalized === "") return "Image card";
  return normalized.length > 150 ? `${normalized.slice(0, 147)}…` : normalized;
}

function formatDue(value: number): string {
  const now = Date.now();
  if (value <= now) return "Due now";
  const days = Math.ceil((value - now) / 86_400_000);
  if (days <= 1) return "Due tomorrow";
  if (days < 14) return `Due in ${days} days`;
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(value));
}

function formatLastStudied(value: number | null): string {
  if (value === null) return "Never studied";
  const days = Math.floor(Math.max(0, Date.now() - value) / 86_400_000);
  if (days === 0) return "Studied within 24 hours";
  if (days === 1) return "Studied 1 day ago";
  if (days < 30) return `Studied ${days} days ago`;
  return `Studied ${new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(value))}`;
}
