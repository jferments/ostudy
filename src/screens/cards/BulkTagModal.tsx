import { useState } from "react";

import { Modal } from "../../components/Modal";
import { TagInput } from "../../components/TagInput";
import {
  bulkTagCards,
  getErrorMessage,
  type SelectionCard,
  type TagChangeMode,
  type TagSummary,
} from "../../ipc";

interface BulkTagModalProps {
  cards: SelectionCard[];
  mode: TagChangeMode;
  suggestions: TagSummary[];
  onClose: () => void;
  onComplete: () => void | Promise<void>;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

export function BulkTagModal({
  cards,
  mode,
  suggestions,
  onClose,
  onComplete,
  showToast,
}: BulkTagModalProps) {
  const [tags, setTags] = useState<string[]>([]);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const adding = mode === "add";

  async function submit(): Promise<void> {
    if (tags.length === 0 || cards.length === 0 || isSaving) return;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      const result = await bulkTagCards({
        noteIds: cards.map((card) => card.noteId),
        tags,
        mode,
      });
      await onComplete();
      showToast(
        "success",
        `${adding ? "Added tags to" : "Removed tags from"} ${result.affectedCount} ${
          result.affectedCount === 1 ? "card" : "cards"
        }.`,
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
      title={adding ? "Add tags to selected cards" : "Remove tags from selected cards"}
      description={`${cards.length} ${cards.length === 1 ? "card is" : "cards are"} selected.`}
      width="medium"
      onClose={() => {
        if (!isSaving) onClose();
      }}
    >
      <div className="bulk-tag-form" aria-busy={isSaving}>
        <TagInput
          tags={tags}
          suggestions={suggestions}
          disabled={isSaving}
          onChange={(nextTags) => {
            setTags(nextTags);
            setErrorMessage(null);
          }}
        />
        <p className="modal-help-text">
          {adding
            ? "Existing tags are preserved. Only the tags listed here will be added."
            : "Only the tags listed here will be removed; all other tags are preserved."}
        </p>
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
            disabled={isSaving || tags.length === 0}
            onClick={() => void submit()}
          >
            {isSaving ? "Saving…" : adding ? "Add tags" : "Remove tags"}
          </button>
        </div>
      </div>
    </Modal>
  );
}
