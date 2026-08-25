import { useState } from "react";

import { Modal } from "../../components/Modal";
import { TagInput } from "../../components/TagInput";
import {
  bulkTagCards,
  type SelectionCard,
  type TagChangeMode,
  type TagSummary,
} from "../../ipc/cards";
import {
  getErrorMessage,
} from "../../ipc/common";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { DialogActions } from "../../ui/overlays/Dialog";

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

    let affectedCount: number;
    try {
      const result = await bulkTagCards({
        noteIds: cards.map((card) => card.noteId),
        tags,
        mode,
      });
      affectedCount = result.affectedCount;
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
      setIsSaving(false);
      return;
    }

    showToast(
      "success",
      `${adding ? "Added tags to" : "Removed tags from"} ${affectedCount} ${
        affectedCount === 1 ? "card" : "cards"
      }.`,
    );
    try {
      await onComplete();
    } catch (error: unknown) {
      showToast(
        "info",
        `The tag changes were saved, but the current view could not refresh: ${getErrorMessage(error)}`,
      );
    } finally {
      setIsSaving(false);
      onClose();
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
          <InlineNotice kind="error">{errorMessage}</InlineNotice>
        ) : null}
        <DialogActions>
          <Button
            type="button"
            variant="secondary"
            disabled={isSaving}
            onClick={onClose}
          >
            Cancel
          </Button>
          <Button
            type="button"
            variant="primary"
            disabled={isSaving || tags.length === 0}
            onClick={() => void submit()}
          >
            {isSaving ? "Saving…" : adding ? "Add tags" : "Remove tags"}
          </Button>
        </DialogActions>
      </div>
    </Modal>
  );
}
