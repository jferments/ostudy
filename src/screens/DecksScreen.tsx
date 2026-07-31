import { useEffect, useMemo, useRef, useState } from "react";
import type { FormEvent } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

import { Modal } from "../components/Modal";
import {
  createDeck,
  deleteCard,
  deleteDeck,
  exportDeck,
  getDeck,
  getErrorMessage,
  importDeck,
  listCards,
  updateDeck,
  type AppSettings,
  type CreateDeckInput,
  type Deck,
  type Flashcard,
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
  const [cards, setCards] = useState<Flashcard[]>([]);
  const [editingCard, setEditingCard] = useState<Flashcard | null>(null);
  const [deckDialog, setDeckDialog] = useState<"create" | "edit" | null>(null);
  const [isLoadingDeck, setIsLoadingDeck] = useState(false);
  const loadingDeckRef = useRef(false);
  const [isTransferring, setIsTransferring] = useState(false);
  const transferringRef = useRef(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  const selectedDeckId = selectedDeck?.id ?? null;

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

  const visibleCards = useMemo(() => {
    const normalized = search.trim().toLocaleLowerCase();
    if (normalized === "") return cards;
    return cards.filter((card) =>
      `${card.frontText} ${card.backText}`.toLocaleLowerCase().includes(normalized),
    );
  }, [cards, search]);

  async function openDeck(deckId: string): Promise<void> {
    if (loadingDeckRef.current) return;
    loadingDeckRef.current = true;
    setIsLoadingDeck(true);
    setErrorMessage(null);
    try {
      const [deck, loadedCards] = await Promise.all([getDeck(deckId), listCards(deckId)]);
      setSelectedDeck(deck);
      setCards(loadedCards);
      setEditingCard(null);
      setSearch("");
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
      const [deck, loadedCards] = await Promise.all([getDeck(deckId), listCards(deckId)]);
      setSelectedDeck(deck);
      setCards(loadedCards);
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
    setCards([]);
    setEditingCard(null);
    setSearch("");
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
        setCards([]);
        setEditingCard(null);
        setView("library");
      }
      showToast("success", `Deleted “${deck.name}”.`);
      void refreshDeckListAfterMutation();
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function handleDeleteCard(card: Flashcard): Promise<void> {
    if (
      settings.confirmDeletions &&
      !window.confirm(`Delete this flashcard?\n\n${card.frontText || "Image card"}`)
    ) {
      return;
    }
    try {
      await deleteCard(card.noteId);
      setCards((current) => current.filter((item) => item.noteId !== card.noteId));
      setSelectedDeck((current) =>
        current
          ? { ...current, cardCount: Math.max(0, current.cardCount - 1) }
          : current,
      );
      showToast("success", "Flashcard deleted.");
      if (selectedDeck) void refreshSelectedDeckAfterMutation(selectedDeck.id);
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
        filters: [{ name: "Study App deck", extensions: ["studydeck"] }],
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
        title: "Import a Study App deck",
        multiple: false,
        directory: false,
        filters: [{ name: "Study App deck", extensions: ["studydeck"] }],
      });
      if (typeof selected !== "string") return;

      const result = await importDeck(selected);
      setSelectedDeck(result.deck);
      setCards([]);
      setEditingCard(null);
      setSearch("");
      setView("detail");
      showToast(
        "success",
        `Imported “${result.deck.name}” with ${result.cardCount} ${result.cardCount === 1 ? "card" : "cards"}.`,
      );

      try {
        setCards(await listCards(result.deck.id));
      } catch (error: unknown) {
        showToast(
          "info",
          `The deck was imported, but its cards could not be displayed yet: ${getErrorMessage(error)}`,
        );
      }
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
        key={editingCard?.noteId ?? "new-card"}
        deck={selectedDeck}
        card={editingCard}
        onDirtyChange={onDirtyChange}
        onCancel={() => {
          onDirtyChange(false);
          setEditingCard(null);
          setView("detail");
        }}
        onSaved={(saved, continueAdding) => {
          const wasEditing = editingCard !== null;
          setCards((current) => {
            const existing = current.findIndex((item) => item.noteId === saved.noteId);
            if (existing < 0) return [...current, saved];
            const next = [...current];
            next[existing] = saved;
            return next;
          });
          if (!wasEditing) {
            setSelectedDeck((current) =>
              current
                ? {
                    ...current,
                    cardCount: current.cardCount + 1,
                    dueCount: current.dueCount + 1,
                  }
                : current,
            );
          }
          showToast("success", wasEditing ? "Flashcard updated." : "Flashcard saved.");
          if (!continueAdding) {
            onDirtyChange(false);
            setEditingCard(null);
            setView("detail");
          }
          void refreshSelectedDeckAfterMutation(selectedDeck.id);
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
              <button type="button" className="primary-button" disabled={isTransferring} onClick={() => setDeckDialog("create")}>
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
                <button type="button" className="primary-button" disabled={isTransferring} onClick={() => setDeckDialog("create")}>
                  Create deck
                </button>
                <button type="button" className="secondary-button" disabled={isTransferring} onClick={() => void handleImport()}>
                  Import deck
                </button>
              </div>
            </section>
          ) : (
            <section className="deck-grid" aria-label="Deck library">
              {decks.map((deck) => (
                <article key={deck.id} className="deck-card">
                  <button type="button" className="deck-card-main" onClick={() => void openDeck(deck.id)}>
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
                    <button type="button" onClick={() => onPracticeDeck(deck.id)} disabled={deck.dueCount === 0} title={deck.dueCount === 0 ? "No cards are due" : undefined}>
                      Practice
                    </button>
                    <button type="button" disabled={isTransferring} onClick={() => void handleExport(deck)}>
                      Export
                    </button>
                    <button type="button" className="danger-text" onClick={() => void handleDeleteDeck(deck)}>
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
              <button type="button" className="secondary-button" onClick={() => onPracticeDeck(selectedDeck.id)} disabled={selectedDeck.dueCount === 0} title={selectedDeck.dueCount === 0 ? "No cards are due" : undefined}>
                Practice
              </button>
              <button type="button" className="secondary-button" disabled={isTransferring} onClick={() => void handleExport(selectedDeck)}>
                Export
              </button>
              <button
                type="button"
                className="primary-button"
                onClick={() => {
                  setEditingCard(null);
                  setView("editor");
                }}
              >
                Add flashcard
              </button>
            </div>
          </header>

          {cards.length === 0 ? (
            <section className="compact-empty-state large-empty-state">
              <div className="empty-icon" aria-hidden="true">+</div>
              <h2>This deck is empty</h2>
              <button
                type="button"
                className="primary-button"
                onClick={() => {
                  setEditingCard(null);
                  setView("editor");
                }}
              >
                Add the first flashcard
              </button>
            </section>
          ) : (
            <section className="card-browser-panel">
              <div className="card-browser-toolbar">
                <label className="search-field">
                  <span>Search this deck</span>
                  <input
                    type="search"
                    value={search}
                    onChange={(event) => setSearch(event.currentTarget.value)}
                    placeholder="Search questions and answers"
                  />
                </label>
                <span className="card-count-summary">
                  {visibleCards.length === cards.length
                    ? `${cards.length} ${cards.length === 1 ? "flashcard" : "flashcards"}`
                    : `${visibleCards.length} of ${cards.length} flashcards`}
                  {selectedDeck.dueCount > 0 ? ` · ${selectedDeck.dueCount} due` : ""}
                </span>
              </div>

              {visibleCards.length === 0 ? (
                <div className="compact-empty-state">
                  <h3>No matching flashcards</h3>
                  <p>Try a different word or phrase.</p>
                </div>
              ) : (
                <div className="card-list">
                  {visibleCards.map((card, index) => (
                    <article key={card.noteId} className="card-row">
                      <button
                        type="button"
                        className="card-row-main"
                        onClick={() => {
                          setEditingCard(card);
                          setView("editor");
                        }}
                      >
                        <span className="card-number">{index + 1}</span>
                        <span className="card-row-side">
                          <small>Question</small>
                          <strong>{summarize(card.frontText, card.assets.length > 0)}</strong>
                        </span>
                        <span className="card-row-divider" aria-hidden="true">→</span>
                        <span className="card-row-side">
                          <small>Answer</small>
                          <strong>{summarize(card.backText, card.assets.length > 0)}</strong>
                        </span>
                      </button>
                      <div className="card-row-actions">
                        <span>{card.assets.length > 0 ? `${card.assets.length} image${card.assets.length === 1 ? "" : "s"}` : "Text"}</span>
                        <button
                          type="button"
                          onClick={() => {
                            setEditingCard(card);
                            setView("editor");
                          }}
                        >
                          Edit
                        </button>
                        <button type="button" className="danger-text" onClick={() => void handleDeleteCard(card)}>
                          Delete
                        </button>
                      </div>
                    </article>
                  ))}
                </div>
              )}
            </section>
          )}
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
          <button type="button" className="secondary-button" disabled={isSaving} onClick={onClose}>
            Cancel
          </button>
          <button type="submit" className="primary-button" disabled={isSaving || name.trim() === ""}>
            {isSaving ? "Saving…" : mode === "create" ? "Create deck" : "Save changes"}
          </button>
        </div>
      </form>
    </Modal>
  );
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

