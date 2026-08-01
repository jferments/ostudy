import { useMemo, useRef, useState } from "react";

import { Modal } from "../../components/Modal";
import {
  getErrorMessage,
  transferCards,
  type CardTransferMode,
  type Deck,
  type TransferCardsResult,
} from "../../ipc";

interface CardTransferModalProps {
  decks: Deck[];
  currentDeck: Deck;
  noteIds: string[];
  initialMode: CardTransferMode;
  onClose: () => void;
  onComplete: (
    mode: CardTransferMode,
    destinationDeckId: string,
    result: TransferCardsResult,
  ) => void | Promise<void>;
}

export function CardTransferModal({
  decks,
  currentDeck,
  noteIds,
  initialMode,
  onClose,
  onComplete,
}: CardTransferModalProps) {
  const destinations = useMemo(
    () => decks.filter((deck) => deck.id !== currentDeck.id),
    [currentDeck.id, decks],
  );
  const [mode, setMode] = useState<CardTransferMode>(initialMode);
  const [destinationDeckId, setDestinationDeckId] = useState(destinations[0]?.id ?? "");
  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  async function submit(): Promise<void> {
    if (savingRef.current || destinationDeckId === "") return;
    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      const result = await transferCards({ noteIds, destinationDeckId, mode });
      await onComplete(mode, destinationDeckId, result);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  }

  const action = mode === "move" ? "Move" : "Copy";
  return (
    <Modal
      title={`${action} ${noteIds.length} ${noteIds.length === 1 ? "card" : "cards"}`}
      description={
        mode === "move"
          ? "Moving keeps each card's tags, images, schedule, and review history."
          : "Copying keeps content, tags, and images, but the new copies begin with fresh schedules and no review history."
      }
      onClose={() => {
        if (!isSaving) onClose();
      }}
      width="small"
    >
      <div className="form-stack">
        <fieldset className="transfer-mode-fieldset" disabled={isSaving}>
          <legend>Action</legend>
          <label>
            <input
              type="radio"
              name="transfer-mode"
              value="move"
              checked={mode === "move"}
              onChange={() => setMode("move")}
            />
            <span>Move cards</span>
          </label>
          <label>
            <input
              type="radio"
              name="transfer-mode"
              value="copy"
              checked={mode === "copy"}
              onChange={() => setMode("copy")}
            />
            <span>Copy cards</span>
          </label>
        </fieldset>

        {destinations.length === 0 ? (
          <div className="inline-alert info-alert">
            Create another deck before moving or copying cards.
          </div>
        ) : (
          <label>
            <span>Destination deck</span>
            <select
              autoFocus
              value={destinationDeckId}
              disabled={isSaving}
              onChange={(event) => setDestinationDeckId(event.currentTarget.value)}
            >
              {destinations.map((deck) => (
                <option key={deck.id} value={deck.id}>
                  {deck.name} ({deck.cardCount} {deck.cardCount === 1 ? "card" : "cards"})
                </option>
              ))}
            </select>
          </label>
        )}

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
            disabled={isSaving || destinationDeckId === ""}
            onClick={() => void submit()}
          >
            {isSaving ? `${action}ing…` : `${action} selected cards`}
          </button>
        </div>
      </div>
    </Modal>
  );
}
