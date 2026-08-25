import { useRef, useState, type ChangeEvent } from "react";

import { Modal } from "../../components/Modal";
import { useDurableDraft } from "../../controllers/useDurableDraft";
import {
  analyzeBulkCards,
  bulkCreateCards,
  type BulkAnalyzeResult,
  type BulkCardRowInput,
} from "../../ipc/cards";
import { getErrorMessage } from "../../ipc/common";
import type { DeckSummary } from "../../ipc/decks";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { DialogActions } from "../../ui/overlays/Dialog";
import { Panel } from "../../ui/primitives/Panel";
import { ownedClassNames, sharedClassMaps } from "../../ui/styles/classOwnership";
import ownershipStyles from "./CardBrowserWorkspace.module.css";

const ownedStyleMaps = [...sharedClassMaps, ownershipStyles] as const;


interface BulkEntryModalProps {
  decks: DeckSummary[];
  defaultDeckId?: string;
  fixedDeckId?: string;
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
  fixedDeckId,
  onClose,
  onSaved,
  showToast,
}: BulkEntryModalProps) {
  const activeDecks = decks.filter((deck) => deck.archivedAt === null);
  const fixedDeck =
    fixedDeckId === undefined
      ? null
      : activeDecks.find((deck) => deck.id === fixedDeckId) ?? null;
  const initialDeckId =
    fixedDeck?.id ??
    (defaultDeckId && activeDecks.some((deck) => deck.id === defaultDeckId)
      ? defaultDeckId
      : (activeDecks[0]?.id ?? ""));
  const sequence = useRef(4);
  const [deckId, setDeckId] = useState(initialDeckId);
  const [rows, setRows] = useState<DraftRow[]>([emptyRow(1), emptyRow(2), emptyRow(3)]);
  const [analysis, setAnalysis] = useState<BulkAnalyzeResult | null>(null);
  const [duplicateRowIds, setDuplicateRowIds] = useState<Set<number>>(new Set());
  const [isSaving, setIsSaving] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const populatedRows = rows.filter(
    (row) => row.question.trim() !== "" || row.answer.trim() !== "",
  );
  const invalidRows = populatedRows.filter(
    (row) => row.question.trim() === "" || row.answer.trim() === "",
  );
  const uniqueCount = analysis
    ? Math.max(0, populatedRows.length - analysis.duplicateCount)
    : populatedRows.length;
  const durableDraft = useDurableDraft({
    kind: "bulk_cards",
    contextEntityId: "b".repeat(32),
    payload: { deckId, rows },
    onRecovered: (payload: { deckId: string; rows: DraftRow[] }) => {
      setDeckId(fixedDeck?.id ?? payload.deckId);
      setRows(payload.rows);
      sequence.current = Math.max(4, ...payload.rows.map((row) => row.id + 1));
    },
  });

  const canSubmit =
    deckId !== "" &&
    populatedRows.length > 0 &&
    invalidRows.length === 0 &&
    !isSaving &&
    (!analysis || uniqueCount > 0);

  function clearAnalysis(): void {
    setAnalysis(null);
    setDuplicateRowIds(new Set());
  }

  function updateRow(id: number, patch: Partial<DraftRow>): void {
    setRows((current) => current.map((row) => (row.id === id ? { ...row, ...patch } : row)));
    setErrorMessage(null);
    clearAnalysis();
  }

  function addRows(count = 3): void {
    const available = Math.max(0, MAX_BULK_ROWS - rows.length);
    const additions = Array.from({ length: Math.min(count, available) }, () => {
      const id = sequence.current;
      sequence.current += 1;
      return emptyRow(id);
    });
    setRows((current) => [...current, ...additions]);
    clearAnalysis();
  }

  function payload(): BulkCardRowInput[] {
    return populatedRows.map((row) => ({
      question: row.question,
      answer: row.answer,
      tags: parseTags(row.tags),
    }));
  }

  async function reviewOrSave(): Promise<void> {
    if (!canSubmit) {
      if (invalidRows.length > 0) {
        setErrorMessage("Every started row needs both a question and an answer.");
      }
      return;
    }
    setIsSaving(true);
    setErrorMessage(null);
    const cardRows = payload();

    if (!analysis) {
      let result: BulkAnalyzeResult;
      try {
        result = await analyzeBulkCards({ deckId, rows: cardRows });
      } catch (error: unknown) {
        setErrorMessage(getErrorMessage(error));
        setIsSaving(false);
        return;
      }
      if (result.duplicateCount > 0) {
        const draftIds = result.duplicateRowIndices
          .map((index) => populatedRows[index]?.id)
          .filter((id): id is number => typeof id === "number");
        setDuplicateRowIds(new Set(draftIds));
        setAnalysis({
          ...result,
          uniqueCount: Math.max(0, cardRows.length - result.duplicateCount),
        });
        setIsSaving(false);
        return;
      }
    }

    let createdCount: number;
    let skippedDuplicates: number;
    try {
      const editorDraft = await durableDraft.prepareCanonicalCommit();
      const result = await bulkCreateCards({ deckId, rows: cardRows, editorDraft });
      durableDraft.acceptCanonicalCommit();
      createdCount = result.createdCount;
      skippedDuplicates = result.skippedDuplicates;
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
      setIsSaving(false);
      return;
    }

    const duplicateCopy =
      skippedDuplicates > 0
        ? ` ${skippedDuplicates} exact ${
            skippedDuplicates === 1 ? "duplicate was" : "duplicates were"
          } removed.`
        : "";
    showToast(
      "success",
      `Added ${createdCount} ${createdCount === 1 ? "card" : "cards"}.${duplicateCopy}`,
    );
    try {
      await onSaved();
    } catch (error: unknown) {
      showToast(
        "info",
        `The cards were added, but the current view could not refresh: ${getErrorMessage(error)}`,
      );
    } finally {
      setIsSaving(false);
      onClose();
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
      <div className={ownedClassNames(ownedStyleMaps, "bulk-entry-layout")} aria-busy={isSaving}>
        <div className={ownedClassNames(ownedStyleMaps, "bulk-entry-controls")}>
          {fixedDeck ? (
            <div className={ownedClassNames(ownedStyleMaps, "bulk-entry-destination")}>
              <span>Destination deck</span>
              <strong>{fixedDeck.name}</strong>
            </div>
          ) : (
            <label>
              <span>Destination deck</span>
              <select
                value={deckId}
                disabled={isSaving}
                onChange={(event: ChangeEvent<HTMLSelectElement>) => {
                  setDeckId(event.currentTarget.value);
                  setErrorMessage(null);
                  clearAnalysis();
                }}
              >
                {activeDecks.map((deck) => (
                  <option key={deck.id} value={deck.id}>
                    {deck.name}
                  </option>
                ))}
              </select>
            </label>
          )}
          <p>Exact duplicate question-and-answer pairs are detected before anything is saved.</p>
        </div>

        <div className={ownedClassNames(ownedStyleMaps, "bulk-entry-table")} role="table" aria-label="Bulk cards">
          <div className={ownedClassNames(ownedStyleMaps, "bulk-entry-header")} role="row">
            <span role="columnheader">Question</span>
            <span role="columnheader">Answer</span>
            <span role="columnheader">Tags</span>
            <span aria-hidden="true" />
          </div>
          {rows.map((row, index) => (
            <div
              className={ownedClassNames(ownedStyleMaps, duplicateRowIds.has(row.id)
                  ? "bulk-entry-row duplicate-bulk-row"
                  : "bulk-entry-row")}
              role="row"
              key={row.id}
            >
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
              <Button
                type="button"
                variant="ghost" size="icon"
                aria-label={`Remove row ${index + 1}`}
                disabled={isSaving || rows.length === 1}
                onClick={() => {
                  setRows((current) => current.filter((candidate) => candidate.id !== row.id));
                  clearAnalysis();
                }}
              >
                ×
              </Button>
              {duplicateRowIds.has(row.id) ? (
                <small className={ownedClassNames(ownedStyleMaps, "duplicate-row-label")}>Exact duplicate</small>
              ) : null}
            </div>
          ))}
        </div>

        <Button
          type="button"
          variant="text" className={ownedClassNames(ownedStyleMaps, "bulk-add-rows")}
          disabled={isSaving || rows.length >= MAX_BULK_ROWS}
          onClick={() => addRows()}
        >
          {rows.length >= MAX_BULK_ROWS ? "500-row limit reached" : "+ Add three rows"}
        </Button>

        {analysis ? (
          <Panel  aria-live="polite">
            <div>
              <h3>
                Remove {analysis.duplicateCount} exact duplicate
                {analysis.duplicateCount === 1 ? "" : "s"}?
              </h3>
              <p>
                Duplicate rows are highlighted above. They will be omitted; the remaining{" "}
                {uniqueCount} unique {uniqueCount === 1 ? "card" : "cards"} will be saved.
              </p>
            </div>
            <Button
              type="button"
              variant="secondary"
              disabled={isSaving}
              onClick={clearAnalysis}
            >
              Keep editing
            </Button>
          </Panel>
        ) : null}

        {analysis && uniqueCount === 0 ? (
          <InlineNotice kind="info">
            Every populated row is already present. Change or remove the highlighted rows before
            saving.
          </InlineNotice>
        ) : null}
        {errorMessage ? <InlineNotice kind="error">{errorMessage}</InlineNotice> : null}

        <DialogActions className={ownedClassNames(ownedStyleMaps, "bulk-entry-actions")}>
          <Button
            type="button"
            variant="dangerGhost"
            disabled={isSaving}
            onClick={() => void durableDraft.discard().then(onClose)}
          >
            Discard draft
          </Button>
          <Button
            type="button"
            variant="secondary"
            disabled={isSaving}
            onClick={() => void durableDraft.flush().finally(onClose)}
          >
            Close
          </Button>
          <Button
            type="button"
            variant="primary"
            disabled={!canSubmit}
            onClick={() => void reviewOrSave()}
          >
            {isSaving
              ? "Checking…"
              : analysis
                ? `Remove duplicates and add ${uniqueCount}`
                : `Add ${populatedRows.length} ${
                    populatedRows.length === 1 ? "card" : "cards"
                  }`}
          </Button>
        </DialogActions>
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
