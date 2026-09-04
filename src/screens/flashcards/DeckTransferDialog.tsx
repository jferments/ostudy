import { useEffect, useRef, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import {
  DeckTransferController,
  type DeckImportOutcome,
} from "../../controllers/deckTransferController";
import { getErrorMessage } from "../../ipc/common";
import type { DeckSummary } from "../../ipc/decks";
import { usePlatformCapabilities } from "../../platform/PlatformProvider";
import { Icon } from "../../ui/icons/Icon";
import { Dialog } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import styles from "./DeckTransferDialog.module.css";

export type DeckTransferDialogRequest =
  | { kind: "import" }
  | { kind: "export"; decks: DeckSummary[] };

interface DeckTransferDialogProps {
  request: DeckTransferDialogRequest;
  refreshDecks(): Promise<DeckSummary[]>;
  onImportedDeck?(outcome: DeckImportOutcome): void | Promise<void>;
  onClose(): void;
  showToast: ShowToast;
}

export function DeckTransferDialog({
  request,
  refreshDecks,
  onImportedDeck,
  onClose,
  showToast,
}: DeckTransferDialogProps) {
  const controllerRef = useRef<DeckTransferController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new DeckTransferController();
  const controller = controllerRef.current;
  const capabilities = usePlatformCapabilities();
  const [busy, setBusy] = useState<"import" | "export" | "share" | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const exportDecks = request.kind === "export" ? request.decks : [];
  const canShare =
    exportDecks.length === 1 &&
    capabilities.native?.services.systemShare.state === "available";

  useEffect(() => () => controller.invalidate(), [controller]);

  async function importDeck(): Promise<void> {
    if (busy) return;
    setBusy("import");
    setErrorMessage(null);
    try {
      const outcome = await controller.importFromFile();
      if (!outcome) return;
      try {
        await refreshDecks();
      } catch (error: unknown) {
        showToast(
          "info",
          `${outcome.message} The library could not refresh immediately: ${getErrorMessage(error)}`,
        );
      }
      showToast("success", outcome.message);
      onClose();
      await onImportedDeck?.(outcome);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusy(null);
    }
  }

  async function saveDecks(): Promise<void> {
    if (busy || exportDecks.length === 0) return;
    setBusy("export");
    setErrorMessage(null);
    try {
      const outcome = await controller.exportDecks(exportDecks);
      if (outcome.cancelled) {
        if (outcome.exported.length > 0) {
          showToast(
            "info",
            `Saved ${outcome.exported.length} of ${outcome.requestedCount} deck files before export was canceled.`,
          );
        }
        onClose();
        return;
      }
      const totalCards = outcome.exported.reduce(
        (sum, exported) => sum + exported.cardCount,
        0,
      );
      showToast(
        "success",
        outcome.exported.length === 1
          ? `Exported ${outcome.exported[0].receipt.displayName}.`
          : `Exported ${outcome.exported.length} deck files with ${totalCards} total cards.`,
      );
      onClose();
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusy(null);
    }
  }

  async function shareDeck(): Promise<void> {
    const deck = exportDecks[0];
    if (!deck || busy) return;
    setBusy("share");
    setErrorMessage(null);
    try {
      const outcome = await controller.shareDeck(deck);
      if (!outcome) return;
      showToast(
        "info",
        `Opened system sharing for “${deck.name}” with ${outcome.cardCount} ${
          outcome.cardCount === 1 ? "card" : "cards"
        }.`,
      );
      onClose();
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setBusy(null);
    }
  }

  const importing = request.kind === "import";
  const title = importing
    ? "Import deck from file"
    : exportDecks.length === 1
      ? `Export “${exportDecks[0]?.name ?? "deck"}”`
      : `Export ${exportDecks.length} selected decks`;
  const description = importing
    ? "Import an ostudy .studydeck backup or generated-card JSON file into this library."
    : exportDecks.length === 1
      ? "Save a portable .studydeck file, or share it through the operating system when available."
      : "Each selected deck is saved as its own portable .studydeck file.";

  return (
    <Dialog
      title={title}
      description={description}
      width="medium"
      busy={busy !== null}
      dismissible={busy === null}
      onClose={onClose}
    >
      <div className={styles.content}>
        {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}

        {importing ? (
          <div className={styles.callout}>
            <span className={styles.icon} aria-hidden="true">
              <Icon name="folder-open" size={28} />
            </span>
            <div>
              <strong>Choose a flashcard file</strong>
              <p>
                Supported imports include complete ostudy deck backups and validated generated-card
                JSON files.
              </p>
            </div>
          </div>
        ) : (
          <div className={styles.deckList} aria-label="Decks to export">
            {exportDecks.map((deck) => (
              <div key={deck.id} className={styles.deckRow}>
                <Icon name="cards" size={20} />
                <span>
                  <strong>{deck.name}</strong>
                  <small>
                    {deck.generatedCardCount} {deck.generatedCardCount === 1 ? "card" : "cards"}
                  </small>
                </span>
              </div>
            ))}
          </div>
        )}

        <div className={styles.actions}>
          <Button variant="ghost" disabled={busy !== null} onClick={onClose}>
            Cancel
          </Button>
          {importing ? (
            <Button
              variant="primary"
              loading={busy === "import"}
              onClick={() => void importDeck()}
            >
              <Icon name="folder-open" size={18} /> Choose file
            </Button>
          ) : (
            <>
              {canShare ? (
                <Button
                  variant="secondary"
                  loading={busy === "share"}
                  disabled={busy !== null}
                  onClick={() => void shareDeck()}
                >
                  <Icon name="export" size={18} /> Share
                </Button>
              ) : null}
              <Button
                variant="primary"
                loading={busy === "export"}
                disabled={busy !== null || exportDecks.length === 0}
                onClick={() => void saveDecks()}
              >
                <Icon name="save" size={18} />
                {exportDecks.length === 1 ? "Save file" : "Save deck files"}
              </Button>
            </>
          )}
        </div>
      </div>
    </Dialog>
  );
}
