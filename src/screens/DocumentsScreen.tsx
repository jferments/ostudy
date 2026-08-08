import { useCallback, useEffect, useMemo, useState } from "react";

import type { ShowToast } from "../components/Toast";
import { getErrorMessage } from "../ipc/common";
import {
  getDocumentSourcePath,
  importPdfAndGenerateFlashcards,
  listDocuments,
  retryPdfFlashcardGeneration,
  setDocumentArchived,
  type DocumentSummary,
} from "../ipc/documents";
import { openFile, openLocalPath } from "../platform/files";
import styles from "./DocumentsScreen.module.css";

interface DocumentsScreenProps {
  refreshDecks: () => Promise<unknown>;
  showToast: ShowToast;
}

export function DocumentsScreen({ refreshDecks, showToast }: DocumentsScreenProps) {
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [includeArchived, setIncludeArchived] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isUploading, setIsUploading] = useState(false);
  const [busyDocumentId, setBusyDocumentId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const hasProcessingDocument = useMemo(
    () => documents.some((document) => document.lifecycleState === "processing"),
    [documents],
  );

  const refreshDocuments = useCallback(async () => {
    const loaded = await listDocuments(includeArchived);
    setDocuments(loaded);
    setErrorMessage(null);
    return loaded;
  }, [includeArchived]);

  useEffect(() => {
    let active = true;
    setIsLoading(true);
    void refreshDocuments()
      .catch((error: unknown) => {
        if (active) setErrorMessage(getErrorMessage(error));
      })
      .finally(() => {
        if (active) setIsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [refreshDocuments]);

  useEffect(() => {
    if (!isUploading && !busyDocumentId && !hasProcessingDocument) return;
    const timer = window.setInterval(() => {
      void refreshDocuments().catch(() => undefined);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [busyDocumentId, hasProcessingDocument, isUploading, refreshDocuments]);

  async function uploadPdf(): Promise<void> {
    if (isUploading || busyDocumentId) return;
    const selected = await openFile({
      title: "Upload PDF",
      filters: [{ name: "PDF documents", extensions: ["pdf"] }],
    });
    if (typeof selected !== "string") return;

    setIsUploading(true);
    setErrorMessage(null);
    try {
      const result = await importPdfAndGenerateFlashcards(selected);
      await Promise.all([refreshDocuments(), refreshDecks()]);
      showToast(
        "success",
        `Created ${result.createdCardCount} flashcard${result.createdCardCount === 1 ? "" : "s"} in a new deck.`,
      );
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
      await refreshDocuments().catch(() => undefined);
    } finally {
      setIsUploading(false);
    }
  }

  async function retry(document: DocumentSummary): Promise<void> {
    if (isUploading || busyDocumentId) return;
    setBusyDocumentId(document.id);
    setErrorMessage(null);
    try {
      const result = await retryPdfFlashcardGeneration(document.id);
      await Promise.all([refreshDocuments(), refreshDecks()]);
      showToast(
        "success",
        `Created ${result.createdCardCount} flashcard${result.createdCardCount === 1 ? "" : "s"} in a new deck.`,
      );
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
      await refreshDocuments().catch(() => undefined);
    } finally {
      setBusyDocumentId(null);
    }
  }

  async function openPdf(document: DocumentSummary): Promise<void> {
    try {
      const sourcePath = await getDocumentSourcePath(document.id);
      await openLocalPath(sourcePath);
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function toggleArchived(document: DocumentSummary): Promise<void> {
    if (isUploading || busyDocumentId || document.lifecycleState === "processing") return;
    setBusyDocumentId(document.id);
    try {
      await setDocumentArchived(document.id, document.revision, document.archivedAt === null);
      await refreshDocuments();
      showToast(
        "success",
        document.archivedAt === null ? "Document archived." : "Document restored.",
      );
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      setBusyDocumentId(null);
    }
  }

  return (
    <section className={styles.screen} aria-labelledby="documents-title">
      <header className={styles.header}>
        <div>
          <p className={styles.eyebrow}>Study sources</p>
          <h1 id="documents-title">Documents</h1>
          <p className={styles.intro}>
            Upload a PDF. ostudy stores it locally, extracts and renders every page, then uses your
            configured Flashcard generation profile to create a normal deck automatically.
          </p>
        </div>
        <button
          type="button"
          className="primary-button"
          disabled={isUploading || busyDocumentId !== null}
          onClick={() => void uploadPdf()}
        >
          {isUploading ? "Processing PDF…" : "Upload PDF"}
        </button>
      </header>

      <div className={styles.toolbar}>
        <label className={styles.archiveToggle}>
          <input
            type="checkbox"
            checked={includeArchived}
            onChange={(event) => setIncludeArchived(event.currentTarget.checked)}
          />
          Show archived documents
        </label>
        <span className={styles.count}>
          {documents.length} document{documents.length === 1 ? "" : "s"}
        </span>
      </div>

      {errorMessage ? (
        <div className={styles.error} role="alert">
          {errorMessage}
        </div>
      ) : null}

      {isLoading ? (
        <div className={styles.empty} aria-live="polite">
          Loading document library…
        </div>
      ) : documents.length === 0 ? (
        <div className={styles.empty}>
          <h2>No PDFs yet</h2>
          <p>
            Upload a course handout, chapter, or study packet to create a deck from its contents.
          </p>
        </div>
      ) : (
        <div className={styles.list}>
          {documents.map((document) => {
            const busy = busyDocumentId === document.id;
            const canRetry =
              document.generatedDeckId === null &&
              document.lifecycleState !== "processing" &&
              document.archivedAt === null;
            return (
              <article key={document.id} className={styles.card}>
                <div className={styles.cardMain}>
                  <div className={styles.titleRow}>
                    <h2>{document.title}</h2>
                    <span className={styles.status} data-state={document.lifecycleState}>
                      {statusLabel(document)}
                    </span>
                  </div>
                  <p className={styles.filename}>{document.originalFilename}</p>
                  <dl className={styles.metrics}>
                    <div>
                      <dt>File</dt>
                      <dd>{formatBytes(document.byteSize)}</dd>
                    </div>
                    <div>
                      <dt>Pages</dt>
                      <dd>
                        {document.renderedPageCount}/{document.pageCount || "—"} rendered
                      </dd>
                    </div>
                    <div>
                      <dt>Cards</dt>
                      <dd>{document.generatedCardCount}</dd>
                    </div>
                    <div>
                      <dt>Updated</dt>
                      <dd>{formatDate(document.updatedAt)}</dd>
                    </div>
                  </dl>
                  {document.lifecycleState === "processing" ? (
                    <p className={styles.progress} aria-live="polite">
                      Extracting text, rendering pages, and generating flashcards. Large PDFs may
                      take several minutes.
                    </p>
                  ) : null}
                  {document.safeErrorMessage ? (
                    <p className={styles.documentError}>{document.safeErrorMessage}</p>
                  ) : null}
                  {document.generatedDeckId ? (
                    <p className={styles.deckNote}>
                      The generated deck is available in Decks and uses the normal editing,
                      scheduling, archive, and deletion tools.
                    </p>
                  ) : document.generatedCardCount > 0 && document.lifecycleState === "ready" ? (
                    <p className={styles.deckNote}>
                      The previous generated deck was deleted. Retry generation to create a new
                      deck from the retained PDF.
                    </p>
                  ) : null}
                </div>
                <div className={styles.actions}>
                  <button
                    type="button"
                    className="secondary-button"
                    disabled={busy}
                    onClick={() => void openPdf(document)}
                  >
                    Open PDF
                  </button>
                  {canRetry ? (
                    <button
                      type="button"
                      className="primary-button"
                      disabled={busy || isUploading}
                      onClick={() => void retry(document)}
                    >
                      {busy ? "Retrying…" : "Retry generation"}
                    </button>
                  ) : null}
                  <button
                    type="button"
                    className="secondary-button"
                    disabled={busy || document.lifecycleState === "processing"}
                    onClick={() => void toggleArchived(document)}
                  >
                    {document.archivedAt === null ? "Archive" : "Restore"}
                  </button>
                </div>
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}

function statusLabel(document: DocumentSummary): string {
  if (document.archivedAt !== null) return "Archived";
  switch (document.lifecycleState) {
    case "processing":
      return "Processing";
    case "ready":
      return document.generatedDeckId ? "Deck created" : "Ready to regenerate";
    case "failed":
      return "Needs attention";
    default:
      return document.lifecycleState;
  }
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; index < units.length && value >= 1024; index += 1) {
    value /= 1024;
    unit = units[index];
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${unit}`;
}

function formatDate(timestamp: number): string {
  return new Date(timestamp).toLocaleString();
}
