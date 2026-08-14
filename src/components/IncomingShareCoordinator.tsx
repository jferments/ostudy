import { useEffect, useRef, useState } from "react";

import type { ShowToast } from "./Toast";
import { prepareLibraryRestore } from "../ipc/backup";
import { createOperationId, getErrorMessage } from "../ipc/common";
import { importDeck } from "../ipc/decks";
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

/**
 * Converts Android ACTION_SEND intents into the same staged-import pipeline used
 * by the regular picker. Nothing is imported automatically: the sender's URI is
 * copied into app-private cache by Kotlin, then the user confirms adoption and
 * shared Rust performs content detection and validation.
 */
export function IncomingShareCoordinator({
  refreshDecks,
  showToast,
}: IncomingShareCoordinatorProps) {
  const capabilities = usePlatformCapabilities();
  const confirm = useConfirm();
  const [queue, setQueue] = useState<IncomingShareSummary[]>([]);
  const processing = useRef(false);
  const android = capabilities.native?.operatingSystem === "android";

  useEffect(() => {
    if (!android) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listIncomingShares()
      .then((items) => {
        if (!disposed) setQueue(items);
      })
      .catch((error: unknown) => {
        if (!disposed) console.error("Unable to list incoming Android shares:", getErrorMessage(error));
      });
    void listenToIncomingShare((incoming) => {
      if (disposed) return;
      setQueue((current) =>
        current.some((item) => item.token === incoming.token) ? current : [...current, incoming],
      );
    })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch((error: unknown) => {
        if (!disposed) {
          console.error("Unable to register the Android incoming-share listener:", getErrorMessage(error));
        }
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [android]);

  useEffect(() => {
    const incoming = queue[0];
    if (!incoming || processing.current) return;
    processing.current = true;
    void handleIncoming(incoming)
      .catch((error: unknown) => showToast("error", getErrorMessage(error)))
      .finally(() => {
        setQueue((current) => current.filter((item) => item.token !== incoming.token));
        processing.current = false;
      });
  }, [confirm, queue, refreshDecks, showToast]);

  async function handleIncoming(incoming: IncomingShareSummary): Promise<void> {
    const accepted = await confirm({
      title: "Import shared ostudy content?",
      description: `${incoming.displayName} (${formatBytes(incoming.byteSize)}) was shared with ostudy. The file will be validated before it changes your library.`,
      confirmLabel: "Import",
      cancelLabel: "Discard",
    });
    if (!accepted) {
      await dismissIncomingShare(incoming.token);
      return;
    }

    const staged = await stageIncomingShare(incoming.token);
    try {
      switch (staged.detectedKind) {
        case "studydeck": {
          const result = await importDeck(staged.id, createOperationId());
          await refreshDecks();
          showToast(
            "success",
            `Imported ${result.cardCount} card${result.cardCount === 1 ? "" : "s"} into ${result.deck.name}.`,
          );
          break;
        }
        case "generated_json": {
          const result = await importGeneratedFlashcards(staged.id);
          await refreshDecks();
          showToast(
            "success",
            `Imported ${result.createdCardCount} generated card${result.createdCardCount === 1 ? "" : "s"}.`,
          );
          break;
        }
        case "ostudybackup": {
          const result = await prepareLibraryRestore(staged.id);
          showToast(
            "info",
            result.restartRequired
              ? "Backup validated. Close and reopen ostudy to apply the restored library."
              : "Backup restored.",
          );
          break;
        }
        default:
          throw new Error("The shared file is not an ostudy deck, generated JSON file, or backup.");
      }
    } catch (error) {
      await releaseStagedExternalImport(staged.id).catch(() => undefined);
      throw error;
    }
  }

  return null;
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  if (value < 1024 * 1024 * 1024) return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
  return `${(value / (1024 * 1024 * 1024)).toFixed(1)} GiB`;
}
