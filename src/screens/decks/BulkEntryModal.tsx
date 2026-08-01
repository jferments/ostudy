import { useMemo, useRef, useState } from "react";

import { Modal } from "../../components/Modal";
import {
  bulkCreateCards,
  getErrorMessage,
  type BulkCardRowInput,
  type BulkCreateResult,
  type Deck,
  type DuplicatePolicy,
} from "../../ipc";

const MAX_ROWS = 500;

export interface ParsedBulkCards {
  rows: BulkCardRowInput[];
  errors: string[];
}

export function parseBulkCards(value: string): ParsedBulkCards {
  const rows: BulkCardRowInput[] = [];
  const errors: string[] = [];
  const lines = value.replaceAll("\r\n", "\n").replaceAll("\r", "\n").split("\n");

  lines.forEach((rawLine, index) => {
    const line = rawLine.trim();
    if (line === "") return;

    let columns: string[];
    if (line.includes("\t")) {
      columns = line.split("\t");
    } else if (line.includes("::")) {
      columns = line.split("::");
    } else {
      errors.push(
        `Line ${index + 1}: separate the question and answer with a tab or ::.`,
      );
      return;
    }

    const frontText = columns[0]?.trim() ?? "";
    const backText = columns[1]?.trim() ?? "";
    const tagText = columns.slice(2).join("::").trim();
    if (frontText === "" || backText === "") {
      errors.push(`Line ${index + 1}: both the question and answer are required.`);
      return;
    }
    const tags = tagText === ""
      ? []
      : tagText
          .split(",")
          .map((tag) => tag.trim())
          .filter((tag) => tag !== "");
    rows.push({ frontText, backText, tags });
  });

  if (rows.length > MAX_ROWS) {
    errors.push(`Bulk entry accepts at most ${MAX_ROWS} cards at a time.`);
  }
  return { rows: rows.slice(0, MAX_ROWS), errors };
}

interface BulkEntryModalProps {
  deck: Deck;
  onClose: () => void;
  onComplete: (result: BulkCreateResult) => void | Promise<void>;
}

export function BulkEntryModal({ deck, onClose, onComplete }: BulkEntryModalProps) {
  const [value, setValue] = useState("");
  const [duplicatePolicy, setDuplicatePolicy] = useState<DuplicatePolicy>("skip");
  const [isSaving, setIsSaving] = useState(false);
  const savingRef = useRef(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const parsed = useMemo(() => parseBulkCards(value), [value]);
  const canSubmit = parsed.rows.length > 0 && parsed.errors.length === 0 && !isSaving;

  async function submit(): Promise<void> {
    if (!canSubmit || savingRef.current) return;
    savingRef.current = true;
    setIsSaving(true);
    setErrorMessage(null);
    try {
      const result = await bulkCreateCards({
        deckId: deck.id,
        rows: parsed.rows,
        duplicatePolicy,
      });
      await onComplete(result);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  }

  return (
    <Modal
      title={`Bulk add to ${deck.name}`}
      description="Enter one card per line. Use a tab between question and answer, with optional comma-separated tags in a third column. You can also use :: as the separator."
      onClose={() => {
        if (!isSaving) onClose();
      }}
      width="large"
    >
      <div className="bulk-entry-layout">
        <label className="bulk-entry-field">
          <span>Cards</span>
          <textarea
            autoFocus
            value={value}
            disabled={isSaving}
            rows={14}
            spellCheck={false}
            onChange={(event) => {
              setValue(event.currentTarget.value);
              setErrorMessage(null);
            }}
            placeholder={
              "What is preload?\tVentricular stretch at end diastole\tcardiology, hemodynamics\nWhat is afterload? :: Resistance the ventricle ejects against :: cardiology"
            }
          />
        </label>

        <div className="bulk-entry-options">
          <label>
            <span>When an identical card already exists</span>
            <select
              value={duplicatePolicy}
              disabled={isSaving}
              onChange={(event) =>
                setDuplicatePolicy(event.currentTarget.value as DuplicatePolicy)
              }
            >
              <option value="skip">Skip duplicates</option>
              <option value="allow">Create them anyway</option>
              <option value="error">Stop without creating anything</option>
            </select>
          </label>
          <div className="bulk-preview-summary" aria-live="polite">
            <strong>{parsed.rows.length}</strong>
            <span>{parsed.rows.length === 1 ? "card ready" : "cards ready"}</span>
          </div>
        </div>

        {parsed.errors.length > 0 ? (
          <div className="inline-alert error-alert bulk-errors">
            {parsed.errors.slice(0, 8).map((error) => (
              <div key={error}>{error}</div>
            ))}
            {parsed.errors.length > 8 ? (
              <div>Fix the remaining {parsed.errors.length - 8} lines as well.</div>
            ) : null}
          </div>
        ) : null}
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
            disabled={!canSubmit}
            onClick={() => void submit()}
          >
            {isSaving ? "Adding cards…" : `Add ${parsed.rows.length || ""} cards`}
          </button>
        </div>
      </div>
    </Modal>
  );
}
