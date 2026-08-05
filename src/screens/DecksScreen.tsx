import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";

import { Modal } from "../components/Modal";
import type { ShowToast } from "../components/Toast";
import { importGeneratedFlashcards } from "../generatedImport";
import {
  archiveDeck,
  createDeck,
  deleteCard,
  deleteDeck,
  createOperationId,
  exportDeck,
  getDeck,
  getOrganizedCard,
  getErrorCode,
  getErrorMessage,
  importDeck,
  listArchivedDecks,
  listDeckCards,
  undoDeletion,
  unarchiveDeck,
  updateDeck,
  type AppSettings,
  type CardTemplate,
  type CreateDeckInput,
  type Deck,
  type DeckCardSummary,
  type OrganizedCard,
  type GenerateTemplateCardsResult,
} from "../ipc";
import { CardEditorScreen } from "./CardEditorScreen";

interface DecksScreenProps {
  decks: Deck[];
  settings: AppSettings;
  refreshDecks: () => Promise<Deck[]>;
  onPracticeDeck: (deckId: string) => void;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
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
  const [cards, setCards] = useState<DeckCardSummary[]>([]);
  const [archivedDecks, setArchivedDecks] = useState<Deck[]>([]);
  const [cardTotal, setCardTotal] = useState(0);
  const [editingCard, setEditingCard] = useState<OrganizedCard | null>(null);
  const [deckDialog, setDeckDialog] = useState<"create" | "edit" | null>(null);
  const [isLoadingDeck, setIsLoadingDeck] = useState(false);
  const loadingDeckRef = useRef(false);
  const deckSearchRequestRef = useRef(0);
  const [isSearchingCards, setIsSearchingCards] = useState(false);
  const [isTransferring, setIsTransferring] = useState(false);
  const transferringRef = useRef(false);
  const pendingImportRef = useRef<{ sourcePath: string; operationId: string } | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [search, setSearch] = useState("");

  const selectedDeckId = selectedDeck?.id ?? null;

  useEffect(() => { void listArchivedDecks().then(setArchivedDecks).catch(() => setArchivedDecks([])); }, [decks]);

  useEffect(() => {
    if (!selectedDeckId) return;
    const updated = decks.find((deck) => deck.id === selectedDeckId);
    if (!updated) return;
    setSelectedDeck((current) => {
      if (!current || current.id !== updated.id) return current;
      if (
        current.name === updated.name &&
        current.description === updated.description &&
        current.generatedCardCount === updated.generatedCardCount &&
        current.studyableDueCount === updated.studyableDueCount &&
        current.updatedAt === updated.updatedAt &&
        current.revision === updated.revision
      ) {
        return current;
      }
      return updated;
    });
  }, [decks, selectedDeckId]);

  const visibleCards = cards;

  useEffect(() => {
    if (view !== "detail" || !selectedDeckId) return;
    const requestId = ++deckSearchRequestRef.current;
    const timer = window.setTimeout(() => {
      setIsSearchingCards(true);
      void listDeckCards(selectedDeckId, search, 50, 0)
        .then((page) => {
          if (deckSearchRequestRef.current !== requestId) return;
          setCards(page.cards);
          setCardTotal(page.totalCount);
        })
        .catch((error: unknown) => {
          if (deckSearchRequestRef.current === requestId) setErrorMessage(getErrorMessage(error));
        })
        .finally(() => {
          if (deckSearchRequestRef.current === requestId) setIsSearchingCards(false);
        });
    }, 180);
    return () => window.clearTimeout(timer);
  }, [search, selectedDeckId, view]);

  async function openDeck(deckId: string): Promise<void> {
    if (loadingDeckRef.current) return;
    loadingDeckRef.current = true;
    setIsLoadingDeck(true);
    setErrorMessage(null);
    try {
      const [deck, page] = await Promise.all([getDeck(deckId), listDeckCards(deckId, search, 50, 0)]);
      const loadedCards = page.cards;
      setCardTotal(page.totalCount);
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
      const [deck, page] = await Promise.all([getDeck(deckId), listDeckCards(deckId, search, 50, 0)]);
      const loadedCards = page.cards;
      setCardTotal(page.totalCount);
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

  async function handleArchiveDeck(deck: Deck): Promise<void> {
    try {
      await archiveDeck(deck.id);
      showToast("success", `Archived “${deck.name}”. Its cards are suspended.`);
      await refreshDeckListAfterMutation();
    } catch (error: unknown) { showToast("error", getErrorMessage(error)); }
  }

  async function handleUnarchiveDeck(deck: Deck): Promise<void> {
    try {
      await unarchiveDeck(deck.id);
      showToast("success", `Restored “${deck.name}”.`);
      await refreshDeckListAfterMutation();
    } catch (error: unknown) { showToast("error", getErrorMessage(error)); }
  }

  async function handleDeleteDeck(deck: Deck): Promise<void> {
    if (
      settings.confirmDeletions &&
      !window.confirm(
        `Delete “${deck.name}” and its ${deck.generatedCardCount} ${
          deck.generatedCardCount === 1 ? "card" : "cards"
        }?`,
      )
    ) {
      return;
    }
    try {
      const deletion = await deleteDeck(deck.id);
      if (selectedDeck?.id === deck.id) {
        setSelectedDeck(null);
        setCards([]);
        setEditingCard(null);
        setView("library");
      }
      showToast("success", `Deleted “${deck.name}”.`, {
        label: "Undo",
        expiresAt: deletion.undoExpiresAt,
        run: async () => {
          try {
            await undoDeletion(deletion.undoToken);
            await refreshDecks();
          } catch (error: unknown) {
            showToast("error", getErrorMessage(error));
            throw error;
          }
        },
      });
      void refreshDeckListAfterMutation();
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function handleDeleteCard(card: DeckCardSummary): Promise<void> {
    if (
      settings.confirmDeletions &&
      !window.confirm(
        `Delete this flashcard and every generated direction?\n\n${
          card.questionExcerpt || "Image card"
        }`,
      )
    ) {
      return;
    }
    try {
      const deletion = await deleteCard(card.noteId);
      const deckId = selectedDeck?.id;
      showToast("success", "Flashcard deleted.", {
        label: "Undo",
        expiresAt: deletion.undoExpiresAt,
        run: async () => {
          try {
            await undoDeletion(deletion.undoToken);
            if (deckId) await refreshSelectedDeckAfterMutation(deckId);
            else await refreshDecks();
          } catch (error: unknown) {
            showToast("error", getErrorMessage(error));
            throw error;
          }
        },
      });
      if (deckId) await refreshSelectedDeckAfterMutation(deckId);
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
        `Exported ${result.generatedCardCount} ${
          result.generatedCardCount === 1 ? "card" : "cards"
        }.`,
      );
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      transferringRef.current = false;
      setIsTransferring(false);
    }
  }

  async function showImportedDeck(
    deckId: string,
    successMessage: string,
    committedDeck?: Deck,
  ): Promise<void> {
    showToast("success", successMessage);
    setCards([]);
    setEditingCard(null);
    setSearch("");
    if (committedDeck) {
      setSelectedDeck(committedDeck);
      setView("detail");
    }

    try {
      const [deck, page] = await Promise.all([
        committedDeck ? Promise.resolve(committedDeck) : getDeck(deckId),
        listDeckCards(deckId, "", 50, 0),
      ]);
      setSelectedDeck(deck);
      setCards(page.cards);
      setCardTotal(page.totalCount);
      setView("detail");
    } catch (error: unknown) {
      if (!committedDeck) {
        setSelectedDeck(null);
        setView("library");
      }
      showToast(
        "info",
        `The deck was imported, but it could not be displayed yet: ${getErrorMessage(error)}`,
      );
    }
    await refreshDeckListAfterMutation();
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
        filters: [
          {
            name: "ostudy deck or generated JSON",
            extensions: ["studydeck", "json"],
          },
        ],
      });
      if (typeof selected !== "string") return;

      const normalizedPath = selected.toLocaleLowerCase();
      if (normalizedPath.endsWith(".studydeck")) {
        const pending =
          pendingImportRef.current?.sourcePath === selected
            ? pendingImportRef.current
            : { sourcePath: selected, operationId: createOperationId() };
        pendingImportRef.current = pending;
        let result;
        try {
          result = await importDeck(selected, pending.operationId);
          pendingImportRef.current = null;
        } catch (error: unknown) {
          if (getErrorCode(error) !== null) pendingImportRef.current = null;
          throw error;
        }
        await showImportedDeck(
          result.deck.id,
          `Imported “${result.deck.name}” with ${result.generatedCardCount} ${
            result.generatedCardCount === 1 ? "card" : "cards"
          }.`,
          result.deck,
        );
        return;
      }

      if (normalizedPath.endsWith(".json")) {
        const result = await importGeneratedFlashcards(selected);
        const duplicateMessage =
          result.skippedDuplicateCount > 0
            ? ` ${result.skippedDuplicateCount} exact ${
                result.skippedDuplicateCount === 1 ? "duplicate was" : "duplicates were"
              } skipped.`
            : "";
        await showImportedDeck(
          result.deckId,
          `Imported ${result.createdNoteCount} source ${
            result.createdNoteCount === 1 ? "note" : "notes"
          } as ${result.createdCardCount} scheduled ${
            result.createdCardCount === 1 ? "card" : "cards"
          }.${duplicateMessage}`,
        );
        return;
      }

      throw new Error("Select an ostudy .studydeck or generated .ostudy.json file.");
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
        decks={decks}
        card={editingCard}
        onDirtyChange={onDirtyChange}
        onCancel={() => {
          onDirtyChange(false);
          setEditingCard(null);
          setView("detail");
        }}
        onSaved={async (_saved, continueAdding) => {
          const wasEditing = editingCard !== null;
          onDirtyChange(false);
          if (!continueAdding) {
            setEditingCard(null);
            setView("detail");
          }
          showToast("success", wasEditing ? "Flashcard updated." : "Flashcard saved.");
          await refreshSelectedDeckAfterMutation(selectedDeck.id);
        }}
        onTemplateGenerated={async (
          result: GenerateTemplateCardsResult,
          continueAdding: boolean,
          template: CardTemplate,
        ) => {
          onDirtyChange(false);
          if (!continueAdding) {
            setEditingCard(null);
            setView("detail");
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
          {isSearchingCards ? <div className="inline-progress">Searching this deck…</div> : null}

          {decks.length === 0 ? (
            <section className="empty-library-card">
              <div className="empty-icon" aria-hidden="true">
                ?
              </div>
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
                    <span className="deck-card-mark" aria-hidden="true">
                      ▤
                    </span>
                    <span className="deck-card-copy">
                      <strong>{deck.name}</strong>
                      {deck.description ? <span>{deck.description}</span> : null}
                    </span>
                    <span className="deck-card-count">
                      <strong>{deck.generatedCardCount}</strong>
                      <small>{deck.generatedCardCount === 1 ? "card" : "cards"}</small>
                    </span>
                  </button>
                  <div className="deck-card-actions">
                    <button
                      type="button"
                      onClick={() => onPracticeDeck(deck.id)}
                      disabled={deck.generatedCardCount === 0}
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
                    <button type="button" onClick={() => void handleArchiveDeck(deck)}>Archive</button>
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
              <button
                type="button"
                className="back-button"
                onClick={() => setView("library")}
              >
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
                disabled={selectedDeck.generatedCardCount === 0}
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
              <div className="empty-icon" aria-hidden="true">
                +
              </div>
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
                  {search.trim() === ""
                    ? `${cardTotal} ${cardTotal === 1 ? "source note" : "source notes"}`
                    : `${visibleCards.length} matching source notes`}
                  {` · ${selectedDeck.generatedCardCount} generated ${
                    selectedDeck.generatedCardCount === 1 ? "card" : "cards"
                  }`}
                  {selectedDeck.studyableDueCount > 0 ? ` · ${selectedDeck.studyableDueCount} due` : ""}
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
                          void getOrganizedCard(card.noteId).then((loaded) => {
                            setEditingCard(loaded);
                            setView("editor");
                          }).catch((error: unknown) => showToast("error", getErrorMessage(error)));
                        }}
                      >
                        <span className="card-number">{index + 1}</span>
                        <span className="card-row-side">
                          <small>Question / source</small>
                          <strong>{summarize(card.questionExcerpt, card.imageCount > 0)}</strong>
                        </span>
                        <span className="card-row-divider" aria-hidden="true">
                          →
                        </span>
                        <span className="card-row-side">
                          <small>Answer</small>
                          <strong>{summarize(card.answerExcerpt, card.imageCount > 0)}</strong>
                        </span>
                      </button>
                      <div className="card-row-actions">
                        <span>
                          {card.imageCount > 0
                            ? `${card.imageCount} image${card.imageCount === 1 ? "" : "s"}`
                            : "Text"}
                        </span>
                        <button
                          type="button"
                          onClick={() => {
                            void getOrganizedCard(card.noteId).then((loaded) => {
                              setEditingCard(loaded);
                              setView("editor");
                            }).catch((error: unknown) => showToast("error", getErrorMessage(error)));
                          }}
                        >
                          Edit
                        </button>
                        <button
                          type="button"
                          className="danger-text"
                          onClick={() => void handleDeleteCard(card)}
                        >
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

      {view === "library" && archivedDecks.length > 0 ? (
        <section className="archived-decks-panel">
          <h2>Archived decks</h2>
          <p>Archived deck history remains in Analytics; its cards stay suspended.</p>
          {archivedDecks.map((deck) => (
            <article key={deck.id} className="deck-card archived">
              <strong>{deck.name}</strong>
              <span>{deck.generatedCardCount} cards</span>
              <button type="button" onClick={() => void handleUnarchiveDeck(deck)}>Unarchive</button>
              <button type="button" className="danger-text" onClick={() => void handleDeleteDeck(deck)}>Delete</button>
            </article>
          ))}
        </section>
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
          <span>
            Description <small>optional</small>
          </span>
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
  if (deck.generatedCardCount === 0) return "Add a flashcard before practicing this deck";
  if (deck.studyableDueCount === 0) return "Practice all cards; none are currently due";
  return `${deck.studyableDueCount} ${
    deck.studyableDueCount === 1 ? "card is" : "cards are"
  } due`;
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
