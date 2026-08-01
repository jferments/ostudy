import { useState } from "react";

import { Modal } from "../../components/Modal";
import {
  deleteCards,
  getErrorMessage,
  type SelectionCard,
} from "../../ipc";

interface DeleteCardsModalProps {
  cards: SelectionCard[];
  onClose: () => void;
  onComplete: () => void | Promise<void>;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

export function DeleteCardsModal({
  cards,
  onClose,
  onComplete,
  showToast,
}: DeleteCardsModalProps) {
  const [isDeleting, setIsDeleting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  async function submit(): Promise<void> {
    if (cards.length === 0 || isDeleting) return;
    setIsDeleting(true);
    setErrorMessage(null);
    try {
      const result = await deleteCards(cards.map((card) => card.noteId));
      await onComplete();
      showToast(
        "success",
        `Deleted ${result.affectedCount} ${result.affectedCount === 1 ? "card" : "cards"}.`,
      );
      onClose();
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      setIsDeleting(false);
    }
  }

  return (
    <Modal
      title="Delete selected cards"
      description={`${cards.length} ${cards.length === 1 ? "card is" : "cards are"} selected.`}
      width="small"
      onClose={() => {
        if (!isDeleting) onClose();
      }}
    >
      <div className="delete-cards-form" aria-busy={isDeleting}>
        <div className="inline-alert delete-warning">
          This removes the selected cards from their decks, including their local
          scheduling state. This action cannot be undone from inside ostudy.
        </div>
        {errorMessage ? (
          <div className="inline-alert error-alert">{errorMessage}</div>
        ) : null}
        <div className="modal-actions">
          <button
            type="button"
            className="secondary-button"
            disabled={isDeleting}
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            type="button"
            className="danger-button"
            disabled={isDeleting}
            onClick={() => void submit()}
          >
            {isDeleting
              ? "Deleting…"
              : `Delete ${cards.length} ${cards.length === 1 ? "card" : "cards"}`}
          </button>
        </div>
      </div>
    </Modal>
  );
}
