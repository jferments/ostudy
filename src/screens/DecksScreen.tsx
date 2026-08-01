import {
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { FormEvent, KeyboardEvent as ReactKeyboardEvent } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

import { Modal } from "../components/Modal";
import {
  browseCards,
  createDeck,
  deleteCard,
  deleteDeck,
  exportDeck,
  getDeck,
  getErrorMessage,
  importDeck,
  updateDeck,
  type AppSettings,
  type CardBrowserItem,
  type CardBrowserResult,
  type CardBrowseSort,
  type CardBrowseStatus,
  type CardTransferMode,
  type CreateDeckInput,
  type Deck,
} from "../ipc";
import { CardEditorScreen } from "./CardEditorScreen";
import { BulkEntryModal } from "./decks/BulkEntryModal";
import { CardTransferModal } from "./decks/CardTransferModal";

interface DecksScreenProps {
  decks: Deck[];
  settings: AppSettings;
  refreshDecks: () => Promise<Deck[]>;
  onPracticeDeck: (deckId: string) => void;
  onDirtyChange: (dirty: boolean) => void;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

type DeckView = "library" | "detail" | "editor";

const PAGE_SIZE = 100;
const EMPTY_BROWSER: CardBrowserResult = {
  items: [],
  total: 0,
  tags: [],
  limit: PAGE_SIZE,
  offset: 0,
};

export function DecksScreen({
  decks,
  settings,
  refreshDecks,
  onPracticeDeck,
  onDirtyChange,
  showToast,
}: DecksScreenProps) {
  const [view, setView] = useState<DeckView>("library");
  const [selectedDeck, setSelectedDeck] = useState<Deck | null>(null);
  const [browser, setBrowser] = useState<CardBrowserResult>(EMPTY_BROWSER);
  const [editingItem, setEditingItem] = useState<CardBrowserItem | null>(null);
  const [deckDialog, setDeckDialog] = useState<"create" | "edit" | null>(null);
  const [bulkEntryOpen, setBulkEntryOpen] = useState(false);
  const [transferMode, setTransferMode] = useState<CardTransferMode | null>(null);
  const [isLoadingDeck, setIsLoadingDeck] = useState(false);
  const loadingDeckRef = useRef(false);
  const [isLoadingCards, setIsLoadingCards] = useState(false);
  const browseRequestRef = useRef(0);
  const [isTransferring, setIsTransferring] = useState(false);
  const transferringRef = useRef(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [selectedTagIds, setSelectedTagIds] = useState<string[]>([]);
  const [status, setStatus] = useState<CardBrowseStatus>("all");
  const [sort, setSort] = useState<CardBrowseSort>("updated_desc");
  const [offset, setOffset] = useState(0);
  const [selectedNoteIds, setSelectedNoteIds] = useState<string[]>([]);
  const [refreshToken, setRefreshToken] = useState(0);
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const rowRefs = useRef<Array<HTMLButtonElement | null>>([]);

  const selectedDeckId = selectedDeck?.id ?? null;
  const selectedNoteIdSet = useMemo(
    () => new Set(selectedNoteIds),
    [selectedNoteIds],
  );

  useEffect(() => {
    if (!selectedDeckId) return;
    const updated = decks.find((deck) => deck.id === selectedDeckId);
    if (!updated) return;

    setSelectedDeck((current) => {
      if (!current || current.id !== updated.id) return current;
      if (
        current.name === updated.name &&
        current.description === updated.description &&
        current.cardCount === updated.cardCount &&
        current.dueCount === updated.dueCount &&
        current.updatedAt === updated.updatedAt &&
        current.revision === updated.revision
      ) {
        return current;
      }
      return updated;
    });
  }, [decks, selectedDeckId]);

  useEffect(() => {
    if (view !== "detail" || selectedDeckId === null) return;
    const requestId = ++browseRequestRef.current;
    const delay = search.trim() === "" ? 0 : 180;
    const timer = window.setTimeout(() => {
      setIsLoadingCards(true);
      setErrorMessage(null);
      void browseCards({
        deckId: selectedDeckId,
        query: search,
        tagIds: selectedTagIds,
        status,
        sort,
        limit: PAGE_SIZE,
        offset,
      })
        .then((result) => {
          if (requestId === browseRequestRef.current) {
            setBrowser(result);
          }
        })
        .catch((error: unknown) => {
          if (requestId === browseRequestRef.current) {
            setErrorMessage(getErrorMessage(error));
          }
        })
        .finally(() => {
          if (requestId === browseRequestRef.current) {
            setIsLoadingCards(false);
          }
        });
    }, delay);
    return () => window.clearTimeout(timer);
  }, [
    offset,
    refreshToken,
    search,
    selectedDeckId,
    selectedTagIds,
    sort,
    status,
    view,
  ]);

  useEffect(() => {
    if (view !== "detail") return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key !== "/" || isTypingTarget(event.target)) return;
      event.preventDefault();
      searchInputRef.current?.focus();
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [view]);

  function resetBrowserState(): void {
    setBrowser(EMPTY_BROWSER);
    setSearch("");
    setSelectedTagIds([]);
    setStatus("all");
    setSort("updated_desc");
    setOffset(0);
    setSelectedNoteIds([]);
  }

  async function openDeck(deckId: string): Promise<void> {
    if (loadingDeckRef.current) return;
    loadingDeckRef.current = true;
    setIsLoadingDeck(true);
    setErrorMessage(null);
    try {
      const deck = await getDeck(deckId);
      setSelectedDeck(deck);
      setEditingItem(null);
      resetBrowserState();
      setView("detail");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      loadingDeckRef.current = false;
      setIsLoadingDeck(false);
    }
  }

  async function refreshDeckListAfterMutation(): Promise<void> {
    try {
      await refreshDecks();
    } catch (error: unknown) {
      showToast(
        "info",
        `The change was saved, but the deck list could not refresh: ${getErrorMessage(error)}`,
      );
    }
  }

  async function refreshSelectedDeckAfterMutation(deckId: string): Promise<void> {
    try {
      setSelectedDeck(await getDeck(deckId));
      setRefreshToken((value) => value + 1);
    } catch (error: unknown) {
      showToast(
        "info",
        `The change was saved, but the open deck could not refresh: ${getErrorMessage(error)}`,
      );
    }
    await refreshDeckListAfterMutation();
  }

  async function handleDeckSaved(input: CreateDeckInput): Promise<void> {
    if (deckDialog === "edit" && selectedDeck) {
      const updated = await updateDeck({
        deckId: selectedDeck.id,
        name: input.name,
        description: input.description,
        revision: selectedDeck.revision,
      });
      setSelectedDeck(updated);
      setDeckDialog(null);
      showToast("success", "Deck details updated.");
      void refreshDeckListAfterMutation();
      return;
    }

    const created = await createDeck(input);
    setSelectedDeck(created);
    setEditingItem(null);
    resetBrowserState();
    setView("detail");
    setDeckDialog(null);
    showToast("success", `Created “${created.name}”.`);
    void refreshDeckListAfterMutation();
  }

  async function handleDeleteDeck(deck: Deck): Promise<void> {
    if (
      settings.confirmDeletions &&
      !window.confirm(
        `Delete “${deck.name}” and its ${deck.cardCount} ${deck.cardCount === 1 ? "card" : "cards"}?`,
      )
    ) {
      return;
    }
    try {
      await deleteDeck(deck.id);
      if (selectedDeck?.id === deck.id) {
        setSelectedDeck(null);
        setEditingItem(null);
        resetBrowserState();
        setView("library");
      }
      showToast("success", `Deleted “${deck.name}”.`);
      void refreshDeckListAfterMutation();
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function handleDeleteCard(item: CardBrowserItem): Promise<void> {
    if (
      settings.confirmDeletions &&
      !window.confirm(
        `Delete this flashcard?\n\n${item.card.frontText || "Image card"}`,
      )
    ) {
      return;
    }
    try {
      await deleteCard(item.card.noteId);
      setSelectedNoteIds((current) =>
        current.filter((noteId) => noteId !== item.card.noteId),
      );
      showToast("success", "Flashcard deleted.");
      if (selectedDeck) await refreshSelectedDeckAfterMutation(selectedDeck.id);
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function handleExport(deck: Deck): Promise<void> {
    if (transferringRef.current) return;
    transferringRef.current = true;
    setIsTransferring(true);
    try {
      const destination = await saveDialog({
        title: "Export deck",
        defaultPath: `${safeFilename(deck.name)}.studydeck`,
        filters: [{ name: "ostudy deck", extensions: ["studydeck"] }],
      });
      if (typeof destination !== "string") return;
      const result = await exportDeck(deck.id, destination);
      showToast(
        "success",
        `Exported ${result.cardCount} ${result.cardCount === 1 ? "card" : "cards"}.`,
      );
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      transferringRef.current = false;
      setIsTransferring(false);
    }
  }

  async function handleImport(): Promise<void> {
    if (transferringRef.current) return;
    transferringRef.current = true;
    setIsTransferring(true);
    try {
      const selected = await openDialog({
        title: "Import an ostudy deck",
        multiple: false,
        directory: false,
        filters: [{ name: "ostudy deck", extensions: ["studydeck"] }],
      });
      if (typeof selected !== "string") return;

      const result = await importDeck(selected);
      setSelectedDeck(result.deck);
      setEditingItem(null);
      resetBrowserState();
      setView("detail");
      showToast(
        "success",
        `Imported “${result.deck.name}” with ${result.cardCount} ${result.cardCount === 1 ? "card" : "cards"}.`,
      );
      void refreshDeckListAfterMutation();
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      transferringRef.current = false;
      setIsTransferring(false);
    }
  }

  function openEditor(item: CardBrowserItem | null): void {
    setEditingItem(item);
    setView("editor");
  }

  function toggleSelection(noteId: string): void {
    setSelectedNoteIds((current) =>
      current.includes(noteId)
        ? current.filter((value) => value !== noteId)
        : [...current, noteId],
    );
  }

  function handleCardListKeyDown(event: ReactKeyboardEvent<HTMLDivElement>): void {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
      event.preventDefault();
      setSelectedNoteIds((current) => [
        ...new Set([...current, ...browser.items.map((item) => item.card.noteId)]),
      ]);
    }
  }

  function handleRowKeyDown(
    event: ReactKeyboardEvent<HTMLButtonElement>,
    item: CardBrowserItem,
    index: number,
  ): void {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      rowRefs.current[Math.min(index + 1, browser.items.length - 1)]?.focus();
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      rowRefs.current[Math.max(index - 1, 0)]?.focus();
    } else if (event.key === " ") {
      event.preventDefault();
      toggleSelection(item.card.noteId);
    }
  }

  function updateFilter(mutator: () => void): void {
    mutator();
    setOffset(0);
    setSelectedNoteIds([]);
  }

  if (view === "editor" && selectedDeck) {
    return (
      <CardEditorScreen
        key={editingItem?.card.noteId ?? "new-card"}
        deck={selectedDeck}
        card={editingItem?.card ?? null}
        initialTags={editingItem?.tags ?? []}
        availableTags={browser.tags}
        onDirtyChange={onDirtyChange}
        onCancel={() => {
          onDirtyChange(false);
          setEditingItem(null);
          setView("detail");
        }}
        onSaved={async (_saved, continueAdding) => {
          const wasEditing = editingItem !== null;
          showToast("success", wasEditing ? "Flashcard updated." : "Flashcard saved.");
          if (!continueAdding) {
            onDirtyChange(false);
            setEditingItem(null);
            setView("detail");
          }
          await refreshSelectedDeckAfterMutation(selectedDeck.id);
        }}
      />
    );
  }

  return (
    <div className="screen decks-screen">
      {view === "library" ? (
        <>
          <header className="screen-header compact-screen-header">
            <div>
              <h1>Decks</h1>
            </div>
            <div className="header-actions">
              <button
                type="button"
                className="secondary-button"
                disabled={isTransferring}
                onClick={() => void handleImport()}
              >
                Import deck
              </button>
              <button
                type="button"
                className="primary-button"
                disabled={isTransferring}
                onClick={() => setDeckDialog("create")}
              >
                New deck
              </button>
            </div>
          </header>

          {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
          {isLoadingDeck ? <div className="inline-progress">Opening deck…</div> : null}

          {decks.length === 0 ? (
            <section className="empty-library-card">
              <div className="empty-icon" aria-hidden="true">?</div>
              <h2>Create your first deck</h2>
              <div className="empty-actions">
                <button
                  type="button"
                  className="primary-button"
                  disabled={isTransferring}
                  onClick={() => setDeckDialog("create")}
                >
                  Create deck
                </button>
                <button
                  type="button"
                  className="secondary-button"
                  disabled={isTransferring}
                  onClick={() => void handleImport()}
                >
                  Import deck
                </button>
              </div>
            </section>
          ) : (
            <section className="deck-grid" aria-label="Deck library">
              {decks.map((deck) => (
                <article key={deck.id} className="deck-card">
                  <button
                    type="button"
                    className="deck-card-main"
                    onClick={() => void openDeck(deck.id)}
                  >
                    <span className="deck-card-mark" aria-hidden="true">▤</span>
                    <span className="deck-card-copy">
                      <strong>{deck.name}</strong>
                      {deck.description ? <span>{deck.description}</span> : null}
                    </span>
                    <span className="deck-card-count">
                      <strong>{deck.cardCount}</strong>
                      <small>{deck.cardCount === 1 ? "card" : "cards"}</small>
                    </span>
                  </button>
                  <div className="deck-card-actions">
                    <button
                      type="button"
                      onClick={() => onPracticeDeck(deck.id)}
                      disabled={deck.cardCount === 0}
                      title={practiceTitle(deck)}
                    >
                      Practice
                    </button>
                    <button
                      type="button"
                      disabled={isTransferring}
                      onClick={() => void handleExport(deck)}
                    >
                      Export
                    </button>
                    <button
                      type="button"
                      className="danger-text"
                      onClick={() => void handleDeleteDeck(deck)}
                    >
                      Delete
                    </button>
                  </div>
                </article>
              ))}
            </section>
          )}
        </>
      ) : null}

      {view === "detail" && selectedDeck ? (
        <>
          <header className="screen-header deck-detail-header">
            <div>
              <button type="button" className="back-button" onClick={() => setView("library")}>
                ← All decks
              </button>
              <div className="deck-title-row">
                <div>
                  <h1>{selectedDeck.name}</h1>
                  {selectedDeck.description ? <p>{selectedDeck.description}</p> : null}
                </div>
                <button
                  type="button"
                  className="icon-button deck-edit-button"
                  aria-label="Edit deck name and description"
                  title="Edit deck"
                  onClick={() => setDeckDialog("edit")}
                >
                  ✎
                </button>
              </div>
            </div>
            <div className="header-actions wrap-actions">
              <button
                type="button"
                className="secondary-button"
                onClick={() => onPracticeDeck(selectedDeck.id)}
                disabled={selectedDeck.cardCount === 0}
                title={practiceTitle(selectedDeck)}
              >
                Practice
              </button>
              <button
                type="button"
                className="secondary-button"
                disabled={isTransferring}
                onClick={() => void handleExport(selectedDeck)}
              >
                Export
              </button>
              <button
                type="button"
                className="secondary-button"
                onClick={() => setBulkEntryOpen(true)}
              >
                Bulk add
              </button>
              <button
                type="button"
                className="primary-button"
                onClick={() => openEditor(null)}
              >
                Add flashcard
              </button>
            </div>
          </header>

          <section className="card-browser-panel" aria-busy={isLoadingCards}>
            <div className="card-browser-toolbar stage2-browser-toolbar">
              <label className="search-field">
                <span>Search this deck</span>
                <input
                  ref={searchInputRef}
                  type="search"
                  value={search}
                  onChange={(event) =>
                    updateFilter(() => {
                      const next = event.currentTarget.value;
                      setSearch(next);
                      if (next.trim() !== "" && search.trim() === "") {
                        setSort("relevance");
                      } else if (next.trim() === "" && sort === "relevance") {
                        setSort("updated_desc");
                      }
                    })
                  }
                  placeholder="Search questions, answers, and tags  /"
                />
              </label>
              <label className="browser-select-field">
                <span>Status</span>
                <select
                  value={status}
                  onChange={(event) =>
                    updateFilter(() =>
                      setStatus(event.currentTarget.value as CardBrowseStatus),
                    )
                  }
                >
                  <option value="all">All cards</option>
                  <option value="due">Due now</option>
                  <option value="new">New</option>
                  <option value="reviewed">Reviewed</option>
                </select>
              </label>
              <label className="browser-select-field">
                <span>Sort</span>
                <select
                  value={sort}
                  onChange={(event) =>
                    updateFilter(() =>
                      setSort(event.currentTarget.value as CardBrowseSort),
                    )
                  }
                >
                  {search.trim() !== "" ? <option value="relevance">Best match</option> : null}
                  <option value="updated_desc">Recently updated</option>
                  <option value="created_desc">Recently created</option>
                  <option value="question_asc">Question A–Z</option>
                  <option value="due_asc">Next due</option>
                </select>
              </label>
              <span className="card-count-summary">
                {browser.total} {browser.total === 1 ? "flashcard" : "flashcards"}
                {selectedDeck.dueCount > 0 ? ` · ${selectedDeck.dueCount} due` : ""}
              </span>
            </div>

            {browser.tags.length > 0 ? (
              <div className="tag-filter-row" aria-label="Filter cards by tag">
                <span>Tags</span>
                {browser.tags.map((tag) => {
                  const selected = selectedTagIds.includes(tag.id);
                  return (
                    <button
                      key={tag.id}
                      type="button"
                      className={selected ? "tag-filter active" : "tag-filter"}
                      aria-pressed={selected}
                      onClick={() =>
                        updateFilter(() =>
                          setSelectedTagIds((current) =>
                            current.includes(tag.id)
                              ? current.filter((id) => id !== tag.id)
                              : [...current, tag.id],
                          ),
                        )
                      }
                    >
                      {tag.name} <span>{tag.cardCount}</span>
                    </button>
                  );
                })}
                {selectedTagIds.length > 0 ? (
                  <button
                    type="button"
                    className="clear-tag-filters"
                    onClick={() => updateFilter(() => setSelectedTagIds([]))}
                  >
                    Clear tags
                  </button>
                ) : null}
              </div>
            ) : null}

            {selectedNoteIds.length > 0 ? (
              <div className="card-selection-bar">
                <strong>
                  {selectedNoteIds.length} {selectedNoteIds.length === 1 ? "card" : "cards"} selected
                </strong>
                <div>
                  <button
                    type="button"
                    className="secondary-button"
                    onClick={() => setTransferMode("move")}
                  >
                    Move
                  </button>
                  <button
                    type="button"
                    className="secondary-button"
                    onClick={() => setTransferMode("copy")}
                  >
                    Copy
                  </button>
                  <button type="button" onClick={() => setSelectedNoteIds([])}>
                    Clear selection
                  </button>
                </div>
              </div>
            ) : null}

            {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
            {isLoadingCards ? <div className="inline-progress">Finding cards…</div> : null}

            {!isLoadingCards && selectedDeck.cardCount === 0 ? (
              <section className="compact-empty-state large-empty-state">
                <div className="empty-icon" aria-hidden="true">+</div>
                <h2>This deck is empty</h2>
                <p>Add one rich-text card or paste many question-and-answer rows at once.</p>
                <div className="empty-actions">
                  <button
                    type="button"
                    className="primary-button"
                    onClick={() => openEditor(null)}
                  >
                    Add the first flashcard
                  </button>
                  <button
                    type="button"
                    className="secondary-button"
                    onClick={() => setBulkEntryOpen(true)}
                  >
                    Bulk add cards
                  </button>
                </div>
              </section>
            ) : null}

            {!isLoadingCards && selectedDeck.cardCount > 0 && browser.items.length === 0 ? (
              <div className="compact-empty-state">
                <h3>No matching flashcards</h3>
                <p>Change the search text, status, or tag filters.</p>
              </div>
            ) : null}

            {browser.items.length > 0 ? (
              <div
                className="card-list selectable-card-list"
                onKeyDown={handleCardListKeyDown}
              >
                {browser.items.map((item, index) => {
                  const selected = selectedNoteIdSet.has(item.card.noteId);
                  return (
                    <article
                      key={item.card.noteId}
                      className={selected ? "card-row selected" : "card-row"}
                    >
                      <label className="card-select-checkbox">
                        <input
                          type="checkbox"
                          checked={selected}
                          aria-label={`Select ${summarize(item.card.frontText, item.card.assets.length > 0)}`}
                          onChange={() => toggleSelection(item.card.noteId)}
                        />
                      </label>
                      <button
                        ref={(element) => {
                          rowRefs.current[index] = element;
                        }}
                        type="button"
                        className="card-row-main"
                        onClick={() => openEditor(item)}
                        onKeyDown={(event) => handleRowKeyDown(event, item, index)}
                      >
                        <span className="card-number">{offset + index + 1}</span>
                        <span className="card-row-side">
                          <small>Question</small>
                          <strong>{summarize(item.card.frontText, item.card.assets.length > 0)}</strong>
                        </span>
                        <span className="card-row-divider" aria-hidden="true">→</span>
                        <span className="card-row-side">
                          <small>Answer</small>
                          <strong>{summarize(item.card.backText, item.card.assets.length > 0)}</strong>
                        </span>
                      </button>
                      <div className="card-row-metadata">
                        <span className={`schedule-badge ${scheduleClass(item)}`}>
                          {scheduleLabel(item)}
                        </span>
                        {item.tags.map((tag) => (
                          <span key={tag.id} className="tag-chip">{tag.name}</span>
                        ))}
                      </div>
                      <div className="card-row-actions">
                        <span>
                          {item.card.assets.length > 0
                            ? `${item.card.assets.length} image${item.card.assets.length === 1 ? "" : "s"}`
                            : "Text"}
                        </span>
                        <button type="button" onClick={() => openEditor(item)}>Edit</button>
                        <button
                          type="button"
                          className="danger-text"
                          onClick={() => void handleDeleteCard(item)}
                        >
                          Delete
                        </button>
                      </div>
                    </article>
                  );
                })}
              </div>
            ) : null}

            {browser.total > PAGE_SIZE ? (
              <nav className="card-pagination" aria-label="Card pages">
                <button
                  type="button"
                  className="secondary-button"
                  disabled={offset === 0 || isLoadingCards}
                  onClick={() => {
                    setOffset((value) => Math.max(0, value - PAGE_SIZE));
                    setSelectedNoteIds([]);
                  }}
                >
                  Previous
                </button>
                <span>
                  {offset + 1}–{Math.min(offset + PAGE_SIZE, browser.total)} of {browser.total}
                </span>
                <button
                  type="button"
                  className="secondary-button"
                  disabled={offset + PAGE_SIZE >= browser.total || isLoadingCards}
                  onClick={() => {
                    setOffset((value) => value + PAGE_SIZE);
                    setSelectedNoteIds([]);
                  }}
                >
                  Next
                </button>
              </nav>
            ) : null}
          </section>
        </>
      ) : null}

      {deckDialog ? (
        <DeckFormModal
          mode={deckDialog}
          deck={deckDialog === "edit" ? selectedDeck : null}
          onClose={() => setDeckDialog(null)}
          onSave={handleDeckSaved}
        />
      ) : null}

      {bulkEntryOpen && selectedDeck ? (
        <BulkEntryModal
          deck={selectedDeck}
          onClose={() => setBulkEntryOpen(false)}
          onComplete={async (result) => {
            setBulkEntryOpen(false);
            const skipped = result.skipped.length;
            showToast(
              skipped > 0 ? "info" : "success",
              `Added ${result.createdCount} ${result.createdCount === 1 ? "card" : "cards"}${skipped > 0 ? ` and skipped ${skipped} duplicate${skipped === 1 ? "" : "s"}` : ""}.`,
            );
            await refreshSelectedDeckAfterMutation(selectedDeck.id);
          }}
        />
      ) : null}

      {transferMode && selectedDeck ? (
        <CardTransferModal
          decks={decks}
          currentDeck={selectedDeck}
          noteIds={selectedNoteIds}
          initialMode={transferMode}
          onClose={() => setTransferMode(null)}
          onComplete={async (mode, destinationDeckId, result) => {
            const destination = decks.find((deck) => deck.id === destinationDeckId);
            setTransferMode(null);
            setSelectedNoteIds([]);
            const skipped = result.skipped.length;
            showToast(
              skipped > 0 ? "info" : "success",
              `${mode === "move" ? "Moved" : "Copied"} ${result.transferredCount} ${result.transferredCount === 1 ? "card" : "cards"}${destination ? ` to “${destination.name}”` : ""}${skipped > 0 ? `; skipped ${skipped} duplicate${skipped === 1 ? "" : "s"}` : ""}.`,
            );
            await refreshSelectedDeckAfterMutation(selectedDeck.id);
          }}
        />
      ) : null}
    </div>
  );
}

interface DeckFormModalProps {
  mode: "create" | "edit";
  deck: Deck | null;
  onClose: () => void;
  onSave: (input: CreateDeckInput) => Promise<void>;
}

function DeckFormModal({ mode, deck, onClose, onSave }: DeckFormModalProps) {
  const [name, setName] = useState(deck?.name ?? "");
  const [description, setDescription] = useState(deck?.description ?? "");
  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (savingRef.current) return;
    if (name.trim() === "") {
      setErrorMessage("Enter a name for the deck.");
      return;
    }
    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      await onSave({ name, description });
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  }

  return (
    <Modal
      title={mode === "create" ? "Create a deck" : "Edit deck"}
      description={undefined}
      onClose={() => {
        if (!isSaving) onClose();
      }}
      width="small"
    >
      <form className="form-stack" onSubmit={(event) => void submit(event)}>
        <label>
          <span>Deck name</span>
          <input
            autoFocus
            disabled={isSaving}
            value={name}
            maxLength={200}
            onChange={(event) => setName(event.currentTarget.value)}
            placeholder="NCLEX Pharmacology"
          />
        </label>
        <label>
          <span>Description <small>optional</small></span>
          <textarea
            value={description}
            disabled={isSaving}
            maxLength={2000}
            rows={4}
            onChange={(event) => setDescription(event.currentTarget.value)}
            placeholder="What this deck covers"
          />
        </label>
        {errorMessage ? <p className="field-error">{errorMessage}</p> : null}
        <div className="modal-actions">
          <button
            type="button"
            className="secondary-button"
            disabled={isSaving}
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="submit"
            className="primary-button"
            disabled={isSaving || name.trim() === ""}
          >
            {isSaving ? "Saving…" : mode === "create" ? "Create deck" : "Save changes"}
          </button>
        </div>
      </form>
    </Modal>
  );
}

function practiceTitle(deck: Deck): string {
  if (deck.cardCount === 0) return "Add a flashcard before practicing this deck";
  if (deck.dueCount === 0) return "Practice all cards; none are currently due";
  return `${deck.dueCount} ${deck.dueCount === 1 ? "card is" : "cards are"} due`;
}

function safeFilename(name: string): string {
  const cleaned = name
    .trim()
    .replace(/[\\/:*?"<>|]+/gu, "-")
    .replace(/\s+/gu, " ")
    .slice(0, 120);
  return cleaned || "deck";
}

function summarize(text: string, hasImage: boolean): string {
  const normalized = text.replace(/\s+/gu, " ").trim();
  if (normalized === "") return hasImage ? "Image content" : "Empty";
  return normalized.length > 150 ? `${normalized.slice(0, 147)}…` : normalized;
}

function scheduleLabel(item: CardBrowserItem): string {
  if (item.reviewCount === 0) return "New";
  if (item.dueAt <= Date.now()) return "Due";
  return "Reviewed";
}

function scheduleClass(item: CardBrowserItem): string {
  return scheduleLabel(item).toLocaleLowerCase();
}

function isTypingTarget(target: EventTarget | null): boolean {
  return target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable);
}
