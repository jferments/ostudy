import { useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";

import { DecksController } from "../controllers/decksController";

import { Modal } from "../components/Modal";
import type { ShowToast } from "../components/Toast";
import {
  type CardNote,
} from "../ipc/cards";
import {
  createOperationId,
  getErrorCode,
  getErrorMessage,
} from "../ipc/common";
import {
  type CreateDeckInput,
  type DeckCardSummary,
  type DeckSummary,
} from "../ipc/decks";
import {
  type AppSettings,
} from "../ipc/settings";
import {
  type CardTemplate,
  type GenerateTemplateCardsResult,
} from "../ipc/templates";
import { openFile, saveFile } from "../platform/files";
import { useMediaQuery } from "../ui/hooks/useMediaQuery";
import { useConfirm } from "../ui/overlays/overlayContext";
import { CardEditorScreen } from "./CardEditorScreen";

interface DecksScreenProps {
  decks: DeckSummary[];
  settings: AppSettings;
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeDeck: (deckId: string) => void | Promise<void>;
  onManageTemplates?: () => void | Promise<void>;
  onBulkAdd?: () => void;
  onViewChange?: (view: DeckView) => void;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

type DeckView = "library" | "detail" | "editor";
const DECK_CARD_PAGE_SIZE = 50;

export function DecksScreen({
  decks,
  settings,
  refreshDecks,
  onPracticeDeck,
  onManageTemplates,
  onBulkAdd,
  onViewChange,
  onDirtyChange,
  showToast,
}: DecksScreenProps) {
  const [view, setView] = useState<DeckView>("library");
  const [selectedDeck, setSelectedDeck] = useState<DeckSummary | null>(null);
  const [cards, setCards] = useState<DeckCardSummary[]>([]);
  const [cardTotalCount, setCardTotalCount] = useState(0);
  const [hasMoreCards, setHasMoreCards] = useState(false);
  const [editingCard, setEditingCard] = useState<CardNote | null>(null);
  const [deckDialog, setDeckDialog] = useState<"create" | "edit" | null>(null);
  const [isLoadingDeck, setIsLoadingDeck] = useState(false);
  const [isLoadingCards, setIsLoadingCards] = useState(false);
  const loadingDeckRef = useRef(false);
  const [isTransferring, setIsTransferring] = useState(false);
  const transferringRef = useRef(false);
  const pendingImportRef = useRef<{ sourcePath: string; operationId: string } | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [searchDialogOpen, setSearchDialogOpen] = useState(false);
  const [showArchived, setShowArchived] = useState(false);
  const [cardSort, setCardSort] = useState<
    "created_desc" | "practiced_desc" | "practiced_asc"
  >("created_desc");
  const controllerRef = useRef<DecksController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new DecksController();
  const controller = controllerRef.current;
  const confirm = useConfirm();
  const compact = useMediaQuery("(max-width: 760px)");

  const selectedDeckId = selectedDeck?.id ?? null;
  const activeDecks = decks.filter((deck) => deck.archivedAt === null);
  const archivedDecks = decks.filter((deck) => deck.archivedAt !== null);

  useEffect(() => () => controller.invalidate(), [controller]);
  useEffect(() => onViewChange?.(view), [onViewChange, view]);
  useEffect(() => {
    if (!compact) setSearchDialogOpen(false);
  }, [compact]);

  useEffect(() => {
    if (!selectedDeckId) return;
    const updated = decks.find((deck) => deck.id === selectedDeckId);
    if (!updated) return;
    setSelectedDeck((current) => {
      if (!current || current.id !== updated.id) return current;
      if (
        current.name === updated.name &&
        current.description === updated.description &&
        current.sourceNoteCount === updated.sourceNoteCount &&
        current.generatedCardCount === updated.generatedCardCount &&
        current.studyableDueCount === updated.studyableDueCount &&
        current.archivedAt === updated.archivedAt &&
        current.updatedAt === updated.updatedAt &&
        current.revision === updated.revision
      ) {
        return current;
      }
      return updated;
    });
  }, [decks, selectedDeckId]);

  useEffect(() => {
    if (view !== "detail" || !selectedDeckId) return;
    const timer = window.setTimeout(() => {
      setIsLoadingCards(true);
      void controller.cards({
        deckId: selectedDeckId,
        query: search,
        sort: cardSort,
        limit: DECK_CARD_PAGE_SIZE,
        offset: 0,
      })
        .then((page) => {
          if (!page) return;
          setCards(page.cards);
          setCardTotalCount(page.totalCount);
          setHasMoreCards(page.hasMore);
          setErrorMessage(null);
        })
        .catch((error: unknown) => {
          setCards([]);
          setCardTotalCount(0);
          setHasMoreCards(false);
          setErrorMessage(getErrorMessage(error));
        })
        .finally(() => setIsLoadingCards(false));
    }, 180);
    return () => window.clearTimeout(timer);
  }, [cardSort, controller, search, selectedDeckId, view]);

  async function openDeck(deckId: string): Promise<void> {
    if (loadingDeckRef.current) return;
    loadingDeckRef.current = true;
    setIsLoadingDeck(true);
    setErrorMessage(null);
    try {
      const deck = await controller.deck(deckId);
      if (!deck) return;
      setSelectedDeck(deck);
      setCards([]);
      setCardTotalCount(0);
      setHasMoreCards(false);
      setEditingCard(null);
      setSearch("");
      setCardSort("created_desc");
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
      const deck = await controller.deck(deckId);
      if (!deck) return;
      setSelectedDeck(deck);
      const page = await controller.cards({
        deckId,
        query: search,
        sort: cardSort,
        limit: DECK_CARD_PAGE_SIZE,
        offset: 0,
      });
      if (page) {
        setCards(page.cards);
        setCardTotalCount(page.totalCount);
        setHasMoreCards(page.hasMore);
      }
    } catch (error: unknown) {
      showToast(
        "info",
        `The change was saved, but the open deck could not refresh: ${getErrorMessage(error)}`,
      );
    }
    await refreshDeckListAfterMutation();
  }

  async function loadMoreCards(): Promise<void> {
    if (!selectedDeck || isLoadingCards || !hasMoreCards) return;
    setIsLoadingCards(true);
    try {
      const page = await controller.cards({
        deckId: selectedDeck.id,
        query: search,
        sort: cardSort,
        limit: DECK_CARD_PAGE_SIZE,
        offset: cards.length,
      });
      if (!page) return;
      setCards((current) => [...current, ...page.cards]);
      setCardTotalCount(page.totalCount);
      setHasMoreCards(page.hasMore);
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      setIsLoadingCards(false);
    }
  }

  async function openCardEditor(summary: DeckCardSummary): Promise<void> {
    try {
      const card = await controller.note(summary.noteId);
      if (!card) return;
      setEditingCard(card);
      setView("editor");
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function handleDeckSaved(input: CreateDeckInput): Promise<void> {
    if (deckDialog === "edit" && selectedDeck) {
      const updated = await controller.update({
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

    const created = await controller.create(input);
    setSelectedDeck(created);
    setCards([]);
    setCardTotalCount(0);
    setHasMoreCards(false);
    setEditingCard(null);
    setSearch("");
    setView("detail");
    setDeckDialog(null);
    showToast("success", `Created “${created.name}”.`);
    void refreshDeckListAfterMutation();
  }

  async function handleArchiveDeck(deck: DeckSummary, archived: boolean): Promise<void> {
    try {
      const updated = await controller.setArchived({
        deckId: deck.id,
        revision: deck.revision,
        archived,
      });
      showToast(
        "success",
        archived
          ? `Archived “${deck.name}”; all generated cards are suspended.`
          : `Restored “${deck.name}” to the active library.`,
      );
      if (selectedDeck?.id === deck.id) {
        setSelectedDeck(updated);
        if (archived) {
          setCards([]);
          setCardTotalCount(0);
          setHasMoreCards(false);
          setView("library");
        }
      }
      await refreshDeckListAfterMutation();
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function handleDeleteDeck(deck: DeckSummary): Promise<void> {
    if (settings.confirmDeletions) {
      const accepted = await confirm({
        title: `Delete “${deck.name}”?`,
        description: `This will delete ${deck.generatedCardCount} ${
          deck.generatedCardCount === 1 ? "card" : "cards"
        } and their review history after the Undo period.`,
        confirmLabel: "Delete deck",
        destructive: true,
      });
      if (!accepted) return;
    }
    try {
      const deletion = await controller.deleteDeck(deck.id);
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
            await controller.undo(deletion.undoToken);
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
    if (settings.confirmDeletions) {
      const accepted = await confirm({
        title: "Delete this flashcard?",
        description: `This card entry and its ${card.generatedCardCount} scheduled ${
          card.generatedCardCount === 1 ? "card" : "card variants"
        } will be deleted. Review history will also be removed after the Undo period. ${
          card.questionExcerpt || "Image card"
        }`,
        confirmLabel: "Delete flashcard",
        destructive: true,
      });
      if (!accepted) return;
    }
    try {
      const deletion = await controller.deleteNote(card.noteId);
      const deckId = selectedDeck?.id;
      showToast("success", "Flashcard deleted.", {
        label: "Undo",
        expiresAt: deletion.undoExpiresAt,
        run: async () => {
          try {
            await controller.undo(deletion.undoToken);
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

  async function handleExport(deck: DeckSummary): Promise<void> {
    if (transferringRef.current) return;
    transferringRef.current = true;
    setIsTransferring(true);
    try {
      const destination = await saveFile({
        title: "Export deck",
        defaultPath: `${safeFilename(deck.name)}.studydeck`,
        filters: [{ name: "ostudy deck", extensions: ["studydeck"] }],
      });
      if (typeof destination !== "string") return;
      const result = await controller.export(deck.id, destination);
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

  async function showImportedDeck(
    deckId: string,
    successMessage: string,
    committedDeck?: DeckSummary,
  ): Promise<void> {
    showToast("success", successMessage);
    setCards([]);
    setCardTotalCount(0);
    setHasMoreCards(false);
    setEditingCard(null);
    setSearch("");
    if (committedDeck) {
      setSelectedDeck(committedDeck);
      setView("detail");
    }

    try {
      const deck = committedDeck ?? (await controller.deck(deckId));
      if (!deck) return;
      setSelectedDeck(deck);
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
      const selected = await openFile({
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
          result = await controller.importStudydeck(selected, pending.operationId);
          pendingImportRef.current = null;
        } catch (error: unknown) {
          if (getErrorCode(error) !== null) pendingImportRef.current = null;
          throw error;
        }
        await showImportedDeck(
          result.deck.id,
          `Imported “${result.deck.name}” with ${result.cardCount} ${
            result.cardCount === 1 ? "card" : "cards"
          }.`,
          result.deck,
        );
        return;
      }

      if (normalizedPath.endsWith(".json")) {
        const result = await controller.importGenerated(selected);
        const duplicateMessage =
          result.skippedDuplicateCount > 0
            ? ` ${result.skippedDuplicateCount} exact ${
                result.skippedDuplicateCount === 1 ? "duplicate was" : "duplicates were"
              } skipped.`
            : "";
        await showImportedDeck(
          result.deckId,
          `Imported ${result.createdNoteCount} ${
            result.createdNoteCount === 1 ? "card entry" : "card entries"
          } as ${result.createdCardCount} ${
            result.createdCardCount === 1 ? "scheduled card" : "scheduled cards"
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
              {onManageTemplates ? (
                <button
                  type="button"
                  className="secondary-button"
                  onClick={() => void onManageTemplates()}
                >
                  Manage templates
                </button>
              ) : null}
              {onBulkAdd ? (
                <button
                  type="button"
                  className="secondary-button"
                  disabled={activeDecks.length === 0}
                  onClick={onBulkAdd}
                >
                  Bulk add
                </button>
              ) : null}
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

          {activeDecks.length === 0 ? (
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
              {activeDecks.map((deck) => {
                const archived = deck.archivedAt !== null;
                return (
                  <article key={deck.id} className={archived ? "deck-card archived" : "deck-card"}>
                    <button
                      type="button"
                      className="deck-card-main"
                      onClick={() => void openDeck(deck.id)}
                      disabled={archived}
                    >
                      <span className="deck-card-mark" aria-hidden="true">
                        ▤
                      </span>
                      <span className="deck-card-copy">
                        <strong>{deck.name}</strong>
                        {archived ? <span>Archived · cards suspended</span> : null}
                        {!archived && deck.description ? <span>{deck.description}</span> : null}
                      </span>
                      <span className="deck-card-count">
                        <strong>{deck.generatedCardCount}</strong>
                        <small>
                          {deck.generatedCardCount === 1 ? "generated card" : "generated cards"}
                        </small>
                        {!archived && deck.studyableDueCount > 0 ? (
                          <small>{deck.studyableDueCount} ready now</small>
                        ) : null}
                      </span>
                    </button>
                    <div className="deck-card-actions">
                      {!archived ? (
                        <button
                          type="button"
                          onClick={() => void onPracticeDeck(deck.id)}
                          disabled={deck.generatedCardCount === 0}
                          title={practiceTitle(deck)}
                        >
                          Practice
                        </button>
                      ) : (
                        <button
                          type="button"
                          onClick={() => void handleArchiveDeck(deck, false)}
                        >
                          Unarchive
                        </button>
                      )}
                      {!archived ? (
                        <button
                          type="button"
                          onClick={() => void handleArchiveDeck(deck, true)}
                        >
                          Archive
                        </button>
                      ) : null}
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
                );
              })}
            </section>
          )}

          {archivedDecks.length > 0 ? (
            <section className="archived-decks-section" aria-labelledby="archived-decks-heading">
              <button
                type="button"
                className="archived-decks-toggle"
                aria-expanded={showArchived}
                onClick={() => setShowArchived((current) => !current)}
              >
                <span>
                  <strong id="archived-decks-heading">Archived decks</strong>
                  <small>{archivedDecks.length} stored with cards suspended</small>
                </span>
                <span aria-hidden="true">{showArchived ? "▴" : "▾"}</span>
              </button>
              {showArchived ? (
                <div className="deck-grid archived-deck-grid">
                  {archivedDecks.map((deck) => (
                    <article key={deck.id} className="deck-card archived">
                      <div className="deck-card-main">
                        <span className="deck-card-mark" aria-hidden="true">
                          ▤
                        </span>
                        <span className="deck-card-copy">
                          <strong>{deck.name}</strong>
                          <span>Archived · cards suspended</span>
                        </span>
                        <span className="deck-card-count">
                          <strong>{deck.generatedCardCount}</strong>
                          <small>{deck.generatedCardCount === 1 ? "card" : "cards"}</small>
                        </span>
                      </div>
                      <div className="deck-card-actions">
                        <button type="button" onClick={() => void handleArchiveDeck(deck, false)}>
                          Unarchive
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
                </div>
              ) : null}
            </section>
          ) : null}
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
                onClick={() => void onPracticeDeck(selectedDeck.id)}
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
                className="secondary-button"
                onClick={() => void handleArchiveDeck(selectedDeck, true)}
              >
                Archive
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

          {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}

          <section className="card-browser-panel">
            <div className="card-browser-toolbar">
              {compact ? (
                <button
                  type="button"
                  className="deck-search-trigger"
                  aria-label="Search this deck"
                  title={search ? `Search: ${search}` : "Search this deck"}
                  onClick={() => setSearchDialogOpen(true)}
                >
                  <span aria-hidden="true">⌕</span>
                  <span className="sr-only">Search this deck</span>
                </button>
              ) : (
                <label className="search-field">
                  <span>Search this deck</span>
                  <input
                    type="search"
                    value={search}
                    onChange={(event) => setSearch(event.currentTarget.value)}
                    placeholder="Search questions and answers"
                  />
                </label>
              )}
              <label>
                <span>Sort</span>
                <select
                  value={cardSort}
                  onChange={(event) =>
                    setCardSort(
                      event.currentTarget.value as
                        | "created_desc"
                        | "practiced_desc"
                        | "practiced_asc",
                    )
                  }
                >
                  <option value="created_desc">Time created</option>
                  <option value="practiced_desc">Most recently practiced</option>
                  <option value="practiced_asc">Least recently practiced</option>
                </select>
              </label>
              <span className="card-count-summary">
                {cardTotalCount} {cardTotalCount === 1 ? "Card" : "Cards"}
                {selectedDeck.studyableDueCount > 0
                  ? ` · ${selectedDeck.studyableDueCount} ready now`
                  : ""}
              </span>
            </div>

            {isLoadingCards && cards.length === 0 ? (
              <div className="inline-progress">Loading cards…</div>
            ) : null}
            {!isLoadingCards && !errorMessage && selectedDeck.sourceNoteCount === 0 ? (
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
            ) : null}
            {!isLoadingCards &&
            !errorMessage &&
            selectedDeck.sourceNoteCount > 0 &&
            cards.length === 0 ? (
              <div className="compact-empty-state">
                <h3>No matching flashcards</h3>
                <p>Try a different word or phrase.</p>
              </div>
            ) : null}
            {cards.length > 0 ? (
              <div className="card-list">
                {cards.map((card, index) => (
                  <article key={card.noteId} className="card-row">
                    <button
                      type="button"
                      className="card-row-main"
                      onClick={() => void openCardEditor(card)}
                    >
                      <span className="card-number">{index + 1}</span>
                      <span className="card-row-side">
                        <small>Question</small>
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
                        {card.generatedCardCount}{" "}
                        {card.generatedCardCount === 1 ? "card" : "card variants"} ·{" "}
                        {card.imageCount > 0
                          ? `${card.imageCount} image${card.imageCount === 1 ? "" : "s"}`
                          : "Text"}
                      </span>
                      <button type="button" onClick={() => void openCardEditor(card)}>
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
                {hasMoreCards ? (
                  <button
                    type="button"
                    className="secondary-button"
                    disabled={isLoadingCards}
                    onClick={() => void loadMoreCards()}
                  >
                    {isLoadingCards ? "Loading…" : "Load more"}
                  </button>
                ) : null}
              </div>
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

      {searchDialogOpen ? (
        <Modal
          title="Search this deck"
          description={`Find cards in ${selectedDeck?.name ?? "this deck"}.`}
          onClose={() => setSearchDialogOpen(false)}
          width="small"
        >
          <div className="form-stack">
            <label>
              <span>Search questions and answers</span>
              <input
                data-autofocus="true"
                type="search"
                value={search}
                onChange={(event) => setSearch(event.currentTarget.value)}
                placeholder="Enter a word or phrase"
              />
            </label>
            <div className="modal-actions">
              {search ? (
                <button type="button" className="secondary-button" onClick={() => setSearch("")}>
                  Clear
                </button>
              ) : null}
              <button
                type="button"
                className="primary-button"
                onClick={() => setSearchDialogOpen(false)}
              >
                Done
              </button>
            </div>
          </div>
        </Modal>
      ) : null}
    </div>
  );
}

interface DeckFormModalProps {
  mode: "create" | "edit";
  deck: DeckSummary | null;
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
      busy={isSaving}
      dismissible={!isSaving}
    >
      <form className="form-stack" onSubmit={(event) => void submit(event)}>
        <label>
          <span>Deck name</span>
          <input
            data-autofocus="true"
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

function practiceTitle(deck: DeckSummary): string {
  if (deck.archivedAt !== null) return "Unarchive this deck before practicing";
  if (deck.generatedCardCount === 0) return "Add a flashcard before practicing this deck";
  if (deck.studyableDueCount === 0) return "Start an extra session; no cards are ready now";
  return `${deck.studyableDueCount} ${
    deck.studyableDueCount === 1 ? "card is" : "cards are"
  } ready now`;
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
