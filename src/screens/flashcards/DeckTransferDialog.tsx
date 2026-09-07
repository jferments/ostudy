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
import { AnkiImportReportView } from "./AnkiImportReport";
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
  const [importedOutcome, setImportedOutcome] = useState<DeckImportOutcome | null>(null);
  const exportDecks = request.kind === "export" ? request.decks : [];
  const canShare =
    exportDecks.length === 1 &&
    capabilities.native?.services.systemShare.state === "available";

  useEffect(() => () => controller.invalidate(), [controller]);

  async function openImportedDeck(outcome: DeckImportOutcome): Promise<void> {
    onClose();
    try {
      await onImportedDeck?.(outcome);
    } catch (error: unknown) {
      // Navigation is after the durable import boundary, not a failed import.
      showToast("info", `The deck was imported but could not be opened: ${getErrorMessage(error)}`);
    }
  }

  async function importDeck(): Promise<void> {
    if (busy || importedOutcome) return;
    setBusy("import");
    setErrorMessage(null);
    try {
      const outcome = await controller.importFromFile();
      if (!outcome) return;
      let refreshError: string | null = null;
      try {
        await refreshDecks();
      } catch (error: unknown) {
        refreshError = getErrorMessage(error);
      }
      const message = refreshError
        ? `${outcome.message} The library could not refresh immediately: ${refreshError}`
        : outcome.message;
      showToast(refreshError ? "info" : "success", message);
      if (outcome.ankiReport) {
        // This is a post-commit report, never an import preview. Keep it visible
        // until the user closes it or chooses to open the new deck.
        setImportedOutcome({ ...outcome, message });
      } else {
        await openImportedDeck(outcome);
      }
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
  const title = importedOutcome
    ? "Anki import complete"
    : importing
      ? "Import deck from file"
      : exportDecks.length === 1
        ? `Export “${exportDecks[0]?.name ?? "deck"}”`
        : `Export ${exportDecks.length} selected decks`;
  const description = importedOutcome
    ? "The deck has been saved. Review conversion details before studying."
    : importing
      ? "Import an ostudy .studydeck file, a compatible Anki .apkg deck, or generated-card JSON."
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

        {importedOutcome?.ankiReport ? (
          <AnkiImportReportView
            report={importedOutcome.ankiReport}
            message={importedOutcome.message}
          />
        ) : importing ? (
          <div className={styles.callout}>
            <span className={styles.icon} aria-hidden="true">
              <Icon name="folder-open" size={28} />
            </span>
            <div>
              <strong>Choose a flashcard file</strong>
              <p>
                Import an ostudy deck, compatible Anki .apkg deck, or validated generated-card
                JSON file. For newer Anki exports, enable “Support older Anki versions” and
                “Include media” when exporting from Anki.
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
            {importedOutcome ? "Close" : "Cancel"}
          </Button>
          {importedOutcome ? (
            <Button variant="primary" onClick={() => void openImportedDeck(importedOutcome)}>
              {onImportedDeck ? "Open imported deck" : "Done"}
            </Button>
          ) : importing ? (
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
