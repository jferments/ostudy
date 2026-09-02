import { useEffect, useRef, useState, type FormEvent } from "react";

import type { DeckOpenRequest } from "../App";
import { Modal } from "../components/Modal";
import { SourceCitationPanel } from "../components/SourceCitationPanel";
import type { ShowToast } from "../components/Toast";
import { DecksController } from "../controllers/decksController";
import { createOperationId, getErrorCode, getErrorMessage } from "../ipc/common";
import type { CreateDeckInput, DeckSummary } from "../ipc/decks";
import type { AppSettings } from "../ipc/settings";
import { registerPlatformBackHandler } from "../platform/backNavigation";
import { publishExport, selectExternalImport } from "../platform/files";
import { IconButton } from "../ui/icons/IconButton";
import { useConfirm } from "../ui/overlays/overlayContext";
import { ResponsiveActionGroup } from "../ui/patterns/ResponsiveActionGroup";
import {
  CardBrowserWorkspace,
  type CardBrowserView,
} from "./cards/CardBrowserWorkspace";
import { useCardPracticeLauncher } from "./cards/CardPracticeContext";
import { ownedClassNames, sharedClassMaps } from "../ui/styles/classOwnership";
import ownershipStyles from "./DecksScreen.module.css";

const ownedStyleMaps = [...sharedClassMaps, ownershipStyles] as const;

interface DecksScreenProps {
  decks: DeckSummary[];
  settings: AppSettings;
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeDeck: (deckId: string) => void | Promise<void>;
  openDeckRequest?: DeckOpenRequest | null;
  onOpenDeckRequestConsumed?: () => void;
  onManageTemplates?: () => void | Promise<void>;
  onViewChange?: (view: DeckView) => void;
  onDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

type DeckView = "library" | "detail" | "editor";

export function DecksScreen({
  decks,
  refreshDecks,
  onPracticeDeck,
  openDeckRequest = null,
  onOpenDeckRequestConsumed,
  onManageTemplates,
  onViewChange,
  onDirtyChange,
  showToast,
}: DecksScreenProps) {
  const [view, setView] = useState<DeckView>("library");
  const [selectedDeck, setSelectedDeck] = useState<DeckSummary | null>(null);
  const [deckDialog, setDeckDialog] = useState<"create" | "edit" | null>(null);
  const [isLoadingDeck, setIsLoadingDeck] = useState(false);
  const loadingDeckRef = useRef(false);
  const [isTransferring, setIsTransferring] = useState(false);
  const transferringRef = useRef(false);
  const pendingImportRef = useRef<{ stagedFileId: string; operationId: string } | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [showArchived, setShowArchived] = useState(false);
  const controllerRef = useRef<DecksController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new DecksController();
  const controller = controllerRef.current;
  const handledOpenRequestRef = useRef<number | null>(null);
  const confirm = useConfirm();
  const onPracticeCards = useCardPracticeLauncher();

  const selectedDeckId = selectedDeck?.id ?? null;
  const activeDecks = decks.filter((deck) => deck.archivedAt === null);
  const archivedDecks = decks.filter((deck) => deck.archivedAt !== null);

  useEffect(() => () => controller.invalidate(), [controller]);
  useEffect(() => onViewChange?.(view), [onViewChange, view]);
  useEffect(() => {
    if (view !== "detail") return;
    return registerPlatformBackHandler(() => {
      setView("library");
      return true;
    });
  }, [view]);

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

  async function openDeck(deckId: string): Promise<void> {
    if (loadingDeckRef.current) return;
    loadingDeckRef.current = true;
    setIsLoadingDeck(true);
    setErrorMessage(null);
    try {
      const deck = await controller.deck(deckId);
      if (!deck) return;
      setSelectedDeck(deck);
      setView("detail");
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      loadingDeckRef.current = false;
      setIsLoadingDeck(false);
    }
  }

  useEffect(() => {
    if (!openDeckRequest || handledOpenRequestRef.current === openDeckRequest.requestId) return;
    handledOpenRequestRef.current = openDeckRequest.requestId;
    void openDeck(openDeckRequest.deckId).finally(() => onOpenDeckRequestConsumed?.());
  }, [openDeckRequest?.requestId]);

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
        if (archived) setView("library");
      }
      await refreshDeckListAfterMutation();
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function handleDeleteDeck(deck: DeckSummary): Promise<void> {
    const accepted = await confirm({
      title: `Delete “${deck.name}”?`,
      description: `This will delete ${deck.generatedCardCount} ${
        deck.generatedCardCount === 1 ? "card" : "cards"
      } and their review history after the Undo period.`,
      confirmLabel: "Delete deck",
      destructive: true,
    });
    if (!accepted) return;
    try {
      const deletion = await controller.deleteDeck(deck.id);
      if (selectedDeck?.id === deck.id) {
        setSelectedDeck(null);
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

  async function handleExport(deck: DeckSummary): Promise<void> {
    if (transferringRef.current) return;
    transferringRef.current = true;
    setIsTransferring(true);
    try {
      const prepared = await controller.prepareExport(deck.id, createOperationId());
      const receipt = await publishExport(prepared.preparedExport.id);
      if (!receipt) return;
      showToast(
        "success",
        `Exported ${prepared.cardCount} ${prepared.cardCount === 1 ? "card" : "cards"}.`,
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
      const selected = await selectExternalImport("study_content");
      if (!selected) return;

      if (selected.detectedKind === "studydeck") {
        const pending =
          pendingImportRef.current?.stagedFileId === selected.id
            ? pendingImportRef.current
            : { stagedFileId: selected.id, operationId: createOperationId() };
        pendingImportRef.current = pending;
        let result;
        try {
          result = await controller.importStudydeck(selected.id, pending.operationId);
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

      if (selected.detectedKind === "generated_json") {
        const result = await controller.importGenerated(selected.id);
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

      throw new Error("Select an ostudy deck or generated-card file.");
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      transferringRef.current = false;
      setIsTransferring(false);
    }
  }

  function handleCardBrowserView(next: CardBrowserView): void {
    setView(next === "editor" ? "editor" : "detail");
  }

  const showingDeck = (view === "detail" || view === "editor") && selectedDeck !== null;

  return (
    <div className={ownedClassNames(ownedStyleMaps, view === "editor" ? "deck-card-browser-editor-host" : "screen decks-screen")}>
      {view === "library" ? (
        <>
          <header className={ownedClassNames(ownedStyleMaps, "screen-header compact-screen-header")}>
            <div>
              <h1>Decks</h1>
            </div>
            <div className={ownedClassNames(ownedStyleMaps, "header-actions")}>
              {onManageTemplates ? (
                <button
                  type="button"
                  className={ownedClassNames(ownedStyleMaps, "secondary-button")}
                  onClick={() => void onManageTemplates()}
                >
                  Manage templates
                </button>
              ) : null}
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "secondary-button")}
                disabled={isTransferring}
                onClick={() => void handleImport()}
              >
                Import deck
              </button>
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "primary-button")}
                disabled={isTransferring}
                onClick={() => setDeckDialog("create")}
              >
                New deck
              </button>
            </div>
          </header>

          {errorMessage ? <div className={ownedClassNames(ownedStyleMaps, "inline-alert error-alert")}>{errorMessage}</div> : null}
          {isLoadingDeck ? <div className={ownedClassNames(ownedStyleMaps, "inline-progress")}>Opening deck…</div> : null}

          {activeDecks.length === 0 ? (
            <section className={ownedClassNames(ownedStyleMaps, "empty-library-card")}>
              <div className={ownedClassNames(ownedStyleMaps, "empty-icon")} aria-hidden="true">
                ?
              </div>
              <h2>Create your first deck</h2>
              <div className={ownedClassNames(ownedStyleMaps, "empty-actions")}>
                <button
                  type="button"
                  className={ownedClassNames(ownedStyleMaps, "primary-button")}
                  disabled={isTransferring}
                  onClick={() => setDeckDialog("create")}
                >
                  Create deck
                </button>
                <button
                  type="button"
                  className={ownedClassNames(ownedStyleMaps, "secondary-button")}
                  disabled={isTransferring}
                  onClick={() => void handleImport()}
                >
                  Import deck
                </button>
              </div>
            </section>
          ) : (
            <section className={ownedClassNames(ownedStyleMaps, "deck-grid")} aria-label="Deck library">
              {activeDecks.map((deck) => {
                const archived = deck.archivedAt !== null;
                return (
                  <article key={deck.id} className={ownedClassNames(ownedStyleMaps, "deck-card")}>
                    <button
                      type="button"
                      className={ownedClassNames(ownedStyleMaps, "deck-card-main")}
                      onClick={() => void openDeck(deck.id)}
                      disabled={archived}
                    >
                      <span className={ownedClassNames(ownedStyleMaps, "deck-card-mark")} aria-hidden="true">
                        ▤
                      </span>
                      <span className={ownedClassNames(ownedStyleMaps, "deck-card-copy")}>
                        <strong>{deck.name}</strong>
                        {archived ? <span>Archived · cards suspended</span> : null}
                        {!archived && deck.description ? <span>{deck.description}</span> : null}
                      </span>
                      <span className={ownedClassNames(ownedStyleMaps, "deck-card-count")}>
                        <strong>{deck.generatedCardCount}</strong>
                        <small>
                          {deck.generatedCardCount === 1 ? "generated card" : "generated cards"}
                        </small>
                        {!archived && deck.studyableDueCount > 0 ? (
                          <small>{deck.studyableDueCount} ready now</small>
                        ) : null}
                      </span>
                    </button>
                    <div className={ownedClassNames(ownedStyleMaps, "deck-card-actions")}>
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
                        <button type="button" onClick={() => void handleArchiveDeck(deck, false)}>
                          Unarchive
                        </button>
                      )}
                      {!archived ? (
                        <button type="button" onClick={() => void handleArchiveDeck(deck, true)}>
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
                        className={ownedClassNames(ownedStyleMaps, "danger-text")}
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
            <section className={ownedClassNames(ownedStyleMaps, "archived-decks-section")} aria-labelledby="archived-decks-heading">
              <button
                type="button"
                className={ownedClassNames(ownedStyleMaps, "archived-decks-toggle")}
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
                <div className={ownedClassNames(ownedStyleMaps, "deck-grid archived-deck-grid")}>
                  {archivedDecks.map((deck) => (
                    <article key={deck.id} className={ownedClassNames(ownedStyleMaps, "deck-card")}>
                      <div className={ownedClassNames(ownedStyleMaps, "deck-card-main")}>
                        <span className={ownedClassNames(ownedStyleMaps, "deck-card-mark")} aria-hidden="true">
                          ▤
                        </span>
                        <span className={ownedClassNames(ownedStyleMaps, "deck-card-copy")}>
                          <strong>{deck.name}</strong>
                          <span>Archived · cards suspended</span>
                        </span>
                        <span className={ownedClassNames(ownedStyleMaps, "deck-card-count")}>
                          <strong>{deck.generatedCardCount}</strong>
                          <small>{deck.generatedCardCount === 1 ? "card" : "cards"}</small>
                        </span>
                      </div>
                      <div className={ownedClassNames(ownedStyleMaps, "deck-card-actions")}>
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
                          className={ownedClassNames(ownedStyleMaps, "danger-text")}
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

      {showingDeck && selectedDeck ? (
        <>
          {view === "detail" ? (
            <header className={ownedClassNames(ownedStyleMaps, "screen-header deck-detail-header")} key="deck-detail-header">
              <button type="button" className={ownedClassNames(ownedStyleMaps, "back-button")} onClick={() => setView("library")}>
                ← All decks
              </button>
              <div className={ownedClassNames(ownedStyleMaps, "deck-title-row")}>
                <div className={ownershipStyles["deck-title-line"]}>
                  <h1>{selectedDeck.name}</h1>
                  <IconButton
                    icon="edit"
                    label="Edit deck name and description"
                    tooltip="Edit deck"
                    className={ownershipStyles["deck-edit-button"]}
                    onClick={() => setDeckDialog("edit")}
                  />
                </div>
                {selectedDeck.description ? <p>{selectedDeck.description}</p> : null}
              </div>
              <ResponsiveActionGroup
                label={`${selectedDeck.name} actions`}
                className={ownershipStyles["deck-detail-actions"]}
                actions={[
                  {
                    id: "practice",
                    label: "Practice",
                    placement: "always-visible",
                    variant: "primary",
                    description: practiceTitle(selectedDeck),
                    disabled:
                      selectedDeck.archivedAt !== null ||
                      selectedDeck.generatedCardCount === 0,
                    onSelect: () => void onPracticeDeck(selectedDeck.id),
                  },
                  {
                    id: "export",
                    label: "Export",
                    placement: "wide-and-medium",
                    loading: isTransferring,
                    onSelect: () => void handleExport(selectedDeck),
                  },
                  {
                    id: selectedDeck.archivedAt === null ? "archive" : "restore",
                    label:
                      selectedDeck.archivedAt === null
                        ? `Archive ${selectedDeck.name}`
                        : `Restore ${selectedDeck.name}`,
                    placement: "wide-only",
                    ...(selectedDeck.archivedAt === null
                      ? { presentation: "icon" as const, icon: "archive" as const }
                      : {}),
                    onSelect: () =>
                      void handleArchiveDeck(
                        selectedDeck,
                        selectedDeck.archivedAt === null,
                      ),
                  },
                  {
                    id: "delete",
                    label: `Delete ${selectedDeck.name}`,
                    placement: "always-visible",
                    presentation: "icon",
                    icon: "delete",
                    description: "Delete deck",
                    destructive: true,
                    onSelect: () => void handleDeleteDeck(selectedDeck),
                  },
                ]}
              />
            </header>
          ) : null}

          {view === "detail" ? <SourceCitationPanel deckId={selectedDeck.id} /> : null}

          {view === "detail" && errorMessage ? (
            <div className={ownedClassNames(ownedStyleMaps, "inline-alert error-alert")}>{errorMessage}</div>
          ) : null}

          <CardBrowserWorkspace
            key={`deck-card-browser:${selectedDeck.id}`}
            decks={decks}
            fixedDeckId={selectedDeck.id}
            showFilters={false}
            showPageHeader={false}
            refreshDecks={refreshDecks}
            onPracticeCards={onPracticeCards}
            onDirtyChange={onDirtyChange}
            onViewChange={handleCardBrowserView}
            showToast={showToast}
          />
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
      <form className={ownedClassNames(ownedStyleMaps, "form-stack")} onSubmit={(event) => void submit(event)}>
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
        {errorMessage ? <p className={ownedClassNames(ownedStyleMaps, "field-error")}>{errorMessage}</p> : null}
        <div className={ownedClassNames(ownedStyleMaps, "modal-actions")}>
          <button
            type="button"
            className={ownedClassNames(ownedStyleMaps, "secondary-button")}
            disabled={isSaving}
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="submit"
            className={ownedClassNames(ownedStyleMaps, "primary-button")}
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
  return `Practice all ${deck.generatedCardCount} ${
    deck.generatedCardCount === 1 ? "card" : "cards"
  } in this deck`;
}
