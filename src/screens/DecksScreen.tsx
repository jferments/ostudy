import { useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

import { Modal } from "../components/Modal";
import {
  browseCards,
  createDeck,
  deleteCard,
  deleteDeck,
  exportDeck,
  getDeck,
  getErrorCode,
  getErrorMessage,
  getNoteForEditing,
  importDeck,
  moveOrCopyNote,
  updateDeck,
  type AppSettings,
  type BrowserCard,
  type CardBrowserSort,
  type CardBrowserStatus,
  type CreateDeckInput,
  type Deck,
  type NoteEditor,
  type NoteMode,
} from "../ipc";
import { CardEditorScreen } from "./CardEditorScreen";

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
  const [cards, setCards] = useState<BrowserCard[]>([]);
  const [totalCards, setTotalCards] = useState(0);
  const [editingNote, setEditingNote] = useState<NoteEditor | null>(null);
  const [deckDialog, setDeckDialog] = useState<"create" | "edit" | null>(null);
  const [moveCopyCard, setMoveCopyCard] = useState<BrowserCard | null>(null);
  const [isLoadingDeck, setIsLoadingDeck] = useState(false);
  const [isLoadingCards, setIsLoadingCards] = useState(false);
  const [isLoadingEditor, setIsLoadingEditor] = useState(false);
  const [isTransferring, setIsTransferring] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [tagFilter, setTagFilter] = useState("");
  const [statusFilter, setStatusFilter] = useState<CardBrowserStatus>("all");
  const [modeFilter, setModeFilter] = useState<"all" | NoteMode>("all");
  const [sort, setSort] = useState<CardBrowserSort>("updated_desc");
  const [page, setPage] = useState(0);
  const [refreshNonce, setRefreshNonce] = useState(0);
  const loadingDeckRef = useRef(false);
  const transferringRef = useRef(false);
  const browserRequestRef = useRef(0);
  const searchInputRef = useRef<HTMLInputElement | null>(null);

  const selectedDeckId = selectedDeck?.id ?? null;
  const tags = useMemo(
    () => tagFilter.split(",").map((tag) => tag.trim()).filter(Boolean),
    [tagFilter],
  );
  const pageCount = Math.max(1, Math.ceil(totalCards / PAGE_SIZE));

  useEffect(() => {
    if (!selectedDeckId) return;
    const updated = decks.find((deck) => deck.id === selectedDeckId);
    if (!updated) return;
    setSelectedDeck(updated);
  }, [decks, selectedDeckId]);

  useEffect(() => {
    if (view !== "detail" || selectedDeckId === null) return;
    const requestId = browserRequestRef.current + 1;
    browserRequestRef.current = requestId;
    const timer = window.setTimeout(() => {
      setIsLoadingCards(true);
      setErrorMessage(null);
      void browseCards({
        deckId: selectedDeckId,
        query: search,
        tags,
        status: statusFilter,
        mode: modeFilter,
        sort,
        limit: PAGE_SIZE,
        offset: page * PAGE_SIZE,
      })
        .then((result) => {
          if (browserRequestRef.current !== requestId) return;
          setCards(result.cards);
          setTotalCards(result.total);
          if (page > 0 && result.cards.length === 0 && result.total > 0) {
            setPage(Math.max(0, Math.ceil(result.total / PAGE_SIZE) - 1));
          }
        })
        .catch((error: unknown) => {
          if (browserRequestRef.current === requestId) {
            setErrorMessage(getErrorMessage(error));
          }
        })
        .finally(() => {
          if (browserRequestRef.current === requestId) setIsLoadingCards(false);
        });
    }, 220);
    return () => window.clearTimeout(timer);
  }, [modeFilter, page, refreshNonce, search, selectedDeckId, sort, statusFilter, tags, view]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent): void {
      if (view !== "detail" || event.ctrlKey || event.metaKey || event.altKey) return;
      const target = event.target;
      const isTyping =
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement ||
        (target instanceof HTMLElement && target.isContentEditable);
      if (event.key === "/" && !isTyping) {
        event.preventDefault();
        searchInputRef.current?.focus();
      } else if (event.key.toLowerCase() === "n" && !isTyping) {
        event.preventDefault();
        openNewEditor();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [view]);

  async function openDeck(deckId: string): Promise<void> {
    if (loadingDeckRef.current) return;
    loadingDeckRef.current = true;
    setIsLoadingDeck(true);
    setErrorMessage(null);
    try {
      const deck = await getDeck(deckId);
      setSelectedDeck(deck);
      setCards([]);
      setTotalCards(0);
      setEditingNote(null);
      resetFilters();
      setView("detail");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      loadingDeckRef.current = false;
      setIsLoadingDeck(false);
    }
  }

  function resetFilters(): void {
    setSearch("");
    setTagFilter("");
    setStatusFilter("all");
    setModeFilter("all");
    setSort("updated_desc");
    setPage(0);
  }

  function triggerCardReload(): void {
    browserRequestRef.current += 1;
    setRefreshNonce((value) => value + 1);
  }

  async function refreshDeckListAfterMutation(): Promise<void> {
    try {
      await refreshDecks();
    } catch (error: unknown) {
      showToast("info", `The change was saved, but the deck list could not refresh: ${getErrorMessage(error)}`);
    }
  }

  async function refreshSelectedDeckAfterMutation(): Promise<void> {
    if (!selectedDeck) return;
    try {
      setSelectedDeck(await getDeck(selectedDeck.id));
    } catch (error: unknown) {
      showToast("info", `The change was saved, but the deck summary could not refresh: ${getErrorMessage(error)}`);
    }
    triggerCardReload();
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
    setCards([]);
    setTotalCards(0);
    setEditingNote(null);
    resetFilters();
    setView("detail");
    setDeckDialog(null);
    showToast("success", `Created “${created.name}”.`);
    void refreshDeckListAfterMutation();
  }

  async function handleDeleteDeck(deck: Deck): Promise<void> {
    if (
      settings.confirmDeletions &&
      !window.confirm(`Delete “${deck.name}” and all ${deck.cardCount} generated review cards?`)
    ) {
      return;
    }
    try {
      await deleteDeck(deck.id);
      if (selectedDeck?.id === deck.id) {
        setSelectedDeck(null);
        setCards([]);
        setView("library");
      }
      showToast("success", `Deleted “${deck.name}”.`);
      void refreshDeckListAfterMutation();
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function handleDeleteCard(card: BrowserCard): Promise<void> {
    if (
      settings.confirmDeletions &&
      !window.confirm(`Delete this note and its ${card.generatedCardCount} generated review card${card.generatedCardCount === 1 ? "" : "s"}?\n\n${summarize(card.frontText, card.hasImages)}`)
    ) {
      return;
    }
    try {
      await deleteCard(card.noteId);
      showToast("success", "Flashcard deleted.");
      await refreshSelectedDeckAfterMutation();
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function openEditorFor(card: BrowserCard): Promise<void> {
    if (isLoadingEditor) return;
    setIsLoadingEditor(true);
    setErrorMessage(null);
    try {
      setEditingNote(await getNoteForEditing(card.noteId));
      setView("editor");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsLoadingEditor(false);
    }
  }

  function openNewEditor(): void {
    setEditingNote(null);
    setView("editor");
  }

  async function handleExport(deck: Deck): Promise<void> {
    if (transferringRef.current) return;
    transferringRef.current = true;
    setIsTransferring(true);
    try {
      const destination = await saveDialog({
        title: "Export deck as a SQLite package",
        defaultPath: `${safeFilename(deck.name)}.studydeck`,
        filters: [{ name: "ostudy SQLite deck", extensions: ["studydeck"] }],
      });
      if (typeof destination !== "string") return;
      const result = await exportDeck(deck.id, destination);
      const details = [
        `${result.cardCount} review ${result.cardCount === 1 ? "card" : "cards"}`,
        result.tagCount === undefined ? null : `${result.tagCount} tags`,
        result.templateCount === undefined ? null : `${result.templateCount} templates`,
      ].filter(Boolean).join(", ");
      showToast("success", `Exported ${details} in one SQLite deck file.`);
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
        title: "Import an ostudy SQLite deck",
        multiple: false,
        directory: false,
        filters: [{ name: "ostudy SQLite deck", extensions: ["studydeck"] }],
      });
      if (typeof selected !== "string") return;
      const result = await importDeck(selected);
      setSelectedDeck(result.deck);
      setCards([]);
      setTotalCards(0);
      setEditingNote(null);
      resetFilters();
      setView("detail");
      showToast("success", `Imported “${result.deck.name}” with ${result.cardCount} generated review cards.`);
      void refreshDeckListAfterMutation();
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      transferringRef.current = false;
      setIsTransferring(false);
    }
  }

  if (view === "editor" && selectedDeck) {
    return (
      <CardEditorScreen
        key={editingNote?.noteId ?? `new-${selectedDeck.id}`}
        deck={selectedDeck}
        note={editingNote}
        onDirtyChange={onDirtyChange}
        showToast={showToast}
        onCancel={() => {
          onDirtyChange(false);
          setEditingNote(null);
          setView("detail");
        }}
        onSaved={async (_saved, continueAdding) => {
          const wasEditing = editingNote !== null;
          showToast("success", wasEditing ? "Flashcard updated." : "Flashcard saved.");
          await refreshSelectedDeckAfterMutation();
          if (!continueAdding) {
            onDirtyChange(false);
            setEditingNote(null);
            setView("detail");
          }
        }}
      />
    );
  }

  return (
    <div className="screen decks-screen" aria-busy={isLoadingDeck || isLoadingCards || isLoadingEditor}>
      {errorMessage ? <div className="inline-alert error-alert" role="alert">{errorMessage}</div> : null}
      {isLoadingDeck || isLoadingEditor ? <div className="inline-progress">Opening deck…</div> : null}

      {view === "library" ? (
        <>
          <header className="screen-header">
            <div>
              <h1>Decks</h1>
              <p>Local SQLite decks with safe templates, tags, and generated review cards.</p>
            </div>
            <div className="header-actions wrap-actions">
              <button type="button" className="secondary-button" disabled={isTransferring} onClick={() => void handleImport()}>
                Import SQLite deck
              </button>
              <button type="button" className="primary-button" onClick={() => setDeckDialog("create")}>
                New deck
              </button>
            </div>
          </header>

          {decks.length === 0 ? (
            <section className="compact-empty-state large-empty-state">
              <div className="empty-icon" aria-hidden="true">▤</div>
              <h2>Create your first deck</h2>
              <p>Decks are stored locally and can be exported as one SQLite file.</p>
              <button type="button" className="primary-button" onClick={() => setDeckDialog("create")}>Create a deck</button>
            </section>
          ) : (
            <section className="deck-grid" aria-label="Deck library">
              {decks.map((deck) => (
                <article className="deck-card" key={deck.id}>
                  <button type="button" className="deck-card-main" onClick={() => void openDeck(deck.id)}>
                    <span className="deck-icon" aria-hidden="true">▤</span>
                    <span className="deck-card-copy">
                      <strong>{deck.name}</strong>
                      {deck.description ? <span>{deck.description}</span> : null}
                    </span>
                    <span className="deck-card-count">
                      <strong>{deck.cardCount}</strong>
                      <small>review cards</small>
                    </span>
                  </button>
                  <div className="deck-card-actions">
                    <button type="button" onClick={() => onPracticeDeck(deck.id)} disabled={deck.dueCount === 0} title={deck.dueCount === 0 ? "No cards are due" : undefined}>Practice</button>
                    <button type="button" disabled={isTransferring} onClick={() => void handleExport(deck)}>Export</button>
                    <button type="button" className="danger-text" onClick={() => void handleDeleteDeck(deck)}>Delete</button>
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
              <button type="button" className="back-button" onClick={() => setView("library")}>← All decks</button>
              <div className="deck-title-row">
                <div>
                  <h1>{selectedDeck.name}</h1>
                  {selectedDeck.description ? <p>{selectedDeck.description}</p> : null}
                </div>
                <button type="button" className="icon-button deck-edit-button" aria-label="Edit deck name and description" title="Edit deck" onClick={() => setDeckDialog("edit")}>✎</button>
              </div>
            </div>
            <div className="header-actions wrap-actions">
              <button type="button" className="secondary-button" onClick={() => onPracticeDeck(selectedDeck.id)} disabled={selectedDeck.dueCount === 0}>Practice</button>
              <button type="button" className="secondary-button" disabled={isTransferring} onClick={() => void handleExport(selectedDeck)}>Export</button>
              <button type="button" className="primary-button" onClick={openNewEditor}>Add flashcard</button>
            </div>
          </header>

          <section className="card-browser-panel">
            <div className="card-browser-toolbar advanced-card-browser-toolbar">
              <label className="search-field">
                <span>Search this deck <small>/</small></span>
                <input
                  ref={searchInputRef}
                  type="search"
                  value={search}
                  onChange={(event) => { setSearch(event.currentTarget.value); setPage(0); }}
                  placeholder="Search text, tags, and template fields"
                />
              </label>
              <label>
                <span>Status</span>
                <select value={statusFilter} onChange={(event) => { setStatusFilter(event.currentTarget.value as CardBrowserStatus); setPage(0); }}>
                  <option value="all">All</option>
                  <option value="due">Due</option>
                  <option value="new">New</option>
                </select>
              </label>
              <label>
                <span>Type</span>
                <select value={modeFilter} onChange={(event) => { setModeFilter(event.currentTarget.value as "all" | NoteMode); setPage(0); }}>
                  <option value="all">All types</option>
                  <option value="basic">Basic</option>
                  <option value="reverse">Reverse</option>
                  <option value="fill_blank">Fill in the blank</option>
                  <option value="template">Template</option>
                </select>
              </label>
              <label>
                <span>Tags</span>
                <input value={tagFilter} onChange={(event) => { setTagFilter(event.currentTarget.value); setPage(0); }} placeholder="tag 1, tag 2" />
              </label>
              <label>
                <span>Sort</span>
                <select value={sort} onChange={(event) => setSort(event.currentTarget.value as CardBrowserSort)}>
                  <option value="updated_desc">Recently edited</option>
                  <option value="created_asc">Oldest created</option>
                  <option value="due_asc">Due first</option>
                  <option value="question_asc">Question A–Z</option>
                </select>
              </label>
            </div>

            <div className="card-browser-summary" aria-live="polite">
              <span>{totalCards} matching {totalCards === 1 ? "note" : "notes"} · {selectedDeck.cardCount} generated review cards · {selectedDeck.dueCount} due</span>
              <span>Keyboard: <kbd>/</kbd> search · <kbd>N</kbd> add</span>
            </div>

            {isLoadingCards ? <div className="inline-progress">Searching cards…</div> : null}
            {!isLoadingCards && totalCards === 0 ? (
              <div className="compact-empty-state large-empty-state">
                <h2>{selectedDeck.cardCount === 0 ? "This deck is empty" : "No cards match these filters"}</h2>
                <p>{selectedDeck.cardCount === 0 ? "Add a flashcard." : "Clear a filter or try another search."}</p>
                {selectedDeck.cardCount === 0 ? <button type="button" className="primary-button" onClick={openNewEditor}>Add the first flashcard</button> : null}
              </div>
            ) : null}

            {cards.length > 0 ? (
              <div className="card-list">
                {cards.map((card, index) => (
                  <article key={card.noteId} className="card-row advanced-card-row">
                    <button type="button" className="card-row-main" onClick={() => void openEditorFor(card)}>
                      <span className="card-number">{page * PAGE_SIZE + index + 1}</span>
                      <span className="card-row-side">
                        <small>{modeLabel(card.mode)}</small>
                        <strong>{summarize(card.frontText, card.hasImages)}</strong>
                        <span className="card-tag-line">
                          {card.tags.slice(0, 5).map((tag) => <i key={tag}>{tag}</i>)}
                        </span>
                      </span>
                      <span className="card-row-divider" aria-hidden="true">→</span>
                      <span className="card-row-side">
                        <small>{card.generatedCardCount} generated</small>
                        <strong>{summarize(card.backText, card.hasImages)}</strong>
                        <span className="card-state-line">
                          {card.isNew ? <i className="status-badge new">New</i> : null}
                          {!card.isNew && card.dueAt ? <i className="status-badge due">{formatDue(card.dueAt)}</i> : null}
                        </span>
                      </span>
                    </button>
                    <div className="card-row-actions advanced-card-actions">
                      <button type="button" onClick={() => void openEditorFor(card)}>Edit</button>
                      <button type="button" onClick={() => setMoveCopyCard(card)}>Move/copy</button>
                      <button type="button" className="danger-text" onClick={() => void handleDeleteCard(card)}>Delete</button>
                    </div>
                  </article>
                ))}
              </div>
            ) : null}

            {totalCards > PAGE_SIZE ? (
              <nav className="pagination-controls" aria-label="Card pages">
                <button type="button" className="secondary-button" disabled={page === 0} onClick={() => setPage((value) => Math.max(0, value - 1))}>Previous</button>
                <span>Page {page + 1} of {pageCount}</span>
                <button type="button" className="secondary-button" disabled={page + 1 >= pageCount} onClick={() => setPage((value) => value + 1)}>Next</button>
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

      {moveCopyCard && selectedDeck ? (
        <MoveCopyModal
          card={moveCopyCard}
          currentDeck={selectedDeck}
          decks={decks}
          onClose={() => setMoveCopyCard(null)}
          onDone={async (operation, destinationName) => {
            setMoveCopyCard(null);
            showToast(
              "success",
              `${operation === "move" ? "Moved" : "Copied"} card to “${destinationName}”.`,
            );
            await refreshSelectedDeckAfterMutation();
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
    if (savingRef.current || name.trim() === "") return;
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
    <Modal title={mode === "create" ? "Create a deck" : "Edit deck"} onClose={() => { if (!isSaving) onClose(); }} width="small">
      <form className="form-stack" onSubmit={(event) => void submit(event)}>
        <label>
          <span>Deck name</span>
          <input autoFocus disabled={isSaving} value={name} maxLength={200} onChange={(event) => setName(event.currentTarget.value)} placeholder="NCLEX Pharmacology" />
        </label>
        <label>
          <span>Description <small>optional</small></span>
          <textarea value={description} disabled={isSaving} maxLength={2000} rows={4} onChange={(event) => setDescription(event.currentTarget.value)} placeholder="What this deck covers" />
        </label>
        {errorMessage ? <p className="field-error" role="alert">{errorMessage}</p> : null}
        <div className="modal-actions">
          <button type="button" className="secondary-button" disabled={isSaving} onClick={onClose}>Cancel</button>
          <button type="submit" className="primary-button" disabled={isSaving || name.trim() === ""}>{isSaving ? "Saving…" : mode === "create" ? "Create deck" : "Save changes"}</button>
        </div>
      </form>
    </Modal>
  );
}

interface MoveCopyModalProps {
  card: BrowserCard;
  currentDeck: Deck;
  decks: Deck[];
  onClose: () => void;
  onDone: (operation: "move" | "copy", destinationName: string) => void | Promise<void>;
}

function MoveCopyModal({
  card,
  currentDeck,
  decks,
  onClose,
  onDone,
}: MoveCopyModalProps) {
  const destinations = decks.filter((deck) => deck.id !== currentDeck.id);
  const [operation, setOperation] = useState<"move" | "copy">("move");
  const [destinationId, setDestinationId] = useState(destinations[0]?.id ?? "");
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  async function perform(destination: Deck, allowDuplicate: boolean): Promise<void> {
    await moveOrCopyNote(card.noteId, destination.id, operation, allowDuplicate);
    await onDone(operation, destination.name);
  }

  async function save(): Promise<void> {
    const destination = destinations.find((deck) => deck.id === destinationId);
    if (!destination || isSaving) return;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      try {
        await perform(destination, false);
      } catch (error: unknown) {
        if (getErrorCode(error) !== "duplicate_card") throw error;
        const intentional = window.confirm(
          `${getErrorMessage(error)}\n\nContinue with the ${operation} anyway?`,
        );
        if (!intentional) return;
        await perform(destination, true);
      }
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <Modal
      title="Move or copy card"
      description={summarize(card.frontText, card.hasImages)}
      onClose={() => { if (!isSaving) onClose(); }}
      width="small"
    >
      <div className="form-stack">
        {destinations.length === 0 ? (
          <div className="inline-alert info-alert">
            Create another deck before moving or copying this card.
          </div>
        ) : null}
        {errorMessage ? <div className="inline-alert error-alert" role="alert">{errorMessage}</div> : null}
        <fieldset className="inline-radio-group">
          <legend>Operation</legend>
          <label>
            <input
              type="radio"
              name="move-copy-operation"
              checked={operation === "move"}
              onChange={() => setOperation("move")}
            />
            Move
          </label>
          <label>
            <input
              type="radio"
              name="move-copy-operation"
              checked={operation === "copy"}
              onChange={() => setOperation("copy")}
            />
            Copy
          </label>
        </fieldset>
        <label>
          <span>Destination deck</span>
          <select
            value={destinationId}
            disabled={isSaving || destinations.length === 0}
            onChange={(event) => setDestinationId(event.currentTarget.value)}
          >
            {destinations.map((deck) => (
              <option key={deck.id} value={deck.id}>{deck.name}</option>
            ))}
          </select>
        </label>
        <p className="field-help">
          Moving preserves the generated cards and their review schedules. Copying creates fresh New cards in the destination.
        </p>
        <div className="modal-actions">
          <button type="button" className="secondary-button" disabled={isSaving} onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="primary-button"
            disabled={isSaving || destinationId === ""}
            onClick={() => void save()}
          >
            {isSaving ? "Saving…" : operation === "move" ? "Move card" : "Copy card"}
          </button>
        </div>
      </div>
    </Modal>
  );
}

function safeFilename(name: string): string {
  const cleaned = name.trim().replace(/[\\/:*?"<>|]+/gu, "-").replace(/\s+/gu, " ").slice(0, 120);
  return cleaned || "deck";
}

function summarize(text: string, hasImage: boolean): string {
  const normalized = text.replace(/\s+/gu, " ").trim();
  if (normalized === "") return hasImage ? "Image content" : "Empty";
  return normalized.length > 150 ? `${normalized.slice(0, 147)}…` : normalized;
}

function modeLabel(mode: NoteMode): string {
  switch (mode) {
    case "basic": return "Basic";
    case "reverse": return "Reverse";
    case "fill_blank": return "Fill in the blank";
    case "template": return "Template";
  }
}

function formatDue(timestamp: number): string {
  const delta = timestamp - Date.now();
  if (delta <= 0) return "Due";
  const days = Math.ceil(delta / 86_400_000);
  if (days <= 1) return "Tomorrow";
  if (days < 14) return `In ${days} days`;
  return new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(new Date(timestamp));
}
