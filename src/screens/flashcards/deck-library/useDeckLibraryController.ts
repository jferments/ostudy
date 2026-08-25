import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import type { ShowToast } from "../../../components/Toast";
import { DecksController } from "../../../controllers/decksController";
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
import type { LibraryMoveIntent, LibrarySheetState } from "./types";

interface DeckMutationController {
  create(input: { name: string; description: string }): Promise<DeckSummary>;
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
  reloadOrganization(): Promise<DeckLibraryOrganization>;
  refreshLibraryAfterDetail(): Promise<void>;
  openDeck(deckId: string): void;
  openDeckCreator(): void;
  openFolderCreator(): void;
  openFolderRenamer(folderId: string): void;
  closeSheet(): void;
  createDeck(name: string, description: string): Promise<void>;
  saveFolder(name: string): Promise<void>;
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

  const refreshLibraryAfterDetail = useCallback(async (): Promise<void> => {
    const results = await Promise.allSettled([refreshDecks(), reloadOrganization()]);
    const failed = results.find((result) => result.status === "rejected");
    if (failed?.status === "rejected") {
      showToast(
        "info",
        `The deck change was saved, but the library could not fully refresh: ${getErrorMessage(failed.reason)}`,
      );
    }
  }, [refreshDecks, reloadOrganization, showToast]);

  const openDeck = useCallback(
    (deckId: string): void => {
      setSheet(null);
      onOpenDeck(deckId);
    },
    [onOpenDeck],
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

  const saveFolder = useCallback(
    async (name: string): Promise<void> => {
      const previous = organizationRef.current;
      const currentSheet = sheet;
      if (
        !previous ||
        !currentSheet ||
        currentSheet.kind === "create-deck" ||
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
    reloadOrganization,
    refreshLibraryAfterDetail,
    openDeck,
    openDeckCreator: () => setSheet({ kind: "create-deck" }),
    openFolderCreator: () => setSheet({ kind: "create-folder", name: "" }),
    openFolderRenamer,
    closeSheet: () => setSheet(null),
    createDeck,
    saveFolder,
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
