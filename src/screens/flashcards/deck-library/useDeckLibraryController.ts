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
  type DeckFolder,
  type DeckLibraryOrganization,
} from "../../../ipc/deckFolders";
import { useConfirm } from "../../../ui/overlays/overlayContext";
import {
  applyOptimisticLibraryMove,
  deckLifecycleSignature,
  isNoopLibraryMove,
  normalizeLibraryMoveIntent,
  partitionDecksByLifecycle,
} from "./organizationModel";
import type { FolderDraft, LibraryMoveIntent } from "./types";

interface UseDeckLibraryControllerOptions {
  decks: DeckSummary[];
  refreshDecks: () => Promise<DeckSummary[]>;
  onOpenDeck: (deckId: string) => void;
  showToast: ShowToast;
}

export function useDeckLibraryController({
  decks,
  refreshDecks,
  onOpenDeck,
  showToast,
}: UseDeckLibraryControllerOptions) {
  const [organization, setOrganization] = useState<DeckLibraryOrganization | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [folderDraft, setFolderDraft] = useState<FolderDraft | null>(null);
  const [isMutating, setIsMutating] = useState(false);
  const [announcement, setAnnouncement] = useState("");
  const [showArchived, setShowArchived] = useState(false);
  const [collapsedFolderIds, setCollapsedFolderIds] = useState<Set<string>>(new Set());
  const organizationRef = useRef<DeckLibraryOrganization | null>(null);
  const mutationRef = useRef(false);
  const lastLifecycleSignatureRef = useRef<string | null>(null);
  const decksControllerRef = useRef<DecksController | null>(null);
  if (decksControllerRef.current === null) decksControllerRef.current = new DecksController();
  const decksController = decksControllerRef.current;
  const confirm = useConfirm();
  const { activeDecks, archivedDecks } = useMemo(
    () => partitionDecksByLifecycle(decks),
    [decks],
  );
  const lifecycleSignature = useMemo(() => deckLifecycleSignature(decks), [decks]);

  const applyOrganization = useCallback((next: DeckLibraryOrganization): void => {
    organizationRef.current = next;
    setOrganization(next);
  }, []);

  const reloadOrganization = useCallback(async (): Promise<DeckLibraryOrganization> => {
    const loaded = await getDeckLibraryOrganization();
    applyOrganization(loaded);
    setErrorMessage(null);
    return loaded;
  }, [applyOrganization]);

  useEffect(() => {
    let active = true;
    setIsLoading(true);
    void getDeckLibraryOrganization()
      .then((loaded) => {
        if (!active) return;
        applyOrganization(loaded);
        setErrorMessage(null);
      })
      .catch((error: unknown) => {
        if (active) setErrorMessage(getErrorMessage(error));
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
      decksController.invalidate();
    };
  }, [applyOrganization, decksController]);

  useEffect(() => {
    const previous = lastLifecycleSignatureRef.current;
    lastLifecycleSignatureRef.current = lifecycleSignature;
    if (previous === null || previous === lifecycleSignature) return;
    void reloadOrganization().catch((error: unknown) => {
      setErrorMessage(getErrorMessage(error));
    });
  }, [lifecycleSignature, reloadOrganization]);

  const beginMutation = useCallback((): boolean => {
    if (mutationRef.current) return false;
    mutationRef.current = true;
    setIsMutating(true);
    return true;
  }, []);

  const finishMutation = useCallback((): void => {
    mutationRef.current = false;
    setIsMutating(false);
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

  const refreshLibrary = useCallback(async (): Promise<void> => {
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
      setFolderDraft(null);
      onOpenDeck(deckId);
    },
    [onOpenDeck],
  );

  const createDeck = useCallback(
    async (name: string, description: string): Promise<void> => {
      if (!beginMutation()) return;
      try {
        const deck = await decksController.create({ name, description });
        setFolderDraft(null);
        const results = await Promise.allSettled([refreshDecks(), reloadOrganization()]);
        const failed = results.find((result) => result.status === "rejected");
        showToast(
          failed ? "info" : "success",
          failed
            ? `Created “${deck.name}”, but the library could not fully refresh: ${getErrorMessage(failed.status === "rejected" ? failed.reason : "")}`
            : `Created “${deck.name}”.`,
        );
        openDeck(deck.id);
      } catch (error: unknown) {
        showToast("error", getErrorMessage(error));
      } finally {
        finishMutation();
      }
    },
    [
      beginMutation,
      decksController,
      finishMutation,
      openDeck,
      refreshDecks,
      reloadOrganization,
      showToast,
    ],
  );

  const removeDeck = useCallback(
    async (deck: DeckSummary): Promise<void> => {
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
        const deletion = await decksController.deleteDeck(deck.id);
        showToast("success", `Deleted “${deck.name}”.`, {
          label: "Undo",
          expiresAt: deletion.undoExpiresAt,
          run: async () => {
            try {
              await decksController.undo(deletion.undoToken);
              await refreshLibrary();
            } catch (error: unknown) {
              showToast("error", getErrorMessage(error));
              throw error;
            }
          },
        });
        await refreshLibrary();
      } catch (error: unknown) {
        showToast("error", getErrorMessage(error));
      } finally {
        finishMutation();
      }
    },
    [beginMutation, confirm, decksController, finishMutation, refreshLibrary, showToast],
  );

  const saveFolder = useCallback(
    async (name: string): Promise<void> => {
      const previous = organizationRef.current;
      const draft = folderDraft;
      if (!previous || !draft || !beginMutation()) return;
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
        finishMutation();
      }
    },
    [
      applyOrganization,
      beginMutation,
      finishMutation,
      folderDraft,
      recoverOrganizationAfterError,
    ],
  );

  const removeFolder = useCallback(
    async (folder: DeckFolder): Promise<void> => {
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
        finishMutation();
      }
    },
    [
      applyOrganization,
      beginMutation,
      confirm,
      finishMutation,
      recoverOrganizationAfterError,
    ],
  );

  const move = useCallback(
    async (intent: LibraryMoveIntent): Promise<void> => {
      const previous = organizationRef.current;
      if (!previous || mutationRef.current) return;
      const normalized = normalizeLibraryMoveIntent(previous, intent);
      if (isNoopLibraryMove(previous, activeDecks, normalized) || !beginMutation()) return;

      setOrganization(applyOptimisticLibraryMove(previous, normalized));
      try {
        const updated =
          normalized.kind === "deck"
            ? await moveDeckInLibrary({
                deckId: normalized.deckId,
                targetFolderId: normalized.targetFolderId,
                targetIndex: normalized.targetIndex,
                expectedOrganizationRevision: previous.revision,
              })
            : await moveDeckFolder({
                folderId: normalized.folderId,
                targetIndex: normalized.targetIndex,
                expectedOrganizationRevision: previous.revision,
              });
        applyOrganization(updated);
        if (normalized.kind === "deck") {
          const deck = decks.find((candidate) => candidate.id === normalized.deckId);
          const folderName =
            normalized.targetFolderId === null
              ? "Unsorted decks"
              : updated.folders.find((folder) => folder.id === normalized.targetFolderId)?.name ??
                "folder";
          setAnnouncement(
            `Moved ${deck?.name ?? "deck"} to ${folderName}, position ${normalized.targetIndex + 1}.`,
          );
        } else {
          const folder = previous.folders.find(
            (candidate) => candidate.id === normalized.folderId,
          );
          setAnnouncement(
            `Moved folder ${folder?.name ?? "folder"} to position ${normalized.targetIndex + 1}.`,
          );
        }
      } catch (error: unknown) {
        await recoverOrganizationAfterError(error, previous);
      } finally {
        finishMutation();
      }
    },
    [
      activeDecks,
      applyOrganization,
      beginMutation,
      decks,
      finishMutation,
      recoverOrganizationAfterError,
    ],
  );

  const toggleFolder = useCallback((folderId: string): void => {
    setCollapsedFolderIds((current) => {
      const next = new Set(current);
      if (next.has(folderId)) next.delete(folderId);
      else next.add(folderId);
      return next;
    });
  }, []);

  const collapseAllFolders = useCallback((): void => {
    setCollapsedFolderIds(
      new Set((organizationRef.current?.folders ?? []).map((folder) => folder.id)),
    );
  }, []);

  return {
    organization,
    isLoading,
    errorMessage,
    folderDraft,
    isMutating,
    announcement,
    showArchived,
    collapsedFolderIds,
    activeDecks,
    archivedDecks,
    setFolderDraft,
    setShowArchived,
    toggleFolder,
    collapseAllFolders,
    openDeck,
    createDeck,
    removeDeck,
    saveFolder,
    removeFolder,
    move,
    refreshLibrary,
  };
}
