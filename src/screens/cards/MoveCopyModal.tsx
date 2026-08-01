import { useMemo, useState } from "react";
import type { ChangeEvent } from "react";

import { Modal } from "../../components/Modal";
import {
  getErrorMessage,
  transferCards,
  type Deck,
  type TransferCardsMode,
} from "../../ipc";

export interface TransferableCard {
  noteId: string;
  deckId: string;
}

interface MoveCopyModalProps {
  cards: TransferableCard[];
  decks: Deck[];
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
  const sourceDeckIds = useMemo(
    () => new Set(cards.map((card) => card.deckId)),
    [cards],
  );
  const destinations = decks.filter((deck) => !sourceDeckIds.has(deck.id));
  const [targetDeckId, setTargetDeckId] = useState(destinations[0]?.id ?? "");
  const [mode, setMode] = useState<TransferCardsMode>("move");
  const [skipDuplicates, setSkipDuplicates] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  async function submit(): Promise<void> {
    if (targetDeckId === "" || cards.length === 0 || isSaving) return;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      const result = await transferCards({
        noteIds: cards.map((card) => card.noteId),
        targetDeckId,
        mode,
        skipDuplicates,
      });
      await onComplete();
      const verb = mode === "move" ? "Moved" : "Copied";
      const duplicateCopy =
        result.skippedDuplicates > 0
          ? ` ${result.skippedDuplicates} ${
              result.skippedDuplicates === 1 ? "duplicate was" : "duplicates were"
            } skipped.`
          : "";
      showToast?.(
        "success",
        `${verb} ${result.affectedCount} ${
          result.affectedCount === 1 ? "card" : "cards"
        }.${duplicateCopy}`,
      );
      onClose();
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <Modal
      title="Move or copy cards"
      description={`${cards.length} ${
        cards.length === 1 ? "card is" : "cards are"
      } selected.`}
      width="medium"
      onClose={() => {
        if (!isSaving) onClose();
      }}
    >
      {destinations.length === 0 ? (
        <div className="compact-empty-state">
          <h3>Create another deck first</h3>
          <p>A destination deck must be different from every selected card’s deck.</p>
          <div className="modal-actions">
            <button type="button" className="primary-button" onClick={onClose}>
              Close
            </button>
          </div>
        </div>
      ) : (
        <div className="move-copy-form" aria-busy={isSaving}>
          <fieldset>
            <legend>Action</legend>
            <label>
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
                <small>Keep review history and remove the cards from their current decks.</small>
              </span>
            </label>
            <label>
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
                <small>Create fresh cards with new schedules in the destination deck.</small>
              </span>
            </label>
          </fieldset>

          <label>
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

          <label className="compact-checkbox">
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

          {errorMessage ? (
            <div className="inline-alert error-alert">{errorMessage}</div>
          ) : null}

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
                : `${mode === "move" ? "Move" : "Copy"} ${
                    cards.length
                  } ${cards.length === 1 ? "card" : "cards"}`}
            </button>
          </div>
        </div>
      )}
    </Modal>
  );
}
