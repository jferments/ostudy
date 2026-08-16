import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type DragEvent,
  type FormEvent,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
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
import { LoadingState } from "../../ui/primitives/EmptyState";
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

type LibraryDragPayload =
  | { kind: "deck"; id: string }
  | { kind: "folder"; id: string };

interface PointerDragState {
  payload: LibraryDragPayload;
  startX: number;
  startY: number;
  active: boolean;
}

const DECK_DRAG_MIME = "application/x-ostudy-deck-id";
const FOLDER_DRAG_MIME = "application/x-ostudy-folder-id";
const UNSORTED_DATA_VALUE = "__unsorted__";

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
  const [isMutating, setIsMutating] = useState(false);
  const [density, setDensity] = useState<DeckViewDensity>(
    settings.deckViewDensity ?? "comfortable",
  );
  const [announcement, setAnnouncement] = useState("");
  const [showArchived, setShowArchived] = useState(false);
  const [collapsedFolderIds, setCollapsedFolderIds] = useState<Set<string>>(new Set());
  const [detailRequest, setDetailRequest] = useState<DeckOpenRequest | null>(null);
  const requestSequence = useRef(1);
  const detailOpenedRef = useRef(false);
  const organizationRef = useRef<DeckLibraryOrganization | null>(null);
  const organizationMutationRef = useRef(false);
  const lastDeckLifecycleSignatureRef = useRef<string | null>(null);
  const pointerDragRef = useRef<PointerDragState | null>(null);
  const pointerDropTargetRef = useRef<HTMLElement | null>(null);
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
      clearPointerDropTarget();
    };
  }, [controller]);

  useEffect(() => {
    if (!openDeckRequest) return;
    detailOpenedRef.current = false;
    setFolderDraft(null);
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
  const unsortedDecks = orderedDecksForFolder(activeDecks, placementByDeck, null);
  const allFoldersCollapsed =
    orderedFolders.length > 0 && orderedFolders.every((folder) => collapsedFolderIds.has(folder.id));

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
        "The folder will be deleted and its decks will move to Unsorted decks. Cards, schedules, and review history will remain unchanged.",
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
      setCollapsedFolderIds((current) => {
        const next = new Set(current);
        next.delete(folder.id);
        return next;
      });
      setAnnouncement(`Deleted folder ${folder.name}. Its decks moved to Unsorted decks.`);
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
    const currentPlacement = previous.placements.find((placement) => placement.deckId === deck.id);
    const currentGroup = orderedDecksForFolder(activeDecks, placementByDeck, currentPlacement?.folderId ?? null);
    const currentIndex = currentGroup.findIndex((candidate) => candidate.id === deck.id);
    if (
      (currentPlacement?.folderId ?? null) === targetFolderId &&
      currentIndex === Math.min(Math.max(0, targetIndex), Math.max(0, currentGroup.length - 1))
    ) {
      finishOrganizationMutation();
      return;
    }
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
          ? "Unsorted decks"
          : updated.folders.find((folder) => folder.id === targetFolderId)?.name ?? "folder";
      setAnnouncement(`Moved ${deck.name} to ${folderName}, position ${targetIndex + 1}.`);
    } catch (error: unknown) {
      await recoverOrganizationAfterError(error, previous);
    } finally {
      finishOrganizationMutation();
    }
  }

  async function reorderFolder(folder: DeckFolder, targetIndex: number): Promise<void> {
    const previous = organizationRef.current;
    if (!previous || isMutating || !beginOrganizationMutation()) return;
    const currentIndex = previous.folders.findIndex((candidate) => candidate.id === folder.id);
    const boundedTarget = Math.min(Math.max(0, targetIndex), Math.max(0, previous.folders.length - 1));
    if (currentIndex === boundedTarget) {
      finishOrganizationMutation();
      return;
    }
    const optimistic = optimisticFolderMove(previous, folder.id, boundedTarget);
    setOrganization(optimistic);
    try {
      const updated = await moveDeckFolder({
        folderId: folder.id,
        targetIndex: boundedTarget,
        expectedOrganizationRevision: previous.revision,
      });
      applyOrganization(updated);
      setAnnouncement(`Moved folder ${folder.name} to position ${boundedTarget + 1}.`);
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

  function handleDeckDrop(
    event: DragEvent<HTMLElement>,
    targetFolderId: string | null,
    targetIndex: number,
  ): void {
    event.preventDefault();
    event.stopPropagation();
    if (organizationMutationRef.current) return;
    const deckId = readTransferValue(event, DECK_DRAG_MIME);
    const deck = decks.find((candidate) => candidate.id === deckId);
    if (deck) void moveDeck(deck, targetFolderId, targetIndex);
  }

  function handleFolderDrop(
    event: DragEvent<HTMLElement>,
    folder: DeckFolder,
    folderIndex: number,
    deckCount: number,
  ): void {
    event.preventDefault();
    event.stopPropagation();
    if (organizationMutationRef.current) return;
    const draggedFolderId = readTransferValue(event, FOLDER_DRAG_MIME);
    if (draggedFolderId) {
      const draggedFolder = orderedFolders.find((candidate) => candidate.id === draggedFolderId);
      if (draggedFolder) void reorderFolder(draggedFolder, folderIndex);
      return;
    }
    handleDeckDrop(event, folder.id, deckCount);
  }

  function handleFolderTailDrop(event: DragEvent<HTMLElement>): void {
    event.preventDefault();
    event.stopPropagation();
    const draggedFolderId = readTransferValue(event, FOLDER_DRAG_MIME);
    const draggedFolder = orderedFolders.find((candidate) => candidate.id === draggedFolderId);
    if (draggedFolder) void reorderFolder(draggedFolder, orderedFolders.length - 1);
  }

  function handleFolderKeyboardMove(
    event: KeyboardEvent<HTMLElement>,
    folder: DeckFolder,
    index: number,
  ): void {
    let target: number | null = null;
    if (event.key === "ArrowUp") target = index - 1;
    if (event.key === "ArrowDown") target = index + 1;
    if (event.key === "Home") target = 0;
    if (event.key === "End") target = orderedFolders.length - 1;
    if (target === null || target < 0 || target >= orderedFolders.length) return;
    event.preventDefault();
    void reorderFolder(folder, target);
  }

  function handleDeckKeyboardMove(
    event: KeyboardEvent<HTMLElement>,
    deck: DeckSummary,
    folderId: string | null,
    index: number,
    count: number,
  ): void {
    let targetIndex: number | null = null;
    if (event.key === "ArrowUp") targetIndex = index - 1;
    if (event.key === "ArrowDown") targetIndex = index + 1;
    if (event.key === "Home") targetIndex = 0;
    if (event.key === "End") targetIndex = count - 1;
    if (targetIndex !== null) {
      if (targetIndex < 0 || targetIndex >= count) return;
      event.preventDefault();
      void moveDeck(deck, folderId, targetIndex);
      return;
    }
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    const groups = [...orderedFolders.map((folder) => folder.id), null];
    const currentGroupIndex = groups.findIndex((candidate) => candidate === folderId);
    const nextGroupIndex = currentGroupIndex + (event.key === "ArrowLeft" ? -1 : 1);
    const nextFolderId = groups[nextGroupIndex];
    if (nextGroupIndex < 0 || nextGroupIndex >= groups.length || nextFolderId === undefined) return;
    event.preventDefault();
    const targetCount = orderedDecksForFolder(
      activeDecks.filter((candidate) => candidate.id !== deck.id),
      placementByDeck,
      nextFolderId,
    ).length;
    void moveDeck(deck, nextFolderId, targetCount);
  }

  function beginPointerDrag(
    event: ReactPointerEvent<HTMLElement>,
    payload: LibraryDragPayload,
  ): void {
    if (event.pointerType === "mouse" || event.button !== 0 || isMutating) return;
    pointerDragRef.current = {
      payload,
      startX: event.clientX,
      startY: event.clientY,
      active: false,
    };
    event.currentTarget.setPointerCapture(event.pointerId);
  }

  function updatePointerDrag(event: ReactPointerEvent<HTMLElement>): void {
    const state = pointerDragRef.current;
    if (!state) return;
    if (!state.active) {
      const distance = Math.hypot(event.clientX - state.startX, event.clientY - state.startY);
      if (distance < 8) return;
      state.active = true;
    }
    event.preventDefault();
    const candidate = document
      .elementFromPoint(event.clientX, event.clientY)
      ?.closest<HTMLElement>("[data-library-drop]");
    const compatible = candidate && isCompatiblePointerTarget(state.payload, candidate)
      ? candidate
      : null;
    setPointerDropTarget(compatible, state.payload.kind);
  }

  function finishPointerDrag(event: ReactPointerEvent<HTMLElement>, commit: boolean): void {
    const state = pointerDragRef.current;
    const target = pointerDropTargetRef.current;
    pointerDragRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    clearPointerDropTarget();
    if (!commit || !state?.active || !target || organizationMutationRef.current) return;

    if (state.payload.kind === "folder") {
      const folder = orderedFolders.find((candidate) => candidate.id === state.payload.id);
      const targetIndex = Number(target.dataset.folderIndex);
      if (folder && Number.isInteger(targetIndex)) void reorderFolder(folder, targetIndex);
      return;
    }

    const deck = decks.find((candidate) => candidate.id === state.payload.id);
    const targetIndex = Number(target.dataset.targetIndex);
    if (!deck || !Number.isInteger(targetIndex)) return;
    const encodedFolderId = target.dataset.folderId;
    const targetFolderId =
      !encodedFolderId || encodedFolderId === UNSORTED_DATA_VALUE ? null : encodedFolderId;
    void moveDeck(deck, targetFolderId, targetIndex);
  }

  function setPointerDropTarget(target: HTMLElement | null, kind: LibraryDragPayload["kind"]): void {
    if (pointerDropTargetRef.current === target) return;
    clearPointerDropTarget();
    pointerDropTargetRef.current = target;
    if (target) target.dataset.dragOver = kind;
  }

  function clearPointerDropTarget(): void {
    if (pointerDropTargetRef.current) delete pointerDropTargetRef.current.dataset.dragOver;
    pointerDropTargetRef.current = null;
  }

  function toggleFolder(folderId: string): void {
    setCollapsedFolderIds((current) => {
      const next = new Set(current);
      if (next.has(folderId)) next.delete(folderId);
      else next.add(folderId);
      return next;
    });
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
      <div className={styles.toolbar} aria-label="Deck library controls">
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
          <Button
            variant="primary"
            onClick={() => setFolderDraft({ mode: "create", folder: null, name: "__deck__" })}
          >
            <Icon name="plus" size={18} /> New deck
          </Button>
        </div>
      </div>

      <div className="sr-only" aria-live="polite">{announcement}</div>
      {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}
      {isLoading ? <LoadingState label="Loading deck organization…" /> : null}

      {!isLoading && organization ? (
        <div className={styles.library} data-density={density}>
          <section className={styles.folderArea} aria-labelledby="folders-heading">
            <div className={styles.sectionHeading}>
              <div>
                <h2 id="folders-heading">Folders</h2>
                <p>{orderedFolders.length} {orderedFolders.length === 1 ? "folder" : "folders"}</p>
              </div>
              <div className={styles.sectionActions}>
                <Button
                  size="small"
                  onClick={() => setFolderDraft({ mode: "create", folder: null, name: "" })}
                >
                  <Icon name="folder" size={17} /> New folder
                </Button>
                <Button
                  size="small"
                  variant="ghost"
                  disabled={orderedFolders.length === 0 || allFoldersCollapsed}
                  onClick={() =>
                    setCollapsedFolderIds(new Set(orderedFolders.map((folder) => folder.id)))
                  }
                >
                  Collapse all
                </Button>
                <HelpTrigger topicId="flashcards.folders" label="Help with folders" />
              </div>
            </div>
            {orderedFolders.length === 0 ? (
              <Surface level={1} material="soft" className={styles.emptyFolderHint}>
                <Icon name="folder" size={24} />
                <span>No folders yet.</span>
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
                      collapsed={collapsedFolderIds.has(folder.id)}
                      onToggleCollapsed={() => toggleFolder(folder.id)}
                      onOpenDeck={openDeck}
                      onPracticeDeck={onPracticeDeck}
                      onRename={() => setFolderDraft({ mode: "rename", folder, name: folder.name })}
                      onDelete={() => void removeFolder(folder)}
                      onMoveFolder={(target) => void reorderFolder(folder, target)}
                      onMoveDeckPosition={(deck, target) => void moveDeck(deck, folder.id, target)}
                      onFolderKeyboardMove={(event) =>
                        handleFolderKeyboardMove(event, folder, folderIndex)
                      }
                      onDeckKeyboardMove={(event, deck, index, count) =>
                        handleDeckKeyboardMove(event, deck, folder.id, index, count)
                      }
                      onFolderDragStart={(event) => {
                        event.dataTransfer.effectAllowed = "move";
                        event.dataTransfer.setData(FOLDER_DRAG_MIME, folder.id);
                        event.dataTransfer.setData("text/plain", folder.id);
                      }}
                      onDeckDragStart={(event, deck) => {
                        event.dataTransfer.effectAllowed = "move";
                        event.dataTransfer.setData(DECK_DRAG_MIME, deck.id);
                        event.dataTransfer.setData("text/plain", deck.id);
                      }}
                      onFolderDrop={(event) =>
                        handleFolderDrop(event, folder, folderIndex, folderDecks.length)
                      }
                      onDeckDrop={(event, index) => handleDeckDrop(event, folder.id, index)}
                      onPointerStart={beginPointerDrag}
                      onPointerMove={updatePointerDrag}
                      onPointerEnd={(event) => finishPointerDrag(event, true)}
                      onPointerCancel={(event) => finishPointerDrag(event, false)}
                    />
                  );
                })}
                <div
                  className={styles.folderDropTail}
                  data-library-drop="folder-tail"
                  data-folder-index={Math.max(0, orderedFolders.length - 1)}
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={handleFolderTailDrop}
                  aria-hidden="true"
                />
              </div>
            )}
          </section>

          <section
            className={styles.unsortedArea}
            aria-labelledby="unsorted-heading"
            data-library-drop="unsorted"
            data-folder-id={UNSORTED_DATA_VALUE}
            data-target-index={unsortedDecks.length}
            onDragOver={(event) => event.preventDefault()}
            onDrop={(event) => handleDeckDrop(event, null, unsortedDecks.length)}
          >
            <div className={styles.sectionHeading}>
              <div>
                <h2 id="unsorted-heading">Unsorted decks</h2>
                <p>{unsortedDecks.length} {unsortedDecks.length === 1 ? "deck" : "decks"}</p>
              </div>
            </div>
            {unsortedDecks.length === 0 ? (
              <Surface level={1} material="soft" className={styles.emptyFolderHint}>
                <Icon name="check" size={24} />
                <span>All active decks are in folders.</span>
              </Surface>
            ) : (
              <div className={styles.deckList}>
                {unsortedDecks.map((deck, index) => (
                  <DeckItem
                    key={deck.id}
                    deck={deck}
                    folderId={null}
                    density={density}
                    index={index}
                    count={unsortedDecks.length}
                    disabled={isMutating}
                    onOpen={() => openDeck(deck.id)}
                    onPractice={() => void onPracticeDeck(deck.id, "library")}
                    onMovePosition={(target) => void moveDeck(deck, null, target)}
                    onKeyboardMove={(event) =>
                      handleDeckKeyboardMove(event, deck, null, index, unsortedDecks.length)
                    }
                    onDragStart={(event) => {
                      event.dataTransfer.effectAllowed = "move";
                      event.dataTransfer.setData(DECK_DRAG_MIME, deck.id);
                      event.dataTransfer.setData("text/plain", deck.id);
                    }}
                    onDrop={(event) => handleDeckDrop(event, null, index)}
                    onPointerStart={(event) => beginPointerDrag(event, { kind: "deck", id: deck.id })}
                    onPointerMove={updatePointerDrag}
                    onPointerEnd={(event) => finishPointerDrag(event, true)}
                    onPointerCancel={(event) => finishPointerDrag(event, false)}
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
                      folderId={null}
                      density="compact"
                      index={index}
                      count={archivedDecks.length}
                      disabled={isMutating}
                      archived
                      onOpen={() => openDeck(deck.id)}
                      onPractice={() => undefined}
                      onMovePosition={() => undefined}
                      onKeyboardMove={() => undefined}
                      onDragStart={() => undefined}
                      onDrop={() => undefined}
                      onPointerStart={() => undefined}
                      onPointerMove={() => undefined}
                      onPointerEnd={() => undefined}
                      onPointerCancel={() => undefined}
                    />
                  ))}
                </div>
              ) : null}
            </section>
          ) : null}
        </div>
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
  collapsed,
  onToggleCollapsed,
  onOpenDeck,
  onPracticeDeck,
  onRename,
  onDelete,
  onMoveFolder,
  onMoveDeckPosition,
  onFolderKeyboardMove,
  onDeckKeyboardMove,
  onFolderDragStart,
  onDeckDragStart,
  onFolderDrop,
  onDeckDrop,
  onPointerStart,
  onPointerMove,
  onPointerEnd,
  onPointerCancel,
}: {
  folder: DeckFolder;
  decks: DeckSummary[];
  density: DeckViewDensity;
  folderIndex: number;
  folderCount: number;
  disabled: boolean;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onOpenDeck: (deckId: string) => void;
  onPracticeDeck: (deckId: string, origin: "library" | "detail") => void | Promise<void>;
  onRename: () => void;
  onDelete: () => void;
  onMoveFolder: (target: number) => void;
  onMoveDeckPosition: (deck: DeckSummary, target: number) => void;
  onFolderKeyboardMove: (event: KeyboardEvent<HTMLElement>) => void;
  onDeckKeyboardMove: (
    event: KeyboardEvent<HTMLElement>,
    deck: DeckSummary,
    index: number,
    count: number,
  ) => void;
  onFolderDragStart: (event: DragEvent<HTMLElement>) => void;
  onDeckDragStart: (event: DragEvent<HTMLElement>, deck: DeckSummary) => void;
  onFolderDrop: (event: DragEvent<HTMLElement>) => void;
  onDeckDrop: (event: DragEvent<HTMLElement>, index: number) => void;
  onPointerStart: (event: ReactPointerEvent<HTMLElement>, payload: LibraryDragPayload) => void;
  onPointerMove: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerEnd: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerCancel: (event: ReactPointerEvent<HTMLElement>) => void;
}) {
  const due = decks.reduce((sum, deck) => sum + deck.studyableDueCount, 0);
  const contentId = `deck-folder-${folder.id}`;
  return (
    <Surface
      as="section"
      level={2}
      className={styles.folder}
      data-library-drop="folder"
      data-folder-id={folder.id}
      data-folder-index={folderIndex}
      data-target-index={decks.length}
      onDragOver={(event) => event.preventDefault()}
      onDrop={onFolderDrop}
    >
      <header className={styles.folderHeader}>
        <span
          className={styles.folderDragHandle}
          draggable={!disabled}
          role="button"
          tabIndex={0}
          aria-label={`Reorder ${folder.name} folder. Use Up and Down arrow keys.`}
          aria-keyshortcuts="ArrowUp ArrowDown Home End"
          title="Drag to reorder folder"
          onDragStart={onFolderDragStart}
          onKeyDown={onFolderKeyboardMove}
          onPointerDown={(event) => onPointerStart(event, { kind: "folder", id: folder.id })}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerEnd}
          onPointerCancel={onPointerCancel}
        >
          <Icon name="move" size={16} />
        </span>
        <button
          type="button"
          className={styles.folderToggle}
          aria-expanded={!collapsed}
          aria-controls={contentId}
          aria-label={`${collapsed ? "Expand" : "Collapse"} ${folder.name} folder`}
          title={collapsed ? "Expand folder" : "Collapse folder"}
          onClick={onToggleCollapsed}
        >
          <span className={styles.folderIcon}>
            <Icon name={collapsed ? "folder" : "folder-open"} size={22} />
          </span>
          <span className={styles.folderTitle}>
            <h3>{folder.name}</h3>
            <span>{decks.length} {decks.length === 1 ? "deck" : "decks"} · {due} ready</span>
          </span>
          <span className={styles.folderChevron} aria-hidden="true">
            <Icon name={collapsed ? "chevron-right" : "chevron-down"} size={18} />
          </span>
        </button>
        <div className={styles.folderActions}>
          <button type="button" disabled={disabled} onClick={onRename}>Rename</button>
          <button type="button" disabled={disabled} className={styles.dangerText} onClick={onDelete}>Delete</button>
        </div>
      </header>
      {!collapsed ? (
        decks.length === 0 ? (
          <div id={contentId} className={styles.emptyFolder}>
            <Artwork slot="empty.folder" />
            <p>Drag a deck here.</p>
          </div>
        ) : (
          <div id={contentId} className={styles.deckList}>
            {decks.map((deck, index) => (
              <DeckItem
                key={deck.id}
                deck={deck}
                folderId={folder.id}
                density={density}
                index={index}
                count={decks.length}
                disabled={disabled}
                onOpen={() => onOpenDeck(deck.id)}
                onPractice={() => void onPracticeDeck(deck.id, "library")}
                onMovePosition={(target) => onMoveDeckPosition(deck, target)}
                onKeyboardMove={(event) => onDeckKeyboardMove(event, deck, index, decks.length)}
                onDragStart={(event) => onDeckDragStart(event, deck)}
                onDrop={(event) => onDeckDrop(event, index)}
                onPointerStart={(event) => onPointerStart(event, { kind: "deck", id: deck.id })}
                onPointerMove={onPointerMove}
                onPointerEnd={onPointerEnd}
                onPointerCancel={onPointerCancel}
              />
            ))}
          </div>
        )
      ) : null}
      <span className="sr-only">
        Folder {folderIndex + 1} of {folderCount}.
      </span>
    </Surface>
  );
}

function DeckItem({
  deck,
  folderId,
  density,
  index,
  count,
  disabled,
  archived = false,
  onOpen,
  onPractice,
  onMovePosition,
  onKeyboardMove,
  onDragStart,
  onDrop,
  onPointerStart,
  onPointerMove,
  onPointerEnd,
  onPointerCancel,
}: {
  deck: DeckSummary;
  folderId: string | null;
  density: DeckViewDensity;
  index: number;
  count: number;
  disabled: boolean;
  archived?: boolean;
  onOpen: () => void;
  onPractice: () => void;
  onMovePosition: (target: number) => void;
  onKeyboardMove: (event: KeyboardEvent<HTMLElement>) => void;
  onDragStart: (event: DragEvent<HTMLElement>) => void;
  onDrop: (event: DragEvent<HTMLElement>) => void;
  onPointerStart: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerMove: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerEnd: (event: ReactPointerEvent<HTMLElement>) => void;
  onPointerCancel: (event: ReactPointerEvent<HTMLElement>) => void;
}) {
  const progressMax = Math.max(1, deck.generatedCardCount);
  const completed = Math.max(0, deck.generatedCardCount - deck.studyableDueCount);
  return (
    <article
      className={styles.deckItem}
      data-density={density}
      data-archived={archived || undefined}
      data-library-drop={archived ? undefined : "deck"}
      data-folder-id={folderId ?? UNSORTED_DATA_VALUE}
      data-target-index={index}
      onDragOver={archived ? undefined : (event) => event.preventDefault()}
      onDrop={archived ? undefined : onDrop}
    >
      {!archived ? (
        <span
          className={styles.dragHandle}
          draggable={!disabled}
          role="button"
          tabIndex={0}
          aria-label={`Reorder ${deck.name}. Use Up and Down within this group or Left and Right between folders.`}
          aria-keyshortcuts="ArrowUp ArrowDown ArrowLeft ArrowRight Home End"
          onDragStart={onDragStart}
          onKeyDown={onKeyboardMove}
          onPointerDown={onPointerStart}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerEnd}
          onPointerCancel={onPointerCancel}
          title="Drag to reorder or move this deck"
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
      {!archived ? (
        <div className={styles.deckActions}>
          <Button
            size="small"
            className={styles.practiceButton}
            disabled={disabled || deck.generatedCardCount === 0}
            onClick={onPractice}
          >
            Practice
          </Button>
        </div>
      ) : null}
      <span className="sr-only">
        Deck {index + 1} of {count}.
        <button type="button" onClick={() => onMovePosition(Math.max(0, index - 1))}>Move earlier</button>
        <button type="button" onClick={() => onMovePosition(Math.min(count - 1, index + 1))}>Move later</button>
      </span>
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
    <Sheet
      title="Create a deck"
      description="Add a new place for related flashcards."
      width="small"
      onClose={onClose}
      busy={busy}
      dismissible={!busy}
    >
      <form
        className={styles.sheetForm}
        onSubmit={(event) => {
          event.preventDefault();
          void onCreate(name, description);
        }}
      >
        <label>
          <span>Name</span>
          <input
            data-autofocus="true"
            value={name}
            maxLength={200}
            onChange={(event) => setName(event.currentTarget.value)}
          />
        </label>
        <label>
          <span>Description <small>optional</small></span>
          <textarea
            value={description}
            maxLength={2000}
            rows={4}
            onChange={(event) => setDescription(event.currentTarget.value)}
          />
        </label>
        <div className={styles.sheetActions}>
          <Button disabled={busy} onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" loading={busy} disabled={name.trim() === ""}>
            Create deck
          </Button>
        </div>
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
    <Sheet
      title={draft.mode === "create" ? "Create a folder" : "Rename folder"}
      width="small"
      onClose={onClose}
      busy={busy}
      dismissible={!busy}
    >
      <form
        className={styles.sheetForm}
        onSubmit={(event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          void onSave(name);
        }}
      >
        <label>
          <span>Folder name</span>
          <input
            data-autofocus="true"
            value={name}
            maxLength={120}
            onChange={(event) => setName(event.currentTarget.value)}
          />
        </label>
        <div className={styles.sheetActions}>
          <Button disabled={busy} onClick={onClose}>Cancel</Button>
          <Button type="submit" variant="primary" loading={busy} disabled={name.trim() === ""}>
            {draft.mode === "create" ? "Create folder" : "Save name"}
          </Button>
        </div>
      </form>
    </Sheet>
  );
}

function readTransferValue(event: DragEvent<HTMLElement>, type: string): string {
  try {
    return event.dataTransfer.getData(type);
  } catch {
    return "";
  }
}

function isCompatiblePointerTarget(payload: LibraryDragPayload, target: HTMLElement): boolean {
  const kind = target.dataset.libraryDrop;
  if (payload.kind === "folder") return kind === "folder" || kind === "folder-tail";
  return kind === "deck" || kind === "folder" || kind === "unsorted";
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
