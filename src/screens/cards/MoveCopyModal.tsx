import { useMemo, useState } from "react";
import type { ChangeEvent } from "react";

import { Modal } from "../../components/Modal";
import {
  getErrorMessage,
  transferCards,
  type DeckSummary,
  type TransferCardsMode,
} from "../../ipc";

export interface TransferableCard {
  noteId: string;
  deckId: string;
}

interface MoveCopyModalProps {
  cards: TransferableCard[];
  decks: DeckSummary[];
  onClose: () => void;
  onComplete: () => void | Promise<void>;
  showToast?: (kind: "success" | "error" | "info", text: string) => void;
}

export function MoveCopyModal({
  cards,
  decks,
  onClose,
  onComplete,
  showToast,
}: MoveCopyModalProps) {
  const sourceDeckIds = useMemo(() => new Set(cards.map((card) => card.deckId)), [cards]);
  const destinations = decks.filter(
    (deck) => deck.archivedAt === null && !sourceDeckIds.has(deck.id),
  );
  const [targetDeckId, setTargetDeckId] = useState(destinations[0]?.id ?? "");
  const [mode, setMode] = useState<TransferCardsMode>("move");
  const [skipDuplicates, setSkipDuplicates] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  async function submit(): Promise<void> {
    if (targetDeckId === "" || cards.length === 0 || isSaving) return;
    setIsSaving(true);
    setErrorMessage(null);

    let affectedCount: number;
    let skippedDuplicates: number;
    try {
      const result = await transferCards({
        noteIds: cards.map((card) => card.noteId),
        targetDeckId,
        mode,
        skipDuplicates,
      });
      affectedCount = result.affectedCount;
      skippedDuplicates = result.skippedDuplicates;
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
      setIsSaving(false);
      return;
    }

    const verb = mode === "move" ? "Moved" : "Copied";
    const duplicateCopy =
      skippedDuplicates > 0
        ? ` ${skippedDuplicates} ${
            skippedDuplicates === 1 ? "duplicate was" : "duplicates were"
          } skipped.`
        : "";
    showToast?.(
      "success",
      `${verb} ${affectedCount} ${affectedCount === 1 ? "card" : "cards"}.${duplicateCopy}`,
    );

    try {
      await onComplete();
    } catch (error: unknown) {
      const message = `The cards were ${
        mode === "move" ? "moved" : "copied"
      }, but the current view could not refresh: ${getErrorMessage(error)}`;
      if (showToast) showToast("info", message);
      else setErrorMessage(message);
    } finally {
      setIsSaving(false);
      onClose();
    }
  }

  return (
    <Modal
      title="Move or copy cards"
      description={`${cards.length} ${cards.length === 1 ? "card is" : "cards are"} selected.`}
      width="medium"
      onClose={() => {
        if (!isSaving) onClose();
      }}
    >
      {destinations.length === 0 ? (
        <div className="compact-empty-state modal-empty-state">
          <h3>Create or unarchive another deck first</h3>
          <p>A destination deck must be active and different from every selected card’s deck.</p>
          <div className="modal-actions">
            <button type="button" className="primary-button" onClick={onClose}>
              Close
            </button>
          </div>
        </div>
      ) : (
        <div className="move-copy-form" aria-busy={isSaving}>
          <fieldset className="transfer-mode-group">
            <legend>Action</legend>
            <label className="transfer-mode-option">
              <input
                type="radio"
                name="transfer-mode"
                value="move"
                checked={mode === "move"}
                disabled={isSaving}
                onChange={() => setMode("move")}
              />
              <span>
                <strong>Move</strong>
                <small>Keep all review history and reattribute it to the destination deck.</small>
              </span>
            </label>
            <label className="transfer-mode-option">
              <input
                type="radio"
                name="transfer-mode"
                value="copy"
                checked={mode === "copy"}
                disabled={isSaving}
                onChange={() => setMode("copy")}
              />
              <span>
                <strong>Copy</strong>
                <small>Create new cards with fresh schedules in the destination.</small>
              </span>
            </label>
          </fieldset>

          <label className="move-copy-destination">
            <span>Destination deck</span>
            <select
              value={targetDeckId}
              disabled={isSaving}
              onChange={(event: ChangeEvent<HTMLSelectElement>) =>
                setTargetDeckId(event.currentTarget.value)
              }
            >
              {destinations.map((deck) => (
                <option key={deck.id} value={deck.id}>
                  {deck.name}
                </option>
              ))}
            </select>
          </label>

          <label className="compact-checkbox move-copy-duplicate-option">
            <input
              type="checkbox"
              checked={skipDuplicates}
              disabled={isSaving}
              onChange={(event: ChangeEvent<HTMLInputElement>) =>
                setSkipDuplicates(event.currentTarget.checked)
              }
            />
            <span>Skip exact duplicates already in the destination</span>
          </label>

          {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}

          <div className="modal-actions">
            <button
              type="button"
              className="secondary-button"
              disabled={isSaving}
              onClick={onClose}
            >
              Cancel
            </button>
            <button
              type="button"
              className="primary-button"
              disabled={isSaving || targetDeckId === ""}
              onClick={() => void submit()}
            >
              {isSaving
                ? mode === "move"
                  ? "Moving…"
                  : "Copying…"
                : `${mode === "move" ? "Move" : "Copy"} ${cards.length} ${
                    cards.length === 1 ? "card" : "cards"
                  }`}
            </button>
          </div>
        </div>
      )}
    </Modal>
  );
}
