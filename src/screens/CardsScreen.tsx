import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { ChangeEvent, KeyboardEvent } from "react";

import {
  getErrorMessage,
  getOrganizedCard,
  listTags,
  searchCards,
  type CardBrowserRow,
  type CardSearchPage,
  type CardSort,
  type Deck,
  type DueFilter,
  type OrganizedCard,
  type TagSummary,
} from "../ipc";
import { CardEditorScreen } from "./CardEditorScreen";
import { BulkEntryModal } from "./cards/BulkEntryModal";
import { MoveCopyModal } from "./cards/MoveCopyModal";

interface CardsScreenProps {
  decks: Deck[];
  refreshDecks: () => Promise<Deck[]>;
  onDirtyChange: (dirty: boolean) => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

const PAGE_SIZE = 100;

export function CardsScreen({
  decks,
  refreshDecks,
  onDirtyChange,
  showToast,
}: CardsScreenProps) {
  const [deckId, setDeckId] = useState("");
  const [query, setQuery] = useState("");
  const [dueFilter, setDueFilter] = useState<DueFilter>("all");
  const [sort, setSort] = useState<CardSort>("updated_desc");
  const [tagIds, setTagIds] = useState<Set<string>>(new Set());
  const [tags, setTags] = useState<TagSummary[]>([]);
  const [page, setPage] = useState<CardSearchPage>({
    cards: [],
    totalCount: 0,
  });
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [selectedNoteIds, setSelectedNoteIds] = useState<Set<string>>(new Set());
  const [editingCard, setEditingCard] = useState<OrganizedCard | null>(null);
  const [creatingDeckId, setCreatingDeckId] = useState<string | null>(null);
  const [isOpeningCard, setIsOpeningCard] = useState(false);
  const [bulkOpen, setBulkOpen] = useState(false);
  const [moveCopyOpen, setMoveCopyOpen] = useState(false);
  const [moveCopyCards, setMoveCopyCards] = useState<CardBrowserRow[]>([]);
  const rowRefs = useRef(new Map<string, HTMLButtonElement>());
  const requestSequence = useRef(0);

  const selectedCards = useMemo(
    () => page.cards.filter((card) => selectedNoteIds.has(card.noteId)),
    [page.cards, selectedNoteIds],
  );

  const loadTags = useCallback(async (): Promise<void> => {
    try {
      const loaded = await listTags(deckId || undefined);
      setTags(loaded);
      setTagIds((current) => {
        const valid = new Set(loaded.map((tag) => tag.id));
        return new Set([...current].filter((id) => valid.has(id)));
      });
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    }
  }, [deckId]);

  const loadCards = useCallback(
    async (offset = 0, append = false): Promise<void> => {
      const requestId = requestSequence.current + 1;
      requestSequence.current = requestId;
      if (append) {
        setIsLoadingMore(true);
      } else {
        setIsLoading(true);
        setPage({ cards: [], totalCount: 0 });
      }
      setErrorMessage(null);
      try {
        const result = await searchCards({
          deckId: deckId || undefined,
          query,
          tagIds: [...tagIds],
          dueFilter,
          sort:
            query.trim() === "" && sort === "relevance"
              ? "updated_desc"
              : sort,
          limit: PAGE_SIZE,
          offset,
        });
        if (requestSequence.current !== requestId) return;
        setPage((current) => ({
          totalCount: result.totalCount,
          cards: append ? [...current.cards, ...result.cards] : result.cards,
        }));
        if (!append) {
          setSelectedNoteIds((current) => {
            const visible = new Set(result.cards.map((card) => card.noteId));
            return new Set([...current].filter((id) => visible.has(id)));
          });
        }
      } catch (error: unknown) {
        if (requestSequence.current === requestId) {
          setErrorMessage(getErrorMessage(error));
        }
      } finally {
        if (requestSequence.current === requestId) {
          setIsLoading(false);
          setIsLoadingMore(false);
        }
      }
    },
    [deckId, dueFilter, query, sort, tagIds],
  );

  useEffect(() => {
    void loadTags();
  }, [loadTags]);

  useEffect(() => {
    if (query.trim() === "" && sort === "relevance") {
      setSort("updated_desc");
    }
  }, [query, sort]);

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      void loadCards();
    }, 180);
    return () => window.clearTimeout(timeout);
  }, [loadCards]);

  async function refreshAfterMutation(): Promise<void> {
    await Promise.all([loadCards(), loadTags(), refreshDecks()]);
    setSelectedNoteIds(new Set());
  }

  async function openCard(noteId: string): Promise<void> {
    if (isOpeningCard) return;
    setIsOpeningCard(true);
    setErrorMessage(null);
    try {
      setEditingCard(await getOrganizedCard(noteId));
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsOpeningCard(false);
    }
  }

  function toggleSelected(noteId: string): void {
    setSelectedNoteIds((current) => {
      const next = new Set(current);
      if (next.has(noteId)) next.delete(noteId);
      else next.add(noteId);
      return next;
    });
  }

  function focusRelative(noteId: string, delta: number): void {
    const index = page.cards.findIndex((card) => card.noteId === noteId);
    if (index < 0) return;
    const nextIndex = Math.min(
      page.cards.length - 1,
      Math.max(0, index + delta),
    );
    const nextId = page.cards[nextIndex]?.noteId;
    if (nextId) rowRefs.current.get(nextId)?.focus();
  }

  function onRowKeyDown(
    event: KeyboardEvent<HTMLButtonElement>,
    card: CardBrowserRow,
  ): void {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusRelative(card.noteId, 1);
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      focusRelative(card.noteId, -1);
    } else if (event.key === " ") {
      event.preventDefault();
      toggleSelected(card.noteId);
    } else if (event.key === "Enter") {
      event.preventDefault();
      void openCard(card.noteId);
    } else if (
      (event.ctrlKey || event.metaKey) &&
      event.key.toLowerCase() === "a"
    ) {
      event.preventDefault();
      setSelectedNoteIds(new Set(page.cards.map((item) => item.noteId)));
    } else if (event.key === "Escape") {
      event.preventDefault();
      setSelectedNoteIds(new Set());
    }
  }

  function beginMoveCopy(cards: CardBrowserRow[]): void {
    if (cards.length === 0) return;
    setMoveCopyCards(cards);
    setMoveCopyOpen(true);
  }

  if (editingCard || creatingDeckId) {
    const editorDeckId = editingCard?.deckId ?? creatingDeckId;
    const deck = decks.find((candidate) => candidate.id === editorDeckId);
    if (!deck) {
      return (
        <div className="screen cards-screen">
          <div className="inline-alert error-alert">
            The card’s deck could not be found. Return to the card library and
            refresh.
          </div>
          <button
            type="button"
            className="secondary-button"
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
    const browserRow = editingCard
      ? page.cards.find((card) => card.noteId === editingCard.noteId)
      : undefined;
    return (
      <>
        <CardEditorScreen
          deck={deck}
          card={editingCard}
          onDirtyChange={onDirtyChange}
          onCancel={() => {
            onDirtyChange(false);
            setEditingCard(null);
            setCreatingDeckId(null);
          }}
          onMoveCopy={
            editingCard && browserRow
              ? () => {
                  beginMoveCopy([browserRow]);
                }
              : undefined
          }
          onSaved={async (saved, continueAdding) => {
            onDirtyChange(false);
            const wasEditing = editingCard !== null;
            if (!continueAdding) {
              setEditingCard(null);
              setCreatingDeckId(null);
            }
            await refreshAfterMutation();
            showToast(
              "success",
              wasEditing ? "Flashcard updated." : "Flashcard saved.",
            );
            if (saved.deckId !== deck.id) {
              showToast("info", "The card is now in a different deck.");
            }
          }}
        />
        {moveCopyOpen ? (
          <MoveCopyModal
            cards={moveCopyCards}
            decks={decks}
            onClose={() => {
              setMoveCopyOpen(false);
              setMoveCopyCards([]);
            }}
            onComplete={async () => {
              onDirtyChange(false);
              setEditingCard(null);
              setCreatingDeckId(null);
              await refreshAfterMutation();
            }}
            showToast={showToast}
          />
        ) : null}
      </>
    );
  }

  const selectedDeck = decks.find((deck) => deck.id === deckId) ?? null;
  const visibleSelected =
    page.cards.length > 0 &&
    page.cards.every((card) => selectedNoteIds.has(card.noteId));

  return (
    <div className="screen cards-screen">
      <header className="screen-header compact-screen-header">
        <div>
          <h1>Cards</h1>
          <p>
            Search, filter, tag, move, copy, and add cards across your library.
          </p>
        </div>
        <div className="header-actions">
          <button
            type="button"
            className="secondary-button"
            disabled={decks.length === 0}
            onClick={() => setBulkOpen(true)}
          >
            Bulk add
          </button>
          <button
            type="button"
            className="primary-button"
            disabled={!selectedDeck}
            title={
              selectedDeck ? undefined : "Choose one deck before adding a card"
            }
            onClick={() => {
              if (!selectedDeck) return;
              setEditingCard(null);
              setCreatingDeckId(selectedDeck.id);
            }}
          >
            Add card
          </button>
        </div>
      </header>

      <section className="card-library-toolbar" aria-label="Card filters">
        <label>
          <span>Deck</span>
          <select
            value={deckId}
            onChange={(event: ChangeEvent<HTMLSelectElement>) => {
              setDeckId(event.currentTarget.value);
              setSelectedNoteIds(new Set());
            }}
          >
            <option value="">All decks</option>
            {decks.map((deck) => (
              <option key={deck.id} value={deck.id}>
                {deck.name}
              </option>
            ))}
          </select>
        </label>

        <label className="card-library-search">
          <span>Search cards</span>
          <input
            type="search"
            value={query}
            placeholder="Question, answer, or tag"
            onChange={(event: ChangeEvent<HTMLInputElement>) => {
              setQuery(event.currentTarget.value);
              if (
                event.currentTarget.value.trim() !== "" &&
                sort === "updated_desc"
              ) {
                setSort("relevance");
              }
            }}
          />
        </label>

        <label>
          <span>Schedule</span>
          <select
            value={dueFilter}
            onChange={(event: ChangeEvent<HTMLSelectElement>) =>
              setDueFilter(event.currentTarget.value as DueFilter)
            }
          >
            <option value="all">All cards</option>
            <option value="due">Due now</option>
            <option value="not_due">Not due</option>
          </select>
        </label>

        <label>
          <span>Sort</span>
          <select
            value={sort}
            onChange={(event: ChangeEvent<HTMLSelectElement>) =>
              setSort(event.currentTarget.value as CardSort)
            }
          >
            <option value="relevance" disabled={query.trim() === ""}>
              Relevance
            </option>
            <option value="updated_desc">Recently updated</option>
            <option value="question_asc">Question A–Z</option>
            <option value="due_asc">Next due</option>
          </select>
        </label>

        <details className="tag-filter-menu">
          <summary>Tags{tagIds.size > 0 ? ` (${tagIds.size})` : ""}</summary>
          <div className="tag-filter-list">
            {tags.length === 0 ? (
              <p>No tags in this selection.</p>
            ) : (
              tags.map((tag) => (
                <label key={tag.id}>
                  <input
                    type="checkbox"
                    checked={tagIds.has(tag.id)}
                    onChange={(event: ChangeEvent<HTMLInputElement>) => {
                      setTagIds((current) => {
                        const next = new Set(current);
                        if (event.currentTarget.checked) next.add(tag.id);
                        else next.delete(tag.id);
                        return next;
                      });
                    }}
                  />
                  <span>{tag.name}</span>
                  <small>{tag.cardCount}</small>
                </label>
              ))
            )}
            {tagIds.size > 0 ? (
              <button
                type="button"
                className="text-button"
                onClick={() => setTagIds(new Set())}
              >
                Clear tag filters
              </button>
            ) : null}
          </div>
        </details>
      </section>

      {selectedNoteIds.size > 0 ? (
        <section className="card-selection-bar" aria-live="polite">
          <strong>{selectedNoteIds.size} selected</strong>
          <button
            type="button"
            className="secondary-button"
            onClick={() => beginMoveCopy(selectedCards)}
          >
            Move / copy
          </button>
          <button
            type="button"
            className="text-button"
            onClick={() => setSelectedNoteIds(new Set())}
          >
            Clear
          </button>
        </section>
      ) : null}

      {errorMessage ? (
        <div className="inline-alert error-alert">{errorMessage}</div>
      ) : null}
      {isLoading ? <div className="inline-progress">Searching cards…</div> : null}

      {!isLoading && page.totalCount === 0 ? (
        <section className="compact-empty-state large-empty-state">
          <h2>No matching cards</h2>
          <p>Try removing a filter, changing the search, or adding cards.</p>
        </section>
      ) : null}

      {page.totalCount > 0 ? (
        <section className="card-library-panel">
          <div className="card-library-summary">
            <div>
              <strong>
                {page.cards.length} of {page.totalCount}
              </strong>
              <span> cards shown</span>
            </div>
            <label className="compact-checkbox">
              <input
                type="checkbox"
                checked={visibleSelected}
                onChange={(event: ChangeEvent<HTMLInputElement>) =>
                  setSelectedNoteIds(
                    event.currentTarget.checked
                      ? new Set(page.cards.map((card) => card.noteId))
                      : new Set(),
                  )
                }
              />
              <span>Select visible</span>
            </label>
            <span className="keyboard-hint">
              ↑/↓ navigate · Space selects · Enter edits · Ctrl/⌘+A selects
              visible
            </span>
          </div>

          <div
            className="organized-card-list"
            role="listbox"
            aria-multiselectable="true"
            aria-label="Cards"
          >
            {page.cards.map((card) => {
              const selected = selectedNoteIds.has(card.noteId);
              return (
                <article
                  key={card.noteId}
                  className={
                    selected
                      ? "organized-card-row selected"
                      : "organized-card-row"
                  }
                  role="option"
                  aria-selected={selected}
                >
                  <input
                    type="checkbox"
                    checked={selected}
                    aria-label={`Select ${summarize(card.frontText)}`}
                    onChange={() => toggleSelected(card.noteId)}
                  />
                  <button
                    type="button"
                    ref={(element: HTMLButtonElement | null) => {
                      if (element) rowRefs.current.set(card.noteId, element);
                      else rowRefs.current.delete(card.noteId);
                    }}
                    className="organized-card-main"
                    aria-keyshortcuts="ArrowUp ArrowDown Space Enter Control+A Meta+A Escape"
                    onKeyDown={(event: KeyboardEvent<HTMLButtonElement>) =>
                      onRowKeyDown(event, card)
                    }
                    onClick={() => void openCard(card.noteId)}
                  >
                    <span className="organized-card-deck">{card.deckName}</span>
                    <span className="organized-card-sides">
                      <span>
                        <small>Question</small>
                        <strong>{summarize(card.frontText)}</strong>
                      </span>
                      <span aria-hidden="true">→</span>
                      <span>
                        <small>Answer</small>
                        <strong>{summarize(card.backText)}</strong>
                      </span>
                    </span>
                    <span className="organized-card-tags">
                      {card.tags.length === 0 ? (
                        <small>No tags</small>
                      ) : (
                        card.tags.map((tag) => (
                          <span className="tag-chip static" key={tag}>
                            {tag}
                          </span>
                        ))
                      )}
                    </span>
                  </button>
                  <div className="organized-card-meta">
                    <span>{formatDue(card.dueAt)}</span>
                    <button
                      type="button"
                      className="text-button"
                      onClick={() => beginMoveCopy([card])}
                    >
                      Move / copy
                    </button>
                  </div>
                </article>
              );
            })}
          </div>

          {page.cards.length < page.totalCount ? (
            <button
              type="button"
              className="secondary-button load-more-cards"
              disabled={isLoadingMore}
              onClick={() => void loadCards(page.cards.length, true)}
            >
              {isLoadingMore ? "Loading…" : "Load more"}
            </button>
          ) : null}
        </section>
      ) : null}

      {bulkOpen ? (
        <BulkEntryModal
          decks={decks}
          defaultDeckId={deckId || undefined}
          onClose={() => setBulkOpen(false)}
          onSaved={refreshAfterMutation}
          showToast={showToast}
        />
      ) : null}

      {moveCopyOpen ? (
        <MoveCopyModal
          cards={moveCopyCards}
          decks={decks}
          onClose={() => {
            setMoveCopyOpen(false);
            setMoveCopyCards([]);
          }}
          onComplete={async () => {
            setEditingCard(null);
            setCreatingDeckId(null);
            await refreshAfterMutation();
          }}
          showToast={showToast}
        />
      ) : null}
    </div>
  );
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
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(
    new Date(value),
  );
}
