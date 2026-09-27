import { useEffect, useRef, useState } from "react";

import { restoredLibraryMessage } from "./libraryRestoreResult";
import { refreshLibraryView } from "../platform/libraryView";
import type { ShowToast } from "./Toast";
import { prepareLibraryRestore } from "../ipc/backup";
import { createOperationId, getErrorMessage } from "../ipc/common";
import { importDeck } from "../ipc/decks";
import { queueFrontendDiagnostic, reportFrontendFailure } from "../ipc/diagnostics";
import {
  dismissIncomingShare,
  listIncomingShares,
  releaseStagedExternalImport,
  stageIncomingShare,
  type IncomingShareSummary,
} from "../ipc/externalFiles";
import { importGeneratedFlashcards } from "../ipc/generated-import";
import { listenToIncomingShare } from "../ipc/platform";
import { usePlatformCapabilities } from "../platform/PlatformProvider";
import { useConfirm } from "../ui/overlays/overlayContext";

interface IncomingShareCoordinatorProps {
  refreshDecks: () => Promise<unknown>;
  showToast: ShowToast;
}

/** ACTION_SEND acquisition is explicit and native-validated. Listener lifetime
 * controls admission, not completion of an already-submitted native mutation. */
export function IncomingShareCoordinator({ refreshDecks, showToast }: IncomingShareCoordinatorProps) {
  const capabilities = usePlatformCapabilities();
  const confirm = useConfirm();
  const [queue, setQueue] = useState<IncomingShareSummary[]>([]);
  const processing = useRef(false);
  const ownerRef = useRef<object | null>(null);
  const handledBeforeSnapshot = useRef(new Set<string>());
  const snapshotPending = useRef(false);
  const android = capabilities.native?.operatingSystem === "android";

  useEffect(() => {
    if (!android) return;
    const owner = {};
    ownerRef.current = owner;
    snapshotPending.current = true;
    handledBeforeSnapshot.current.clear();
    let unlisten: (() => void) | undefined;
    const current = () => ownerRef.current === owner;
    const disposeListener = (dispose: () => void) => {
      try { dispose(); }
      catch (error: unknown) { reportFrontendFailure("platform_listener_cleanup_failed", error); }
    };
    void listIncomingShares().then((items) => {
      if (!current()) return;
      const handled = new Set(handledBeforeSnapshot.current);
      setQueue((queued) => {
        const known = new Set(queued.map((item) => item.token));
        return [...queued, ...items.filter((item) => {
          if (known.has(item.token) || handled.has(item.token)) return false;
          known.add(item.token);
          return true;
        })];
      });
    }).catch((error: unknown) => {
      if (current()) reportFrontendFailure("incoming_share_list_failed", error);
    }).finally(() => {
      if (current()) {
        snapshotPending.current = false;
        handledBeforeSnapshot.current.clear();
      }
    });
    void listenToIncomingShare((incoming) => {
      if (!current()) return;
      setQueue((queued) => queued.some((item) => item.token === incoming.token)
        ? queued : [...queued, incoming]);
    }).then((dispose) => {
      if (!current()) disposeListener(dispose);
      else unlisten = dispose;
    }).catch((error: unknown) => {
      if (current()) reportFrontendFailure("incoming_share_listener_failed", error);
    });
    return () => {
      if (ownerRef.current === owner) ownerRef.current = null;
      if (unlisten) disposeListener(unlisten);
    };
  }, [android]);

  useEffect(() => {
    const incoming = queue[0];
    const owner = ownerRef.current;
    if (!android || !owner || !incoming || processing.current) return;
    processing.current = true;
    if (snapshotPending.current) handledBeforeSnapshot.current.add(incoming.token);
    const current = () => ownerRef.current === owner;
    const notify: ShowToast = (kind, message) => {
      if (!current()) return;
      try { showToast(kind, message); }
      catch {
        queueFrontendDiagnostic({ level: "warn", target: "frontend.share",
          event: "incoming_share_notification_failed", message: "The shared-content notification could not be displayed." });
      }
    };
    void handleIncoming(incoming, confirm, refreshDecks, notify, current)
      .catch((error: unknown) => notify("error", getErrorMessage(error)))
      .finally(() => {
        processing.current = false;
        if (current()) setQueue((queued) => queued.filter((item) => item.token !== incoming.token));
      });
  }, [android, confirm, queue, refreshDecks, showToast]);

  return null;
}

async function handleIncoming(
  incoming: IncomingShareSummary,
  confirm: ReturnType<typeof useConfirm>,
  refreshDecks: () => Promise<unknown>,
  notify: ShowToast,
  current: () => boolean,
): Promise<void> {
  const accepted = await confirm({
    title: "Import shared ostudy content?",
    description: `${incoming.displayName} (${formatBytes(incoming.byteSize)}) was shared with ostudy. The file will be validated before it changes your library.`,
    confirmLabel: "Import", cancelLabel: "Discard",
  });
  if (!current()) return;
  if (!accepted) { await dismissIncomingShare(incoming.token); return; }

  const staged = await stageIncomingShare(incoming.token);
  if (!current()) {
    await releaseStagedExternalImport(staged.id);
    return;
  }
  let committed = false;
  try {
    switch (staged.detectedKind) {
      case "studydeck": {
        const result = await importDeck(staged.id, createOperationId());
        committed = true;
        notify("success", `Imported ${result.cardCount} card${result.cardCount === 1 ? "" : "s"} into ${result.deck.name}.`);
        break;
      }
      case "generated_json": {
        const result = await importGeneratedFlashcards(staged.id);
        committed = true;
        notify("success", `Imported ${result.createdCardCount} generated card${result.createdCardCount === 1 ? "" : "s"}.`);
        break;
      }
      case "ostudybackup": {
        const replace = await confirm({
          title: "Replace this complete library?",
          description: "A complete backup replaces this library; it is not a deck merge. Back up anything you need to keep first. Documents, decks, schedules, reviews, and settings will be replaced after validation. Saved API keys are not changed.",
          confirmLabel: "Restore library", destructive: true,
        });
        if (!replace || !current()) { await releaseStagedExternalImport(staged.id); return; }
        const result = await prepareLibraryRestore(staged.id);
        committed = true;
        notify("success", restoredLibraryMessage(result));
        if (current()) refreshLibraryView();
        return;
      }
      default:
        throw new Error("The shared file is not an ostudy deck, generated JSON file, or backup.");
    }
    if (current()) await refreshDecks();
  } catch (error: unknown) {
    if (committed) {
      // A successfully committed import is not undone or retried by a view error.
      queueFrontendDiagnostic({ level: "warn", target: "frontend.share",
        event: "incoming_share_reconciliation_failed", message: "Shared content committed, but its view could not refresh." });
      notify("info", "The shared content was imported. Refresh the view to see it.");
      return;
    }
    // A failed complete restore may need this only local copy for recovery.
    if (staged.detectedKind !== "ostudybackup") {
      await releaseStagedExternalImport(staged.id).catch(() => undefined);
    }
    throw error;
  }
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  if (value < 1024 * 1024 * 1024) return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
  return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GiB`;
}
