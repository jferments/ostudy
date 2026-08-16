import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type DragEvent,
  type FormEvent,
} from "react";

import type { DeckOpenRequest } from "../../App";
import type { ShowToast } from "../../components/Toast";
import { DecksController } from "../../controllers/decksController";
import { getErrorCode, getErrorMessage } from "../../ipc/common";
import type { DeckSummary } from "../../ipc/decks";
import {
  createDeckFolder,
  deleteDeckFolder,
  getDeckLibraryOrganization,
  moveDeckFolder,
  moveDeckInLibrary,
  updateDeckFolder,
  type DeckFolder,
  type DeckLibraryOrganization,
} from "../../ipc/deckFolders";
import { saveSettings, type AppSettings, type DeckViewDensity } from "../../ipc/settings";
import { Artwork } from "../../ui/artwork/Artwork";
import { HelpTrigger } from "../../ui/help/HelpProvider";
import { Icon } from "../../ui/icons/Icon";
import { Sheet } from "../../ui/overlays/Dialog";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { EmptyState, LoadingState } from "../../ui/primitives/EmptyState";
import { Progress } from "../../ui/primitives/Progress";
import { Surface } from "../../ui/primitives/Surface";
import { DecksScreen } from "../DecksScreen";
import styles from "./DeckLibraryWorkspace.module.css";

interface DeckLibraryWorkspaceProps {
  decks: DeckSummary[];
  settings: AppSettings;
  refreshDecks: () => Promise<DeckSummary[]>;
  onPracticeDeck: (deckId: string, origin: "library" | "detail") => void | Promise<void>;
  openDeckRequest: DeckOpenRequest | null;
  onOpenDeckRequestConsumed: () => void;
  onManageTemplates: () => void | Promise<void>;
  onCreateCards: () => void | Promise<void>;
  onCardDirtyChange: (dirty: boolean) => void;
  showToast: ShowToast;
}

interface FolderDraft {
  mode: "create" | "rename";
  folder: DeckFolder | null;
  name: string;
}

interface MoveDeckDraft {
  deck: DeckSummary;
  folderId: string | null;
}

export function DeckLibraryWorkspace({
  decks,
  settings,
  refreshDecks,
  onPracticeDeck,
  openDeckRequest,
  onOpenDeckRequestConsumed,
  onManageTemplates,
  onCreateCards,
  onCardDirtyChange,
  showToast,
}: DeckLibraryWorkspaceProps) {
  const [organization, setOrganization] = useState<DeckLibraryOrganization | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [folderDraft, setFolderDraft] = useState<FolderDraft | null>(null);
  const [moveDraft, setMoveDraft] = useState<MoveDeckDraft | null>(null);
  const [isMutating, setIsMutating] = useState(false);
  const [density, setDensity] = useState<DeckViewDensity>(
    settings.deckViewDensity ?? "comfortable",
  );
  const [announcement, setAnnouncement] = useState("");
  const [showArchived, setShowArchived] = useState(false);
  const [detailRequest, setDetailRequest] = useState<DeckOpenRequest | null>(null);
  const requestSequence = useRef(1);
  const detailOpenedRef = useRef(false);
  const organizationRef = useRef<DeckLibraryOrganization | null>(null);
  const organizationMutationRef = useRef(false);
  const lastDeckLifecycleSignatureRef = useRef<string | null>(null);
  const controllerRef = useRef<DecksController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new DecksController();
  const controller = controllerRef.current;
  const confirm = useConfirm();

  function applyOrganization(next: DeckLibraryOrganization): void {
    organizationRef.current = next;
    setOrganization(next);
  }

  useEffect(() => {
    let active = true;
    setIsLoading(true);
    void getDeckLibraryOrganization()
      .then((loaded) => {
        if (active) {
          applyOrganization(loaded);
          setErrorMessage(null);
        }
      })
      .catch((error: unknown) => {
        if (active) setErrorMessage(getErrorMessage(error));
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
      controller.invalidate();
    };
  }, [controller]);

  useEffect(() => {
    if (!openDeckRequest) return;
    detailOpenedRef.current = false;
    setFolderDraft(null);
    setMoveDraft(null);
    setDetailRequest(openDeckRequest);
    onOpenDeckRequestConsumed();
  }, [onOpenDeckRequestConsumed, openDeckRequest]);

  useEffect(() => {
    setDensity(settings.deckViewDensity ?? "comfortable");
  }, [settings.deckViewDensity]);

  const placementByDeck = useMemo(
    () => new Map((organization?.placements ?? []).map((placement) => [placement.deckId, placement])),
    [organization?.placements],
  );
  const activeDecks = useMemo(
    () => decks.filter((deck) => deck.archivedAt === null),
    [decks],
  );
  const archivedDecks = useMemo(
    () => decks.filter((deck) => deck.archivedAt !== null),
    [decks],
  );
  const deckLifecycleSignature = useMemo(
    () =>
      decks
        .map((deck) => `${deck.id}:${deck.archivedAt ?? "active"}`)
        .sort()
        .join("|"),
    [decks],
  );
  const orderedFolders = organization?.folders ?? [];
  const unfiledDecks = orderedDecksForFolder(activeDecks, placementByDeck, null);

  useEffect(() => {
    const previous = lastDeckLifecycleSignatureRef.current;
    lastDeckLifecycleSignatureRef.current = deckLifecycleSignature;
    if (previous === null || previous === deckLifecycleSignature) return;
    void reloadOrganization().catch((error: unknown) => {
      setErrorMessage(getErrorMessage(error));
    });
  }, [deckLifecycleSignature]);

  async function reloadOrganization(): Promise<DeckLibraryOrganization> {
    const loaded = await getDeckLibraryOrganization();
    applyOrganization(loaded);
    setErrorMessage(null);
    return loaded;
  }

  function beginOrganizationMutation(): boolean {
    if (organizationMutationRef.current) return false;
    organizationMutationRef.current = true;
    setIsMutating(true);
    return true;
  }

  function finishOrganizationMutation(): void {
    organizationMutationRef.current = false;
    setIsMutating(false);
  }

  async function recoverOrganizationAfterError(
    error: unknown,
    fallback: DeckLibraryOrganization,
  ): Promise<void> {
    let refreshed = false;
    let refreshError: unknown = null;
    try {
      await reloadOrganization();
      refreshed = true;
    } catch (caught: unknown) {
      refreshError = caught;
      applyOrganization(fallback);
    }

    if (getErrorCode(error) === "library_organization_changed") {
      showToast(
        refreshed ? "info" : "error",
        refreshed
          ? "The deck library changed while that action was running. The latest organization is loaded now; try the action again."
          : `The deck library changed and automatic refresh failed: ${getErrorMessage(refreshError)}`,
      );
      return;
    }
    showToast("error", getErrorMessage(error));
  }

  async function refreshLibraryAfterDetail(): Promise<void> {
    const results = await Promise.allSettled([refreshDecks(), reloadOrganization()]);
    const failed = results.find((result) => result.status === "rejected");
    if (failed?.status === "rejected") {
      showToast(
        "info",
        `The deck change was saved, but the library could not fully refresh: ${getErrorMessage(failed.reason)}`,
      );
    }
  }

  function openDeck(deckId: string): void {
    detailOpenedRef.current = false;
    setFolderDraft(null);
    setMoveDraft(null);
    setDetailRequest({ requestId: requestSequence.current, deckId });
    requestSequence.current += 1;
  }

  async function createDeck(name: string, description: string): Promise<void> {
    if (isMutating) return;
    setIsMutating(true);
    try {
      const deck = await controller.create({ name, description });
      setFolderDraft(null);
      const results = await Promise.allSettled([refreshDecks(), reloadOrganization()]);
      const failed = results.find((result) => result.status === "rejected");
      if (failed?.status === "rejected") {
        showToast(
          "info",
          `Created “${deck.name}”, but the library could not fully refresh: ${getErrorMessage(failed.reason)}`,
        );
      } else {
        showToast("success", `Created “${deck.name}”.`);
      }
      openDeck(deck.id);
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    } finally {
      setIsMutating(false);
    }
  }

  async function saveFolder(name: string): Promise<void> {
    const previous = organizationRef.current;
    if (!previous || !folderDraft || isMutating || !beginOrganizationMutation()) return;
    const draft = folderDraft;
    try {
      const updated =
        draft.mode === "create"
          ? await createDeckFolder({
              name,
              expectedOrganizationRevision: previous.revision,
            })
          : await updateDeckFolder({
              folderId: draft.folder?.id ?? "",
              name,
              revision: draft.folder?.revision ?? 0,
              expectedOrganizationRevision: previous.revision,
            });
      applyOrganization(updated);
      setFolderDraft(null);
      setAnnouncement(
        draft.mode === "create" ? `Created folder ${name}.` : `Renamed folder to ${name}.`,
      );
    } catch (error: unknown) {
      await recoverOrganizationAfterError(error, previous);
    } finally {
      finishOrganizationMutation();
    }
  }

  async function removeFolder(folder: DeckFolder): Promise<void> {
    if (!organizationRef.current || isMutating) return;
    const accepted = await confirm({
      title: `Delete “${folder.name}”?`,
      description:
        "The folder will be deleted and its decks will move to Unfiled. Cards, schedules, and review history will remain unchanged.",
      confirmLabel: "Delete folder",
      destructive: true,
    });
    const previous = organizationRef.current;
    if (!accepted || !previous || !beginOrganizationMutation()) return;
    try {
      const updated = await deleteDeckFolder({
        folderId: folder.id,
        revision: folder.revision,
        expectedOrganizationRevision: previous.revision,
      });
      applyOrganization(updated);
      setAnnouncement(`Deleted folder ${folder.name}. Its decks moved to Unfiled.`);
    } catch (error: unknown) {
      await recoverOrganizationAfterError(error, previous);
    } finally {
      finishOrganizationMutation();
    }
  }

  async function moveDeck(
    deck: DeckSummary,
    targetFolderId: string | null,
    targetIndex: number,
  ): Promise<void> {
    const previous = organizationRef.current;
    if (!previous || isMutating || !beginOrganizationMutation()) return;
    const optimistic = optimisticDeckMove(previous, deck.id, targetFolderId, targetIndex);
    setOrganization(optimistic);
    try {
      const updated = await moveDeckInLibrary({
        deckId: deck.id,
        targetFolderId,
        targetIndex,
        expectedOrganizationRevision: previous.revision,
      });
      applyOrganization(updated);
      const folderName =
        targetFolderId === null
          ? "Unfiled"
          : updated.folders.find((folder) => folder.id === targetFolderId)?.name ?? "folder";
      setAnnouncement(`Moved ${deck.name} to ${folderName}, position ${targetIndex + 1}.`);
      setMoveDraft(null);
    } catch (error: unknown) {
      await recoverOrganizationAfterError(error, previous);
    } finally {
      finishOrganizationMutation();
    }
  }

  async function reorderFolder(folder: DeckFolder, targetIndex: number): Promise<void> {
    const previous = organizationRef.current;
    if (!previous || isMutating || !beginOrganizationMutation()) return;
    const optimistic = optimisticFolderMove(previous, folder.id, targetIndex);
    setOrganization(optimistic);
    try {
      const updated = await moveDeckFolder({
        folderId: folder.id,
        targetIndex,
        expectedOrganizationRevision: previous.revision,
      });
      applyOrganization(updated);
      setAnnouncement(`Moved folder ${folder.name} to position ${targetIndex + 1}.`);
    } catch (error: unknown) {
      await recoverOrganizationAfterError(error, previous);
    } finally {
      finishOrganizationMutation();
    }
  }

  async function changeDensity(next: DeckViewDensity): Promise<void> {
    if (next === density) return;
    const previous = density;
    setDensity(next);
    try {
      await saveSettings({ ...settings, deckViewDensity: next });
      setAnnouncement(`${next === "compact" ? "Compact" : "Comfortable"} deck view enabled.`);
    } catch (error: unknown) {
      setDensity(previous);
      showToast("error", getErrorMessage(error));
    }
  }

  function handleDrop(
    event: DragEvent<HTMLElement>,
    targetFolderId: string | null,
    targetIndex: number,
  ): void {
    event.preventDefault();
    event.stopPropagation();
    if (organizationMutationRef.current) return;
    const deckId = event.dataTransfer.getData("application/x-ostudy-deck-id");
    const deck = decks.find((candidate) => candidate.id === deckId);
    if (deck) void moveDeck(deck, targetFolderId, targetIndex);
  }

  if (detailRequest) {
    return (
      <DecksScreen
        key={detailRequest.requestId}
        decks={decks}
        settings={settings}
        refreshDecks={refreshDecks}
        onPracticeDeck={onPracticeDeck}
        openDeckRequest={detailRequest}
        onOpenDeckRequestConsumed={() => undefined}
        onManageTemplates={onManageTemplates}
        onBulkAdd={onCreateCards}
        onViewChange={(view) => {
          if (view === "detail" || view === "editor") detailOpenedRef.current = true;
          if (view === "library" && detailOpenedRef.current) {
            detailOpenedRef.current = false;
            setDetailRequest(null);
            void refreshLibraryAfterDetail();
          }
        }}
        onDirtyChange={onCardDirtyChange}
        showToast={showToast}
      />
    );
  }

  return (
    <div className={styles.root} aria-busy={isLoading || isMutating}>
      <header className={styles.toolbar}>
        <div>
          <span className={styles.eyebrow}>Deck library</span>
          <h2>Organize what you study</h2>
          <p>Folders appear first, followed by decks that have not been filed yet.</p>
        </div>
        <div className={styles.toolbarActions}>
          <div className={styles.density} role="group" aria-label="Deck list density">
            <button
              type="button"
              aria-pressed={density === "comfortable"}
              onClick={() => void changeDensity("comfortable")}
            >
              Comfortable
            </button>
            <button
              type="button"
              aria-pressed={density === "compact"}
              onClick={() => void changeDensity("compact")}
            >
              Compact
            </button>
            <HelpTrigger topicId="flashcards.deckDensity" label="Help with deck density" />
          </div>
          <Button onClick={() => setFolderDraft({ mode: "create", folder: null, name: "" })}>
            <Icon name="folder" size={18} /> New folder
          </Button>
          <Button
            variant="primary"
            onClick={() => setFolderDraft({ mode: "create", folder: null, name: "__deck__" })}
          >
            <Icon name="plus" size={18} /> New deck
          </Button>
        </div>
      </header>

      <div className="sr-only" aria-live="polite">{announcement}</div>
      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      {isLoading ? <LoadingState label="Loading deck organization…" /> : null}

      {!isLoading && organization ? (
        activeDecks.length === 0 && orderedFolders.length === 0 ? (
          <EmptyState
            title="Create your first deck"
            description="Start with a deck, then organize related study material into folders as the library grows."
            icon={<Artwork slot="empty.deck" />}
            actions={
              <Button
                variant="primary"
                onClick={() => setFolderDraft({ mode: "create", folder: null, name: "__deck__" })}
              >
                Create deck
              </Button>
            }
          />
        ) : (
          <div className={styles.library} data-density={density}>
            <section className={styles.folderArea} aria-labelledby="folders-heading">
              <div className={styles.sectionHeading}>
                <div>
                  <h3 id="folders-heading">Folders</h3>
                  <p>{orderedFolders.length} {orderedFolders.length === 1 ? "folder" : "folders"}</p>
                </div>
                <HelpTrigger topicId="flashcards.folders" label="Help with folders" />
              </div>
              {orderedFolders.length === 0 ? (
                <Surface level={1} material="soft" className={styles.emptyFolderHint}>
                  <Icon name="folder" size={24} />
                  <span>Create folders when you want to group related decks.</span>
                </Surface>
              ) : (
                <div className={styles.folderList}>
                  {orderedFolders.map((folder, folderIndex) => {
                    const folderDecks = orderedDecksForFolder(activeDecks, placementByDeck, folder.id);
                    return (
                      <FolderSection
                        key={folder.id}
                        folder={folder}
                        decks={folderDecks}
                        density={density}
                        folderIndex={folderIndex}
                        folderCount={orderedFolders.length}
                        disabled={isMutating}
                        onOpenDeck={openDeck}
                        onPracticeDeck={onPracticeDeck}
                        onRename={() => setFolderDraft({ mode: "rename", folder, name: folder.name })}
                        onDelete={() => void removeFolder(folder)}
                        onMoveFolder={(target) => void reorderFolder(folder, target)}
                        onMoveDeck={(deck) => setMoveDraft({ deck, folderId: folder.id })}
                        onMoveDeckPosition={(deck, target) => void moveDeck(deck, folder.id, target)}
                        onDragStart={(event, deck) => {
                          event.dataTransfer.effectAllowed = "move";
                          event.dataTransfer.setData("application/x-ostudy-deck-id", deck.id);
                        }}
                        onDrop={(event, index) => handleDrop(event, folder.id, index)}
                      />
                    );
                  })}
                </div>
              )}
            </section>

            <section
              className={styles.unfiledArea}
              aria-labelledby="unfiled-heading"
              onDragOver={(event) => event.preventDefault()}
              onDrop={(event) => handleDrop(event, null, unfiledDecks.length)}
            >
              <div className={styles.sectionHeading}>
                <div>
                  <h3 id="unfiled-heading">Unfiled decks</h3>
                  <p>{unfiledDecks.length} {unfiledDecks.length === 1 ? "deck" : "decks"}</p>
                </div>
              </div>
              {unfiledDecks.length === 0 ? (
                <Surface level={1} material="soft" className={styles.emptyFolderHint}>
                  <Icon name="check" size={24} />
                  <span>Every active deck is currently organized in a folder.</span>
                </Surface>
              ) : (
                <div className={styles.deckList}>
                  {unfiledDecks.map((deck, index) => (
                    <DeckItem
                      key={deck.id}
                      deck={deck}
                      density={density}
                      index={index}
                      count={unfiledDecks.length}
                      disabled={isMutating}
                      onOpen={() => openDeck(deck.id)}
                      onPractice={() => void onPracticeDeck(deck.id, "library")}
                      onMove={() => setMoveDraft({ deck, folderId: null })}
                      onMovePosition={(target) => void moveDeck(deck, null, target)}
                      onDragStart={(event) => {
                        event.dataTransfer.effectAllowed = "move";
                        event.dataTransfer.setData("application/x-ostudy-deck-id", deck.id);
                      }}
                      onDrop={(event) => handleDrop(event, null, index)}
                    />
                  ))}
                </div>
              )}
            </section>

            {archivedDecks.length > 0 ? (
              <section className={styles.archivedArea}>
                <button
                  type="button"
                  className={styles.archivedToggle}
                  aria-expanded={showArchived}
                  onClick={() => setShowArchived((current) => !current)}
                >
                  <span>
                    <strong>Archived decks</strong>
                    <small>{archivedDecks.length} stored with cards suspended</small>
                  </span>
                  <Icon name={showArchived ? "chevron-down" : "chevron-right"} size={18} />
                </button>
                {showArchived ? (
                  <div className={styles.deckList}>
                    {archivedDecks.map((deck, index) => (
                      <DeckItem
                        key={deck.id}
                        deck={deck}
                        density="compact"
                        index={index}
                        count={archivedDecks.length}
                        disabled={isMutating}
                        archived
                        onOpen={() => openDeck(deck.id)}
                        onPractice={() => undefined}
                        onMove={() => undefined}
                        onMovePosition={() => undefined}
                        onDragStart={() => undefined}
                        onDrop={() => undefined}
                      />
                    ))}
                  </div>
                ) : null}
              </section>
            ) : null}
          </div>
        )
      ) : null}

      {folderDraft?.name === "__deck__" ? (
        <DeckCreateSheet
          busy={isMutating}
          onClose={() => setFolderDraft(null)}
          onCreate={createDeck}
        />
      ) : null}
      {folderDraft && folderDraft.name !== "__deck__" ? (
        <FolderSheet
          draft={folderDraft}
          busy={isMutating}
          onClose={() => setFolderDraft(null)}
          onSave={saveFolder}
        />
      ) : null}
      {moveDraft && organization ? (
        <MoveDeckSheet
          draft={moveDraft}
          organization={organization}
          disabled={isMutating}
          onClose={() => setMoveDraft(null)}
          onMove={(folderId) => {
            const targetCount = orderedDecksForFolder(
              activeDecks.filter((deck) => deck.id !== moveDraft.deck.id),
              placementByDeck,
              folderId,
            ).length;
            void moveDeck(moveDraft.deck, folderId, targetCount);
          }}
        />
      ) : null}
    </div>
  );
}

function FolderSection({
  folder,
  decks,
  density,
  folderIndex,
  folderCount,
  disabled,
  onOpenDeck,
  onPracticeDeck,
  onRename,
  onDelete,
  onMoveFolder,
  onMoveDeck,
  onMoveDeckPosition,
  onDragStart,
  onDrop,
}: {
  folder: DeckFolder;
  decks: DeckSummary[];
  density: DeckViewDensity;
  folderIndex: number;
  folderCount: number;
  disabled: boolean;
  onOpenDeck: (deckId: string) => void;
  onPracticeDeck: (deckId: string, origin: "library" | "detail") => void | Promise<void>;
  onRename: () => void;
  onDelete: () => void;
  onMoveFolder: (target: number) => void;
  onMoveDeck: (deck: DeckSummary) => void;
  onMoveDeckPosition: (deck: DeckSummary, target: number) => void;
  onDragStart: (event: DragEvent<HTMLElement>, deck: DeckSummary) => void;
  onDrop: (event: DragEvent<HTMLElement>, index: number) => void;
}) {
  const [collapsed, setCollapsed] = useState(false);
  const due = decks.reduce((sum, deck) => sum + deck.studyableDueCount, 0);
  const contentId = `deck-folder-${folder.id}`;
  return (
    <Surface
      as="section"
      level={2}
      className={styles.folder}
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => onDrop(event, decks.length)}
    >
      <header className={styles.folderHeader}>
        <button
          type="button"
          className={styles.folderToggle}
          aria-expanded={!collapsed}
          aria-controls={contentId}
          aria-label={`${collapsed ? "Expand" : "Collapse"} ${folder.name} folder`}
          onClick={() => setCollapsed((current) => !current)}
        >
          <span className={styles.folderIcon}>
            <Icon name={collapsed ? "folder" : "folder-open"} size={22} />
          </span>
          <span className={styles.folderTitle}>
            <h4>{folder.name}</h4>
            <span>{decks.length} {decks.length === 1 ? "deck" : "decks"} · {due} ready</span>
          </span>
          <span className={styles.folderChevron} aria-hidden="true">
            <Icon name={collapsed ? "chevron-right" : "chevron-down"} size={18} />
          </span>
        </button>
        <div className={styles.folderActions}>
          <button type="button" disabled={disabled || folderIndex === 0} aria-label={`Move ${folder.name} folder up`} onClick={() => onMoveFolder(folderIndex - 1)}>↑</button>
          <button type="button" disabled={disabled || folderIndex === folderCount - 1} aria-label={`Move ${folder.name} folder down`} onClick={() => onMoveFolder(folderIndex + 1)}>↓</button>
          <button type="button" disabled={disabled} onClick={onRename}>Rename</button>
          <button type="button" disabled={disabled} className={styles.dangerText} onClick={onDelete}>Delete</button>
        </div>
      </header>
      {!collapsed ? (
        decks.length === 0 ? (
          <div id={contentId} className={styles.emptyFolder}>
            <Artwork slot="empty.folder" />
            <p>Drag a deck here or use its Move action.</p>
          </div>
        ) : (
          <div id={contentId} className={styles.deckList}>
            {decks.map((deck, index) => (
              <DeckItem
                key={deck.id}
                deck={deck}
                density={density}
                index={index}
                count={decks.length}
                disabled={disabled}
                onOpen={() => onOpenDeck(deck.id)}
                onPractice={() => void onPracticeDeck(deck.id, "library")}
                onMove={() => onMoveDeck(deck)}
                onMovePosition={(target) => onMoveDeckPosition(deck, target)}
                onDragStart={(event) => onDragStart(event, deck)}
                onDrop={(event) => onDrop(event, index)}
              />
            ))}
          </div>
        )
      ) : null}
    </Surface>
  );
}

function DeckItem({
  deck,
  density,
  index,
  count,
  disabled,
  archived = false,
  onOpen,
  onPractice,
  onMove,
  onMovePosition,
  onDragStart,
  onDrop,
}: {
  deck: DeckSummary;
  density: DeckViewDensity;
  index: number;
  count: number;
  disabled: boolean;
  archived?: boolean;
  onOpen: () => void;
  onPractice: () => void;
  onMove: () => void;
  onMovePosition: (target: number) => void;
  onDragStart: (event: DragEvent<HTMLElement>) => void;
  onDrop: (event: DragEvent<HTMLElement>) => void;
}) {
  const progressMax = Math.max(1, deck.generatedCardCount);
  const completed = Math.max(0, deck.generatedCardCount - deck.studyableDueCount);
  return (
    <article
      className={styles.deckItem}
      data-density={density}
      data-archived={archived || undefined}
      onDragOver={(event) => event.preventDefault()}
      onDrop={onDrop}
    >
      {!archived ? (
        <span
          className={styles.dragHandle}
          draggable={!disabled}
          role="button"
          tabIndex={0}
          aria-label={`Drag ${deck.name}`}
          onDragStart={onDragStart}
          title="Drag to move this deck"
        >
          <Icon name="move" size={16} />
        </span>
      ) : null}
      <button type="button" className={styles.deckMain} onClick={onOpen}>
        <span className={styles.deckMark}><Icon name="cards" size={20} /></span>
        <span className={styles.deckCopy}>
          <strong>{deck.name}</strong>
          {deck.description ? <span>{deck.description}</span> : null}
          <small>{deck.generatedCardCount} cards · {deck.studyableDueCount} ready</small>
        </span>
        {density === "comfortable" && !archived ? (
          <Progress
            className={styles.deckProgress}
            value={completed}
            max={progressMax}
            label={`${deck.name} study progress`}
            size="small"
          />
        ) : null}
      </button>
      <div className={styles.deckActions}>
        {!archived ? (
          <button type="button" disabled={disabled || deck.generatedCardCount === 0} onClick={onPractice}>Practice</button>
        ) : null}
        {!archived ? <button type="button" disabled={disabled} onClick={onMove}>Move</button> : null}
        {!archived ? (
          <>
            <button type="button" disabled={disabled || index === 0} aria-label={`Move ${deck.name} up`} onClick={() => onMovePosition(index - 1)}>↑</button>
            <button type="button" disabled={disabled || index === count - 1} aria-label={`Move ${deck.name} down`} onClick={() => onMovePosition(index + 1)}>↓</button>
          </>
        ) : null}
      </div>
    </article>
  );
}

function DeckCreateSheet({
  busy,
  onClose,
  onCreate,
}: {
  busy: boolean;
  onClose: () => void;
  onCreate: (name: string, description: string) => Promise<void>;
}) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  return (
    <Sheet title="Create a deck" description="Add a new place for related flashcards." width="small" onClose={onClose} busy={busy} dismissible={!busy}>
      <form className={styles.sheetForm} onSubmit={(event) => { event.preventDefault(); void onCreate(name, description); }}>
        <label><span>Name</span><input data-autofocus="true" value={name} maxLength={200} onChange={(event) => setName(event.currentTarget.value)} /></label>
        <label><span>Description <small>optional</small></span><textarea value={description} maxLength={2000} rows={4} onChange={(event) => setDescription(event.currentTarget.value)} /></label>
        <div className={styles.sheetActions}><Button disabled={busy} onClick={onClose}>Cancel</Button><Button type="submit" variant="primary" loading={busy} disabled={name.trim() === ""}>Create deck</Button></div>
      </form>
    </Sheet>
  );
}

function FolderSheet({
  draft,
  busy,
  onClose,
  onSave,
}: {
  draft: FolderDraft;
  busy: boolean;
  onClose: () => void;
  onSave: (name: string) => Promise<void>;
}) {
  const [name, setName] = useState(draft.name);
  return (
    <Sheet title={draft.mode === "create" ? "Create a folder" : "Rename folder"} description="Folders organize decks without changing their cards or review history." width="small" onClose={onClose} busy={busy} dismissible={!busy}>
      <form className={styles.sheetForm} onSubmit={(event: FormEvent<HTMLFormElement>) => { event.preventDefault(); void onSave(name); }}>
        <label><span>Folder name</span><input data-autofocus="true" value={name} maxLength={120} onChange={(event) => setName(event.currentTarget.value)} /></label>
        <div className={styles.sheetActions}><Button disabled={busy} onClick={onClose}>Cancel</Button><Button type="submit" variant="primary" loading={busy} disabled={name.trim() === ""}>{draft.mode === "create" ? "Create folder" : "Save name"}</Button></div>
      </form>
    </Sheet>
  );
}

function MoveDeckSheet({
  draft,
  organization,
  disabled,
  onClose,
  onMove,
}: {
  draft: MoveDeckDraft;
  organization: DeckLibraryOrganization;
  disabled: boolean;
  onClose: () => void;
  onMove: (folderId: string | null) => void;
}) {
  const [folderId, setFolderId] = useState(draft.folderId ?? "");
  return (
    <Sheet title={`Move “${draft.deck.name}”`} description="Choose a folder. Unfiled keeps the deck at the top level." width="small" onClose={onClose} busy={disabled} dismissible={!disabled}>
      <div className={styles.sheetForm}>
        <label><span>Destination</span><select value={folderId} onChange={(event) => setFolderId(event.currentTarget.value)}><option value="">Unfiled</option>{organization.folders.map((folder) => <option key={folder.id} value={folder.id}>{folder.name}</option>)}</select></label>
        <div className={styles.sheetActions}><Button disabled={disabled} onClick={onClose}>Cancel</Button><Button variant="primary" loading={disabled} onClick={() => onMove(folderId === "" ? null : folderId)}>Move deck</Button></div>
      </div>
    </Sheet>
  );
}

function orderedDecksForFolder(
  decks: DeckSummary[],
  placementByDeck: Map<string, { folderId: string | null; sortOrder: number }>,
  folderId: string | null,
): DeckSummary[] {
  return decks
    .filter((deck) => (placementByDeck.get(deck.id)?.folderId ?? null) === folderId)
    .sort((left, right) => {
      const leftOrder = placementByDeck.get(left.id)?.sortOrder ?? Number.MAX_SAFE_INTEGER;
      const rightOrder = placementByDeck.get(right.id)?.sortOrder ?? Number.MAX_SAFE_INTEGER;
      return leftOrder - rightOrder || left.name.localeCompare(right.name);
    });
}

function optimisticDeckMove(
  organization: DeckLibraryOrganization,
  deckId: string,
  targetFolderId: string | null,
  targetIndex: number,
): DeckLibraryOrganization {
  const placements = organization.placements.map((placement) => ({ ...placement }));
  const moved = placements.find((placement) => placement.deckId === deckId);
  if (!moved) return organization;
  const previousFolderId = moved.folderId;
  moved.folderId = targetFolderId;
  const normalize = (folderId: string | null, insertedId?: string) => {
    const ids = placements
      .filter((placement) => placement.folderId === folderId && placement.deckId !== insertedId)
      .sort((left, right) => left.sortOrder - right.sortOrder)
      .map((placement) => placement.deckId);
    if (insertedId) ids.splice(Math.min(Math.max(0, targetIndex), ids.length), 0, insertedId);
    ids.forEach((id, index) => {
      const placement = placements.find((candidate) => candidate.deckId === id);
      if (placement) placement.sortOrder = (index + 1) * 1_000;
    });
  };
  normalize(targetFolderId, deckId);
  if (previousFolderId !== targetFolderId) normalize(previousFolderId);
  return { ...organization, revision: organization.revision + 1, placements };
}

function optimisticFolderMove(
  organization: DeckLibraryOrganization,
  folderId: string,
  targetIndex: number,
): DeckLibraryOrganization {
  const folders = organization.folders.filter((folder) => folder.id !== folderId);
  const moved = organization.folders.find((folder) => folder.id === folderId);
  if (!moved) return organization;
  folders.splice(Math.min(Math.max(0, targetIndex), folders.length), 0, moved);
  return {
    ...organization,
    revision: organization.revision + 1,
    folders: folders.map((folder, index) => ({
      ...folder,
      sortOrder: (index + 1) * 1_000,
    })),
  };
}