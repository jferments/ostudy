import { useEffect, useRef, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";

import { getErrorMessage, type Deck } from "../ipc";
import {
  commitFlashcardImport,
  inspectFlashcardImport,
  type FlashcardImportPreview,
  type FlashcardImportResult,
} from "../generatedImport";
import { Modal } from "./Modal";

interface GeneratedImportDialogProps {
  decks: Deck[];
  onClose: () => void;
  onImported: (result: FlashcardImportResult) => Promise<void>;
}

export function GeneratedImportDialog({
  decks,
  onClose,
  onImported,
}: GeneratedImportDialogProps) {
  const [sourcePath, setSourcePath] = useState<string | null>(null);
  const [destinationDeckId, setDestinationDeckId] = useState("");
  const [preview, setPreview] = useState<FlashcardImportPreview | null>(null);
  const [allowDuplicates, setAllowDuplicates] = useState(false);
  const [isBusy, setIsBusy] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const requestRef = useRef(0);

  useEffect(() => {
    if (!sourcePath) return;
    const request = ++requestRef.current;
    setIsBusy(true);
    setErrorMessage(null);
    void inspectFlashcardImport(sourcePath, destinationDeckId || undefined)
      .then((result) => {
        if (request === requestRef.current) setPreview(result);
      })
      .catch((error: unknown) => {
        if (request === requestRef.current) {
          setPreview(null);
          setErrorMessage(getErrorMessage(error));
        }
      })
      .finally(() => {
        if (request === requestRef.current) setIsBusy(false);
      });
  }, [sourcePath, destinationDeckId]);

  async function chooseFile(): Promise<void> {
    const selected = await openDialog({
      title: "Import generated flashcards",
      multiple: false,
      directory: false,
      filters: [
        { name: "ostudy generated flashcards", extensions: ["json"] },
      ],
    });
    if (typeof selected !== "string") return;
    setSourcePath(selected);
    setPreview(null);
    setErrorMessage(null);
  }

  async function importCards(): Promise<void> {
    if (!sourcePath || !preview || isBusy) return;
    setIsBusy(true);
    setErrorMessage(null);
    try {
      const result = await commitFlashcardImport({
        sourcePath,
        expectedFingerprint: preview.fingerprint,
        destinationDeckId: destinationDeckId || undefined,
        allowDuplicates,
      });
      await onImported(result);
    } catch (error: unknown) {
      setErrorMessage(getErrorMessage(error));
      setIsBusy(false);
    }
  }

  return (
    <Modal
      title="Import generated flashcards"
      description="Preview a versioned .ostudy.json file before anything is written."
      width="large"
      onClose={() => {
        if (!isBusy) onClose();
      }}
    >
      <div className="form-stack generated-import-dialog">
        <div className="generated-import-file-row">
          <button
            type="button"
            className="secondary-button"
            disabled={isBusy}
            onClick={() => void chooseFile()}
          >
            Choose JSON file
          </button>
          <span className="muted-text">
            {sourcePath ? sourcePath.split(/[\\/]/u).at(-1) : "No file selected"}
          </span>
        </div>

        <label>
          <span>Destination</span>
          <select
            value={destinationDeckId}
            disabled={isBusy || !sourcePath}
            onChange={(event) => setDestinationDeckId(event.currentTarget.value)}
          >
            <option value="">Create the deck named in the file</option>
            {decks.map((deck) => (
              <option key={deck.id} value={deck.id}>
                Add to {deck.name}
              </option>
            ))}
          </select>
        </label>

        {isBusy && !preview ? <div className="inline-progress">Validating import…</div> : null}
        {errorMessage ? <div className="inline-alert error-alert">{errorMessage}</div> : null}

        {preview ? (
          <>
            <section className="generated-import-summary" aria-label="Import summary">
              <div>
                <small>Proposed deck</small>
                <strong>{preview.deckName}</strong>
                {preview.deckDescription ? <span>{preview.deckDescription}</span> : null}
              </div>
              <dl>
                <div><dt>Source notes</dt><dd>{preview.sourceNoteCount}</dd></div>
                <div><dt>Generated cards</dt><dd>{preview.generatedCardCount}</dd></div>
                <div><dt>Basic</dt><dd>{preview.basicCount}</dd></div>
                <div><dt>Reverse enabled</dt><dd>{preview.reverseEnabledCount}</dd></div>
                <div><dt>Fill in the blank</dt><dd>{preview.clozeCount}</dd></div>
                <div><dt>Exact duplicates</dt><dd>{preview.duplicateCount}</dd></div>
              </dl>
            </section>

            {preview.uniqueTags.length > 0 ? (
              <div className="tag-list" aria-label="Imported tags">
                {preview.uniqueTags.map((tag) => <span key={tag}>{tag}</span>)}
              </div>
            ) : null}

            <section className="generated-import-samples" aria-label="Card preview">
              <h3>Preview</h3>
              {preview.samples.map((sample, index) => (
                <article key={`${sample.cardType}-${index}`}>
                  <small>{sample.cardType === "cloze" ? "Fill in the blank" : sample.cardType === "basic_reverse" ? "Basic + reverse" : "Basic"}</small>
                  <strong>{sample.question}</strong>
                  <span>{sample.answer}</span>
                </article>
              ))}
            </section>

            <label className="checkbox-row">
              <input
                type="checkbox"
                checked={allowDuplicates}
                disabled={isBusy}
                onChange={(event) => setAllowDuplicates(event.currentTarget.checked)}
              />
              <span>
                Import exact duplicates
                <small>Off by default. Duplicate cards are otherwise skipped.</small>
              </span>
            </label>
          </>
        ) : null}

        <div className="modal-actions">
          <button type="button" className="secondary-button" disabled={isBusy} onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="primary-button"
            disabled={!preview || isBusy}
            onClick={() => void importCards()}
          >
            {isBusy && preview ? "Importing…" : "Import flashcards"}
          </button>
        </div>
      </div>
    </Modal>
  );
}
