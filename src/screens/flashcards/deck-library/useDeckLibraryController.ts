import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import type { ShowToast } from "../../../components/Toast";
import {
  DecksController,
  runSequentialDeckMutations,
  type SequentialDeckMutationOutcome,
} from "../../../controllers/decksController";
import { getErrorCode, getErrorMessage } from "../../../ipc/common";
import type { DeckSummary } from "../../../ipc/decks";
import {
  createDeckFolder,
  deleteDeckFolder,
  getDeckLibraryOrganization,
  moveDeckFolder,
  moveDeckInLibrary,
  updateDeckFolder,
  type CreateDeckFolderInput,
  type DeckLibraryOrganization,
  type DeleteDeckFolderInput,
  type MoveDeckFolderInput,
  type MoveDeckInLibraryInput,
  type UpdateDeckFolderInput,
} from "../../../ipc/deckFolders";
import type { ConfirmOptions } from "../../../ui/overlays/overlayContext";
import {
  applyOptimisticMove,
  createDeckLibrarySnapshot,
  deckLifecycleSignature,
  validateMoveIntent,
  type DeckLibrarySnapshot,
} from "./organizationModel";
import {
  addDecksToSelection,
  analyzeDeckSelection,
  clearDeckSelection,
  deckSelectionIds,
  emptyDeckSelection,
  reconcileDeckSelection,
  removeDecksFromSelection,
  toggleDeckSelection,
  type DeckSelectionAnalysis,
  type DeckSelectionState,
} from "./selectionModel";
import type { LibraryMoveIntent, LibrarySheetState } from "./types";

interface DeckMutationController {
  create(input: { name: string; description: string }): Promise<DeckSummary>;
  setArchived(input: {
    deckId: string;
    revision: number;
    archived: boolean;
  }): Promise<DeckSummary>;
  deleteDeck(deckId: string): Promise<{
    undoToken: string;
    undoExpiresAt: number;
  }>;
  undo(undoToken: string): Promise<unknown>;
  invalidate(): void;
}

export interface DeckLibraryControllerDependencies {
  getOrganization(): Promise<DeckLibraryOrganization>;
  createFolder(input: CreateDeckFolderInput): Promise<DeckLibraryOrganization>;
  updateFolder(input: UpdateDeckFolderInput): Promise<DeckLibraryOrganization>;
  deleteFolder(input: DeleteDeckFolderInput): Promise<DeckLibraryOrganization>;
  moveDeck(input: MoveDeckInLibraryInput): Promise<DeckLibraryOrganization>;
  moveFolder(input: MoveDeckFolderInput): Promise<DeckLibraryOrganization>;
  createDeckController(): DeckMutationController;
}

const defaultDependencies: DeckLibraryControllerDependencies = {
  getOrganization: getDeckLibraryOrganization,
  createFolder: createDeckFolder,
  updateFolder: updateDeckFolder,
  deleteFolder: deleteDeckFolder,
  moveDeck: moveDeckInLibrary,
  moveFolder: moveDeckFolder,
  createDeckController: () => new DecksController(),
};

export interface UseDeckLibraryControllerOptions {
  decks: DeckSummary[];
  refreshDecks: () => Promise<DeckSummary[]>;
  onOpenDeck: (deckId: string) => void;
  showToast: ShowToast;
  confirm: (options: ConfirmOptions) => Promise<boolean>;
  dependencies?: DeckLibraryControllerDependencies;
}

export interface DeckLibraryController {
  organization: DeckLibraryOrganization | null;
  snapshot: DeckLibrarySnapshot | null;
  isLoading: boolean;
  isMutating: boolean;
  errorMessage: string | null;
  announcement: string;
  sheet: LibrarySheetState | null;
  showArchived: boolean;
  collapsedFolderIds: ReadonlySet<string>;
  allFoldersCollapsed: boolean;
  selection: DeckSelectionState;
  selectedIds: ReadonlySet<string>;
  selectionAnalysis: DeckSelectionAnalysis;
  visibleDeckCount: number;
  currentScopeDeckCount: number;
  reloadOrganization(): Promise<DeckLibraryOrganization>;
  refreshLibraryAfterDetail(): Promise<void>;
  openDeck(deckId: string): void;
  openDeckCreator(): void;
  openFolderCreator(): void;
  openFolderRenamer(folderId: string): void;
  closeSheet(): void;
  createDeck(name: string, description: string): Promise<void>;
  saveFolder(name: string): Promise<void>;
  toggleDeckSelected(deck: DeckSummary): void;
  selectVisible(): void;
  selectCurrentScope(): void;
  clearSelection(): void;
  openSelectionMover(): void;
  moveSelection(targetFolderId: string | null): Promise<void>;
  setSelectionArchived(archived: boolean): Promise<void>;
  deleteSelection(): Promise<void>;
  removeDeck(deckId: string): Promise<void>;
  removeFolder(folderId: string): Promise<void>;
  move(intent: LibraryMoveIntent): Promise<void>;
  toggleFolder(folderId: string): void;
  collapseAllFolders(): void;
  toggleArchived(): void;
}

export function useDeckLibraryController({
  decks,
  refreshDecks,
  onOpenDeck,
  showToast,
  confirm,
  dependencies = defaultDependencies,
}: UseDeckLibraryControllerOptions): DeckLibraryController {
  const [organization, setOrganization] = useState<DeckLibraryOrganization | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isMutating, setIsMutating] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [announcement, setAnnouncement] = useState("");
  const [sheet, setSheet] = useState<LibrarySheetState | null>(null);
  const [showArchived, setShowArchived] = useState(false);
  const [collapsedFolderIds, setCollapsedFolderIds] = useState<Set<string>>(new Set());
  const [selection, setSelection] = useState<DeckSelectionState>(emptyDeckSelection);
  const organizationRef = useRef<DeckLibraryOrganization | null>(null);
  const mutationRef = useRef(false);
  const lifecycleSignatureRef = useRef<string | null>(null);
  const mountedRef = useRef(true);
  const deckControllerRef = useRef<DeckMutationController | null>(null);
  if (deckControllerRef.current === null) {
    deckControllerRef.current = dependencies.createDeckController();
  }
  const deckController = deckControllerRef.current;

  const applyOrganization = useCallback((next: DeckLibraryOrganization): void => {
    organizationRef.current = next;
    if (mountedRef.current) setOrganization(next);
  }, []);

  const reloadOrganization = useCallback(async (): Promise<DeckLibraryOrganization> => {
    const loaded = await dependencies.getOrganization();
    applyOrganization(loaded);
    if (mountedRef.current) setErrorMessage(null);
    return loaded;
  }, [applyOrganization, dependencies]);

  useEffect(() => {
    mountedRef.current = true;
    setIsLoading(true);
    void reloadOrganization()
      .catch((error: unknown) => {
        if (mountedRef.current) setErrorMessage(getErrorMessage(error));
      })
      .finally(() => {
        if (mountedRef.current) setIsLoading(false);
      });
    return () => {
      mountedRef.current = false;
      deckController.invalidate();
    };
  }, [deckController, reloadOrganization]);

  const lifecycleSignature = useMemo(() => deckLifecycleSignature(decks), [decks]);
  useEffect(() => {
    const previous = lifecycleSignatureRef.current;
    lifecycleSignatureRef.current = lifecycleSignature;
    if (previous === null || previous === lifecycleSignature) return;
    void reloadOrganization().catch((error: unknown) => {
      if (mountedRef.current) setErrorMessage(getErrorMessage(error));
    });
  }, [lifecycleSignature, reloadOrganization]);

  const snapshot = useMemo(
    () => (organization ? createDeckLibrarySnapshot(decks, organization) : null),
    [decks, organization],
  );
  const allFoldersCollapsed = Boolean(
    snapshot &&
      snapshot.folders.length > 0 &&
      snapshot.folders.every((folder) => collapsedFolderIds.has(folder.id)),
  );
  const currentScopeDecks = useMemo(
    () =>
      snapshot
        ? [
            ...snapshot.activeDecks,
            ...(showArchived ? snapshot.archivedDecks : []),
          ]
        : [],
    [showArchived, snapshot],
  );
  const visibleDecks = useMemo(() => {
    if (!snapshot) return [];
    const visibleActive = snapshot.folders.flatMap((folder) =>
      collapsedFolderIds.has(folder.id)
        ? []
        : [...(snapshot.decksByFolder.get(folder.id) ?? [])],
    );
    return [
      ...visibleActive,
      ...snapshot.unsortedDecks,
      ...(showArchived ? snapshot.archivedDecks : []),
    ];
  }, [collapsedFolderIds, showArchived, snapshot]);

  useEffect(() => {
    setSelection((current) => reconcileDeckSelection(current, currentScopeDecks));
  }, [currentScopeDecks]);

  const selectedIds = useMemo(() => deckSelectionIds(selection), [selection]);
  const selectionAnalysis = useMemo(
    () => analyzeDeckSelection(selection, decks),
    [decks, selection],
  );

  const beginMutation = useCallback((): boolean => {
    if (mutationRef.current) return false;
    mutationRef.current = true;
    if (mountedRef.current) setIsMutating(true);
    return true;
  }, []);

  const finishMutation = useCallback((): void => {
    mutationRef.current = false;
    if (mountedRef.current) setIsMutating(false);
  }, []);

  const recoverOrganizationAfterError = useCallback(
    async (error: unknown, fallback: DeckLibraryOrganization): Promise<void> => {
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
    },
    [applyOrganization, reloadOrganization, showToast],
  );

  const reconcileLibrary = useCallback(async (): Promise<string | null> => {
    const results = await Promise.allSettled([refreshDecks(), reloadOrganization()]);
    const failed = results.find((result) => result.status === "rejected");
    return failed?.status === "rejected" ? getErrorMessage(failed.reason) : null;
  }, [refreshDecks, reloadOrganization]);

  const refreshLibraryAfterDetail = useCallback(async (): Promise<void> => {
    const reconciliationError = await reconcileLibrary();
    if (reconciliationError) {
      showToast(
        "info",
        `The deck change was saved, but the library could not fully refresh: ${reconciliationError}`,
      );
    }
  }, [reconcileLibrary, showToast]);

  const openDeck = useCallback(
    (deckId: string): void => {
      setSheet(null);
      onOpenDeck(deckId);
    },
    [onOpenDeck],
  );

  const selectionIsCurrent = useCallback(
    (analysis: DeckSelectionAnalysis): boolean => {
      if (analysis.missingIds.length > 0) {
        showToast("info", "Some selected decks are no longer in this library scope. The selection was refreshed; review it and try again.");
        setSelection((current) => reconcileDeckSelection(current, currentScopeDecks));
        return false;
      }
      if (analysis.staleIds.length > 0) {
        showToast(
          "info",
          `${analysis.staleIds.length} selected ${analysis.staleIds.length === 1 ? "deck has" : "decks have"} changed since selection. Clear and reselect before changing them.`,
        );
        return false;
      }
      return true;
    },
    [currentScopeDecks, showToast],
  );

  const createDeck = useCallback(
    async (name: string, description: string): Promise<void> => {
      if (!beginMutation()) return;
      try {
        const deck = await deckController.create({ name, description });
        if (mountedRef.current) setSheet(null);
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
        finishMutation();
      }
    },
    [
      beginMutation,
      deckController,
      finishMutation,
      openDeck,
      refreshDecks,
      reloadOrganization,
      showToast,
    ],
  );

  const removeDeck = useCallback(
    async (deckId: string): Promise<void> => {
      const deck = decks.find((candidate) => candidate.id === deckId);
      if (!deck || mutationRef.current) return;
      const accepted = await confirm({
        title: `Delete “${deck.name}”?`,
        description: `This will delete ${deck.generatedCardCount} ${
          deck.generatedCardCount === 1 ? "card" : "cards"
        } and their review history after the Undo period.`,
        confirmLabel: "Delete deck",
        destructive: true,
      });
      if (!accepted || !beginMutation()) return;
      try {
        const deletion = await deckController.deleteDeck(deck.id);
        showToast("success", `Deleted “${deck.name}”.`, {
          label: "Undo",
          expiresAt: deletion.undoExpiresAt,
          run: async () => {
            try {
              await deckController.undo(deletion.undoToken);
              await refreshLibraryAfterDetail();
            } catch (error: unknown) {
              showToast("error", getErrorMessage(error));
              throw error;
            }
          },
        });
        await refreshLibraryAfterDetail();
      } catch (error: unknown) {
        showToast("error", getErrorMessage(error));
      } finally {
        finishMutation();
      }
    },
    [
      beginMutation,
      confirm,
      deckController,
      decks,
      finishMutation,
      refreshLibraryAfterDetail,
      showToast,
    ],
  );

  const openSelectionMover = useCallback((): void => {
    const analysis = analyzeDeckSelection(selection, decks);
    if (!selectionIsCurrent(analysis)) return;
    if (analysis.selectedIds.length === 0) return;
    if (analysis.archivedIds.length > 0) {
      showToast(
        "info",
        analysis.isMixedLifecycle
          ? "Move is available only when every selected deck is active. Archived decks remain selected so you can restore or clear them."
          : "Archived decks must be restored before they can be moved.",
      );
      return;
    }
    setSheet({ kind: "move-selection" });
  }, [decks, selection, selectionIsCurrent, showToast]);

  const moveSelection = useCallback(
    async (targetFolderId: string | null): Promise<void> => {
      const previous = organizationRef.current;
      const analysis = analyzeDeckSelection(selection, decks);
      if (
        !previous ||
        analysis.selectedIds.length === 0 ||
        analysis.archivedIds.length > 0 ||
        !selectionIsCurrent(analysis) ||
        !beginMutation()
      ) {
        return;
      }
      const selected = new Set(analysis.activeIds);
      const startingSnapshot = createDeckLibrarySnapshot(decks, previous);
      const orderedIds = [
        ...startingSnapshot.folders.flatMap((folder) =>
          startingSnapshot.decksByFolder.get(folder.id) ?? [],
        ),
        ...startingSnapshot.unsortedDecks,
      ]
        .filter((deck) => selected.has(deck.id))
        .map((deck) => deck.id);
      let currentOrganization = previous;
      let reconciliationError: string | null = null;
      try {
        const outcome = await runSequentialDeckMutations(
          orderedIds,
          "stop-after-failure",
          async (deckId) => {
            const currentSnapshot = createDeckLibrarySnapshot(decks, currentOrganization);
            const targetCount =
              targetFolderId === null
                ? currentSnapshot.unsortedDecks.filter((deck) => deck.id !== deckId).length
                : (currentSnapshot.decksByFolder.get(targetFolderId) ?? []).filter(
                    (deck) => deck.id !== deckId,
                  ).length;
            const validation = validateMoveIntent(currentSnapshot, {
              kind: "deck",
              deckId,
              targetFolderId,
              targetIndex: targetCount,
              source: "fallback",
            });
            if (validation.status === "invalid") throw new Error(validation.reason);
            if (validation.status === "noop") return currentOrganization;
            const updated = await dependencies.moveDeck({
              deckId,
              targetFolderId,
              targetIndex: validation.intent.targetIndex,
              expectedOrganizationRevision: currentOrganization.revision,
            });
            currentOrganization = updated;
            applyOrganization(updated);
            return updated;
          },
        );
        if (outcome.failed.length > 0) {
          try {
            await reloadOrganization();
          } catch (error: unknown) {
            reconciliationError = getErrorMessage(error);
            applyOrganization(currentOrganization);
          }
        }
        if (mountedRef.current) {
          setSheet(null);
          setAnnouncement(bulkOutcomeText("Moved", outcome, decks));
        }
        showToast(
          outcome.failed.length > 0 || reconciliationError ? "info" : "success",
          appendReconciliationError(bulkOutcomeText("Moved", outcome, decks), reconciliationError),
        );
      } finally {
        finishMutation();
      }
    },
    [
      applyOrganization,
      beginMutation,
      decks,
      dependencies,
      finishMutation,
      reloadOrganization,
      selection,
      selectionIsCurrent,
      showToast,
    ],
  );

  const setSelectionArchived = useCallback(
    async (archived: boolean): Promise<void> => {
      const analysis = analyzeDeckSelection(selection, decks);
      if (!selectionIsCurrent(analysis)) return;
      const targetIds = (archived ? analysis.activeIds : analysis.archivedIds).sort();
      if (mutationRef.current) return;
      if (targetIds.length === 0) {
        showToast(
          "info",
          archived
            ? "No active selected decks can be archived."
            : "No archived selected decks can be restored.",
        );
        return;
      }
      const unchangedCount = analysis.selectedIds.length - targetIds.length;
      const accepted = await confirm({
        title: `${archived ? "Archive" : "Restore"} ${targetIds.length} selected ${targetIds.length === 1 ? "deck" : "decks"}?`,
        description: archived
          ? `Each successful archive suspends that deck’s generated cards. This is not atomic; failures do not roll back earlier decks.${unchangedCount > 0 ? ` ${unchangedCount} already archived selected ${unchangedCount === 1 ? "deck remains" : "decks remain"} unchanged.` : ""}`
          : `Each successful restore returns that deck to the active library. This is not atomic; failures do not roll back earlier decks.${unchangedCount > 0 ? ` ${unchangedCount} active selected ${unchangedCount === 1 ? "deck remains" : "decks remain"} unchanged.` : ""}`,
        confirmLabel: archived ? "Archive selected" : "Restore selected",
      });
      if (!accepted || !beginMutation()) return;
      try {
        const deckById = new Map(analysis.decks.map((deck) => [deck.id, deck]));
        const outcome = await runSequentialDeckMutations(
          targetIds,
          "continue-after-failure",
          (deckId) => {
            const deck = deckById.get(deckId);
            if (!deck) throw new Error(`Selected deck is unavailable: ${deckId}`);
            return deckController.setArchived({
              deckId,
              revision: deck.revision,
              archived,
            });
          },
        );
        setSelection((current) =>
          removeDecksFromSelection(
            current,
            outcome.successful.map(({ deckId }) => deckId),
          ),
        );
        const reconciliationError =
          outcome.successful.length > 0 ? await reconcileLibrary() : null;
        const verb = archived ? "Archived" : "Restored";
        const message = appendReconciliationError(
          bulkOutcomeText(verb, outcome, decks),
          reconciliationError,
        );
        if (mountedRef.current) setAnnouncement(message);
        showToast(
          outcome.failed.length > 0 || reconciliationError ? "info" : "success",
          message,
        );
      } finally {
        finishMutation();
      }
    },
    [
      beginMutation,
      confirm,
      deckController,
      decks,
      finishMutation,
      reconcileLibrary,
      selection,
      selectionIsCurrent,
      showToast,
    ],
  );

  const deleteSelection = useCallback(async (): Promise<void> => {
    const analysis = analyzeDeckSelection(selection, decks);
    if (!selectionIsCurrent(analysis) || analysis.selectedIds.length === 0 || mutationRef.current) {
      return;
    }
    const totalCards = analysis.decks.reduce(
      (total, deck) => total + deck.generatedCardCount,
      0,
    );
    const accepted = await confirm({
      title: `Delete ${analysis.selectedIds.length} selected ${analysis.selectedIds.length === 1 ? "deck" : "decks"}?`,
      description: `Selection: ${analysis.activeIds.length} active, ${analysis.archivedIds.length} archived, ${analysis.emptyIds.length} empty, and ${analysis.generatedIds.length} with generated cards. This will delete ${totalCards} ${totalCards === 1 ? "card" : "cards"} plus review history after each deck’s Undo period. Deletions are sequential, not atomic; failures do not roll back earlier decks.`,
      confirmLabel: "Delete selected",
      destructive: true,
    });
    if (!accepted || !beginMutation()) return;
    try {
      const targetIds = [...analysis.selectedIds].sort();
      const outcome = await runSequentialDeckMutations(
        targetIds,
        "continue-after-failure",
        (deckId) => deckController.deleteDeck(deckId),
      );
      setSelection((current) =>
        removeDecksFromSelection(
          current,
          outcome.successful.map(({ deckId }) => deckId),
        ),
      );
      const reconciliationError =
        outcome.successful.length > 0 ? await reconcileLibrary() : null;
      const message = appendReconciliationError(
        bulkOutcomeText("Deleted", outcome, decks),
        reconciliationError,
      );
      const successfulDeletions = outcome.successful;
      const undoExpiresAt = successfulDeletions.reduce(
        (earliest, success) => Math.min(earliest, success.value.undoExpiresAt),
        Number.POSITIVE_INFINITY,
      );
      const pendingUndoById = new Map(
        successfulDeletions.map(({ deckId, value }) => [deckId, value]),
      );
      const undoAll = async (): Promise<void> => {
        const undoOutcome = await runSequentialDeckMutations(
          [...pendingUndoById.keys()],
          "continue-after-failure",
          (deckId) => deckController.undo(pendingUndoById.get(deckId)?.undoToken ?? ""),
        );
        for (const { deckId } of undoOutcome.successful) pendingUndoById.delete(deckId);
        const undoReconciliationError =
          undoOutcome.successful.length > 0 ? await reconcileLibrary() : null;
        const undoMessage = appendReconciliationError(
          bulkOutcomeText("Restored", undoOutcome, decks),
          undoReconciliationError,
        );
        if (undoOutcome.failed.length > 0) {
          showToast("error", undoMessage, {
            label: "Retry remaining undo",
            expiresAt: earliestUndoExpiry(pendingUndoById.values()),
            run: undoAll,
          });
          throw new Error("One or more deleted decks could not be restored.");
        }
        if (undoReconciliationError) {
          showToast("info", undoMessage);
          throw new Error("Restored decks could not be reconciled in the current view.");
        }
      };
      if (mountedRef.current) setAnnouncement(message);
      showToast(
        outcome.failed.length > 0 || reconciliationError ? "info" : "success",
        message,
        successfulDeletions.length > 0
          ? {
              label: "Undo all",
              expiresAt: undoExpiresAt,
              run: undoAll,
            }
          : undefined,
      );
    } finally {
      finishMutation();
    }
  }, [
    beginMutation,
    confirm,
    deckController,
    decks,
    finishMutation,
    reconcileLibrary,
    selection,
    selectionIsCurrent,
    showToast,
  ]);

  const saveFolder = useCallback(
    async (name: string): Promise<void> => {
      const previous = organizationRef.current;
      const currentSheet = sheet;
      if (
        !previous ||
        !currentSheet ||
        currentSheet.kind === "create-deck" ||
        currentSheet.kind === "move-selection" ||
        !beginMutation()
      ) {
        return;
      }
      try {
        const updated =
          currentSheet.kind === "create-folder"
            ? await dependencies.createFolder({
                name,
                expectedOrganizationRevision: previous.revision,
              })
            : await dependencies.updateFolder({
                folderId: currentSheet.folder.id,
                name,
                revision: currentSheet.folder.revision,
                expectedOrganizationRevision: previous.revision,
              });
        applyOrganization(updated);
        if (mountedRef.current) {
          setSheet(null);
          setAnnouncement(
            currentSheet.kind === "create-folder"
              ? `Created folder ${name}.`
              : `Renamed folder to ${name}.`,
          );
        }
      } catch (error: unknown) {
        await recoverOrganizationAfterError(error, previous);
      } finally {
        finishMutation();
      }
    },
    [
      applyOrganization,
      beginMutation,
      dependencies,
      finishMutation,
      recoverOrganizationAfterError,
      sheet,
    ],
  );

  const removeFolder = useCallback(
    async (folderId: string): Promise<void> => {
      const folder = snapshot?.folders.find((candidate) => candidate.id === folderId);
      if (!folder || mutationRef.current) return;
      const accepted = await confirm({
        title: `Delete “${folder.name}”?`,
        description:
          "The folder will be deleted and its decks will move to Unsorted decks. Cards, schedules, and review history will remain unchanged.",
        confirmLabel: "Delete folder",
        destructive: true,
      });
      const previous = organizationRef.current;
      if (!accepted || !previous || !beginMutation()) return;
      try {
        const updated = await dependencies.deleteFolder({
          folderId: folder.id,
          revision: folder.revision,
          expectedOrganizationRevision: previous.revision,
        });
        applyOrganization(updated);
        if (mountedRef.current) {
          setCollapsedFolderIds((current) => {
            const next = new Set(current);
            next.delete(folder.id);
            return next;
          });
          setAnnouncement(`Deleted folder ${folder.name}. Its decks moved to Unsorted decks.`);
        }
      } catch (error: unknown) {
        await recoverOrganizationAfterError(error, previous);
      } finally {
        finishMutation();
      }
    },
    [
      applyOrganization,
      beginMutation,
      confirm,
      dependencies,
      finishMutation,
      recoverOrganizationAfterError,
      snapshot?.folders,
    ],
  );

  const move = useCallback(
    async (requestedIntent: LibraryMoveIntent): Promise<void> => {
      const previous = organizationRef.current;
      if (!previous || mutationRef.current) return;
      const currentSnapshot = createDeckLibrarySnapshot(decks, previous);
      const validation = validateMoveIntent(currentSnapshot, requestedIntent);
      if (validation.status === "invalid") {
        showToast("info", "The deck library changed. Refreshing its organization now.");
        try {
          await reloadOrganization();
        } catch (error: unknown) {
          if (mountedRef.current) setErrorMessage(getErrorMessage(error));
        }
        return;
      }
      if (validation.status === "noop" || !beginMutation()) return;
      const intent = validation.intent;
      setOrganization(applyOptimisticMove(previous, intent));
      try {
        if (intent.kind === "deck") {
          const updated = await dependencies.moveDeck({
            deckId: intent.deckId,
            targetFolderId: intent.targetFolderId,
            targetIndex: intent.targetIndex,
            expectedOrganizationRevision: previous.revision,
          });
          applyOrganization(updated);
          const deck = decks.find((candidate) => candidate.id === intent.deckId);
          const folderName =
            intent.targetFolderId === null
              ? "Unsorted decks"
              : updated.folders.find((folder) => folder.id === intent.targetFolderId)?.name ??
                "folder";
          if (mountedRef.current) {
            setAnnouncement(
              `Moved ${deck?.name ?? "deck"} to ${folderName}, position ${intent.targetIndex + 1}.`,
            );
          }
        } else {
          const updated = await dependencies.moveFolder({
            folderId: intent.folderId,
            targetIndex: intent.targetIndex,
            expectedOrganizationRevision: previous.revision,
          });
          applyOrganization(updated);
          const folder = currentSnapshot.folders.find(
            (candidate) => candidate.id === intent.folderId,
          );
          if (mountedRef.current) {
            setAnnouncement(
              `Moved folder ${folder?.name ?? "folder"} to position ${intent.targetIndex + 1}.`,
            );
          }
        }
      } catch (error: unknown) {
        await recoverOrganizationAfterError(error, previous);
      } finally {
        finishMutation();
      }
    },
    [
      applyOrganization,
      beginMutation,
      decks,
      dependencies,
      finishMutation,
      recoverOrganizationAfterError,
      reloadOrganization,
      showToast,
    ],
  );

  const openFolderRenamer = useCallback(
    (folderId: string): void => {
      const folder = snapshot?.folders.find((candidate) => candidate.id === folderId);
      if (folder) setSheet({ kind: "rename-folder", folder, name: folder.name });
    },
    [snapshot?.folders],
  );

  return {
    organization,
    snapshot,
    isLoading,
    isMutating,
    errorMessage,
    announcement,
    sheet,
    showArchived,
    collapsedFolderIds,
    allFoldersCollapsed,
    selection,
    selectedIds,
    selectionAnalysis,
    visibleDeckCount: visibleDecks.length,
    currentScopeDeckCount: currentScopeDecks.length,
    reloadOrganization,
    refreshLibraryAfterDetail,
    openDeck,
    openDeckCreator: () => setSheet({ kind: "create-deck" }),
    openFolderCreator: () => setSheet({ kind: "create-folder", name: "" }),
    openFolderRenamer,
    closeSheet: () => setSheet(null),
    createDeck,
    saveFolder,
    toggleDeckSelected: (deck) =>
      setSelection((current) => toggleDeckSelection(current, deck)),
    selectVisible: () =>
      setSelection((current) => addDecksToSelection(current, visibleDecks)),
    selectCurrentScope: () =>
      setSelection((current) => addDecksToSelection(current, currentScopeDecks)),
    clearSelection: () => setSelection(clearDeckSelection()),
    openSelectionMover,
    moveSelection,
    setSelectionArchived,
    deleteSelection,
    removeDeck,
    removeFolder,
    move,
    toggleFolder: (folderId: string) => {
      setCollapsedFolderIds((current) => {
        const next = new Set(current);
        if (next.has(folderId)) next.delete(folderId);
        else next.add(folderId);
        return next;
      });
    },
    collapseAllFolders: () =>
      setCollapsedFolderIds(new Set(snapshot?.folders.map((folder) => folder.id) ?? [])),
    toggleArchived: () => setShowArchived((current) => !current),
  };
}

function earliestUndoExpiry(
  deletions: Iterable<{ undoExpiresAt: number }>,
): number {
  let earliest = Number.POSITIVE_INFINITY;
  for (const deletion of deletions) earliest = Math.min(earliest, deletion.undoExpiresAt);
  return earliest;
}

function bulkOutcomeText<T>(
  verb: string,
  outcome: SequentialDeckMutationOutcome<T>,
  decks: readonly DeckSummary[],
): string {
  const deckById = new Map(decks.map((deck) => [deck.id, deck]));
  const labels = (ids: readonly string[]) =>
    ids
      .map((id) => `${deckById.get(id)?.name ?? "Unknown deck"} (${id})`)
      .join(", ");
  const sentences = [
    `${verb} ${outcome.successful.length} of ${outcome.requestedIds.length} selected ${outcome.requestedIds.length === 1 ? "deck" : "decks"}.`,
  ];
  if (outcome.successful.length > 0) {
    sentences.push(`Succeeded: ${labels(outcome.successful.map(({ deckId }) => deckId))}.`);
  }
  if (outcome.failed.length > 0) {
    sentences.push(
      `Failed: ${outcome.failed
        .map(
          ({ deckId, error }) =>
            `${labels([deckId])}: ${getErrorMessage(error)}`,
        )
        .join(", ")}.`,
    );
  }
  if (outcome.unattemptedIds.length > 0) {
    sentences.push(`Not attempted: ${labels(outcome.unattemptedIds)}.`);
  }
  return sentences.join(" ");
}

function appendReconciliationError(
  outcomeMessage: string,
  reconciliationError: string | null,
): string {
  return reconciliationError
    ? `${outcomeMessage} The committed changes remain saved, but the library could not fully refresh: ${reconciliationError}`
    : outcomeMessage;
}
