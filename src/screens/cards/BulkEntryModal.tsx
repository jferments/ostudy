import { useEffect, useRef, useState } from "react";
import type { ChangeEvent } from "react";

import { Modal } from "../../components/Modal";
import {
  bulkCreateCards,
  getErrorMessage,
  type Deck,
} from "../../ipc";

interface BulkEntryModalProps {
  decks: Deck[];
  defaultDeckId?: string;
  onClose: () => void;
  onSaved: () => void | Promise<void>;
  showToast: (kind: "success" | "error" | "info", text: string) => void;
}

const MAX_BULK_ROWS = 500;

interface DraftRow {
  id: number;
  question: string;
  answer: string;
  tags: string;
}

export function BulkEntryModal({
  decks,
  defaultDeckId,
  onClose,
  onSaved,
  showToast,
}: BulkEntryModalProps) {
  const sequence = useRef(4);
  const [deckId, setDeckId] = useState(
    defaultDeckId && decks.some((deck) => deck.id === defaultDeckId)
      ? defaultDeckId
      : (decks[0]?.id ?? ""),
  );
  const [rows, setRows] = useState<DraftRow[]>([
    emptyRow(1),
    emptyRow(2),
    emptyRow(3),
  ]);
  const [skipDuplicates, setSkipDuplicates] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const populatedRows = rows.filter(
    (row) => row.question.trim() !== "" || row.answer.trim() !== "",
  );
  const invalidRows = populatedRows.filter(
    (row) => row.question.trim() === "" || row.answer.trim() === "",
  );
  const canSave =
    deckId !== "" &&
    populatedRows.length > 0 &&
    invalidRows.length === 0 &&
    !isSaving;

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent): void {
      if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
        event.preventDefault();
        if (canSave) void save();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  function updateRow(id: number, patch: Partial<DraftRow>): void {
    setRows((current) =>
      current.map((row) => (row.id === id ? { ...row, ...patch } : row)),
    );
    setErrorMessage(null);
  }

  function addRows(count = 3): void {
    const available = Math.max(0, MAX_BULK_ROWS - rows.length);
    const additions = Array.from({ length: Math.min(count, available) }, () => {
      const id = sequence.current;
      sequence.current += 1;
      return emptyRow(id);
    });
    setRows((current) => [...current, ...additions]);
  }

  async function save(): Promise<void> {
    if (!canSave) {
      if (invalidRows.length > 0) {
        setErrorMessage("Every started row needs both a question and an answer.");
      }
      return;
    }
    setIsSaving(true);
    setErrorMessage(null);
    try {
      const result = await bulkCreateCards({
        deckId,
        skipDuplicates,
        rows: populatedRows.map((row) => ({
          question: row.question,
          answer: row.answer,
          tags: parseTags(row.tags),
        })),
      });
      await onSaved();
      const duplicateCopy =
        result.skippedDuplicates > 0
          ? ` ${result.skippedDuplicates} exact ${
              result.skippedDuplicates === 1 ? "duplicate was" : "duplicates were"
            } skipped.`
          : "";
      showToast(
        "success",
        `Added ${result.createdCount} ${
          result.createdCount === 1 ? "card" : "cards"
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
      title="Bulk add cards"
      description="Enter several plain-text cards at once. Empty rows are ignored."
      width="large"
      onClose={() => {
        if (!isSaving) onClose();
      }}
    >
      <div className="bulk-entry-layout" aria-busy={isSaving}>
        <div className="bulk-entry-controls">
          <label>
            <span>Destination deck</span>
            <select
              value={deckId}
              disabled={isSaving}
              onChange={(event: ChangeEvent<HTMLSelectElement>) => setDeckId(event.currentTarget.value)}
            >
              {decks.map((deck) => (
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
              onChange={(event: ChangeEvent<HTMLInputElement>) => setSkipDuplicates(event.currentTarget.checked)}
            />
            <span>Skip exact duplicates</span>
          </label>
        </div>

        <div className="bulk-entry-table" role="table" aria-label="Bulk cards">
          <div className="bulk-entry-header" role="row">
            <span role="columnheader">Question</span>
            <span role="columnheader">Answer</span>
            <span role="columnheader">Tags</span>
            <span aria-hidden="true" />
          </div>
          {rows.map((row, index) => (
            <div className="bulk-entry-row" role="row" key={row.id}>
              <label>
                <span className="sr-only">Question {index + 1}</span>
                <textarea
                  rows={2}
                  value={row.question}
                  disabled={isSaving}
                  placeholder="Question"
                  onChange={(event: ChangeEvent<HTMLTextAreaElement>) =>
                    updateRow(row.id, { question: event.currentTarget.value })
                  }
                />
              </label>
              <label>
                <span className="sr-only">Answer {index + 1}</span>
                <textarea
                  rows={2}
                  value={row.answer}
                  disabled={isSaving}
                  placeholder="Answer"
                  onChange={(event: ChangeEvent<HTMLTextAreaElement>) =>
                    updateRow(row.id, { answer: event.currentTarget.value })
                  }
                />
              </label>
              <label>
                <span className="sr-only">Tags {index + 1}</span>
                <input
                  value={row.tags}
                  disabled={isSaving}
                  placeholder="tag one, tag two"
                  onChange={(event: ChangeEvent<HTMLInputElement>) =>
                    updateRow(row.id, { tags: event.currentTarget.value })
                  }
                />
              </label>
              <button
                type="button"
                className="icon-button"
                aria-label={`Remove row ${index + 1}`}
                disabled={isSaving || rows.length === 1}
                onClick={() =>
                  setRows((current) =>
                    current.filter((candidate) => candidate.id !== row.id),
                  )
                }
              >
                ×
              </button>
            </div>
          ))}
        </div>

        <button
          type="button"
          className="text-button bulk-add-rows"
          disabled={isSaving || rows.length >= MAX_BULK_ROWS}
          onClick={() => addRows()}
        >
          {rows.length >= MAX_BULK_ROWS ? "500-row limit reached" : "+ Add three rows"}
        </button>

        {errorMessage ? (
          <div className="inline-alert error-alert">{errorMessage}</div>
        ) : null}

        <div className="modal-actions">
          <span className="keyboard-hint">Ctrl/⌘ + Enter saves</span>
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
            disabled={!canSave}
            onClick={() => void save()}
          >
            {isSaving
              ? "Adding…"
              : `Add ${populatedRows.length} ${
                  populatedRows.length === 1 ? "card" : "cards"
                }`}
          </button>
        </div>
      </div>
    </Modal>
  );
}

function emptyRow(id: number): DraftRow {
  return { id, question: "", answer: "", tags: "" };
}

function parseTags(value: string): string[] {
  const seen = new Set<string>();
  const tags: string[] = [];
  for (const raw of value.split(",")) {
    const tag = raw.trim().replace(/\s+/gu, " ");
    const normalized = tag.toLocaleLowerCase();
    if (tag !== "" && !seen.has(normalized)) {
      seen.add(normalized);
      tags.push(tag);
    }
  }
  return tags;
}
