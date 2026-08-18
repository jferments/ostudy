import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";

import type { ShowToast } from "../components/Toast";
import { getErrorCode, getErrorMessage } from "../ipc/common";
import {
  cancelPdfFlashcardGeneration,
  deletePdfDocument,
  generateMorePdfFlashcards,
  getDocumentGenerationProgress,
  importPdfAndGenerateFlashcards,
  listDocuments,
  openDocumentSource,
  retryPdfFlashcardGeneration,
  setDocumentArchived,
  type DocumentGenerationProgress,
  type DocumentSummary,
} from "../ipc/documents";
import { selectExternalImport } from "../platform/files";
import { useConfirm } from "../ui/overlays/overlayContext";
import { PageHeader } from "../ui/patterns/WorkspacePatterns";
import progressStyles from "./DocumentGenerationProgress.module.css";
import styles from "./DocumentsScreen.module.css";

interface DocumentsScreenProps {
  refreshDecks: () => Promise<unknown>;
  showToast: ShowToast;
  headerSupplement?: ReactNode;
}

export function DocumentsScreen({
  refreshDecks,
  showToast,
  headerSupplement,
}: DocumentsScreenProps) {
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [generationProgress, setGenerationProgress] = useState<
    Record<string, DocumentGenerationProgress | null>
  >({});
  const [clockNow, setClockNow] = useState(() => Date.now());
  const [includeArchived, setIncludeArchived] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isUploading, setIsUploading] = useState(false);
  const [busyDocumentId, setBusyDocumentId] = useState<string | null>(null);
  const [cancelingDocumentId, setCancelingDocumentId] = useState<string | null>(null);
  const [deletingDocumentId, setDeletingDocumentId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const confirm = useConfirm();

  const processingDocuments = useMemo(
    () => documents.filter((document) => document.lifecycleState === "processing"),
    [documents],
  );
  const hasProcessingDocument = processingDocuments.length > 0;

  const refreshDocuments = useCallback(async () => {
    const loaded = await listDocuments(includeArchived);
    setDocuments(loaded);
    setErrorMessage(null);
    return loaded;
  }, [includeArchived]);

  const refreshLiveProgress = useCallback(async () => {
    if (processingDocuments.length === 0) {
      setGenerationProgress({});
      return;
    }
    const entries = await Promise.all(
      processingDocuments.map(async (document) => {
        const progress = await getDocumentGenerationProgress(document.id).catch(() => null);
        return [document.id, progress] as const;
      }),
    );
    setGenerationProgress(Object.fromEntries(entries));
    setClockNow(Date.now());
  }, [processingDocuments]);

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
    const refresh = () => {
      void Promise.all([refreshDocuments(), refreshLiveProgress()]).catch(() => undefined);
    };
    refresh();
    const timer = window.setInterval(refresh, 1000);
    return () => window.clearInterval(timer);
  }, [busyDocumentId, hasProcessingDocument, isUploading, refreshDocuments, refreshLiveProgress]);

  useEffect(() => {
    if (!cancelingDocumentId) return;
    const current = documents.find((document) => document.id === cancelingDocumentId);
    if (!current || current.lifecycleState !== "processing") {
      setCancelingDocumentId(null);
    }
  }, [cancelingDocumentId, documents]);

  async function uploadPdf(): Promise<void> {
    if (isUploading || busyDocumentId || deletingDocumentId) return;
    const selected = await selectExternalImport("pdf_document");
    if (!selected) return;

    showToast("info", "Document processing started. Live stage, page, request, and token progress will appear here.");
    setIsUploading(true);
    setErrorMessage(null);
    try {
      const result = await importPdfAndGenerateFlashcards(selected.id);
      await Promise.all([refreshDocuments(), refreshDecks()]);
      showGenerationSuccess(result.document, result.createdCardCount, showToast);
    } catch (error: unknown) {
      await handleGenerationFailure(error);
    } finally {
      setIsUploading(false);
    }
  }

  async function retry(document: DocumentSummary): Promise<void> {
    if (isUploading || busyDocumentId || deletingDocumentId) return;
    setBusyDocumentId(document.id);
    setErrorMessage(null);
    try {
      const result = await retryPdfFlashcardGeneration(document.id);
      await Promise.all([refreshDocuments(), refreshDecks()]);
      showGenerationSuccess(result.document, result.createdCardCount, showToast);
    } catch (error: unknown) {
      await handleGenerationFailure(error);
    } finally {
      setBusyDocumentId(null);
    }
  }

  async function generateMore(document: DocumentSummary): Promise<void> {
    if (isUploading || busyDocumentId || deletingDocumentId || !document.generatedDeckId) return;
    setBusyDocumentId(document.id);
    setErrorMessage(null);
    try {
      const result = await generateMorePdfFlashcards(document.id);
      await Promise.all([refreshDocuments(), refreshDecks()]);
      if (result.createdCardCount === 0) {
        showToast("info", "The second pass did not find any additional non-duplicate cards to add.");
      } else {
        showToast(
          "success",
          `Added ${result.createdCardCount} new flashcard${result.createdCardCount === 1 ? "" : "s"} to the existing deck.`,
        );
      }
    } catch (error: unknown) {
      await handleGenerationFailure(error);
    } finally {
      setBusyDocumentId(null);
    }
  }

  async function handleGenerationFailure(error: unknown): Promise<void> {
    const code = getErrorCode(error);
    if (code === "document_generation_canceled" || code === "ai_request_canceled") {
      showToast("info", "Generation stopped. The PDF is still available to retry.");
      setErrorMessage(null);
    } else if (code === "document_no_additional_cards") {
      showToast("info", getErrorMessage(error));
      setErrorMessage(null);
    } else {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    }
    await refreshDocuments().catch(() => undefined);
  }

  async function stopGeneration(document: DocumentSummary): Promise<void> {
    if (cancelingDocumentId) return;
    setCancelingDocumentId(document.id);
    try {
      await cancelPdfFlashcardGeneration(document.id);
      showToast("info", "Stopping the current AI request…");
    } catch (error: unknown) {
      setCancelingDocumentId(null);
      const code = getErrorCode(error);
      if (code !== "document_generation_not_running") {
        showToast("error", getErrorMessage(error));
      }
      await refreshDocuments().catch(() => undefined);
    }
  }

  async function openPdf(document: DocumentSummary): Promise<void> {
    try {
      await openDocumentSource(document.id);
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
  }

  async function deleteDocument(document: DocumentSummary): Promise<void> {
    if (
      isUploading ||
      busyDocumentId ||
      deletingDocumentId ||
      document.lifecycleState === "processing"
    ) {
      return;
    }
    const hasSurvivingGeneratedDeck = document.generatedDeckId !== null;
    const accepted = await confirm({
      title: "Delete this PDF permanently?",
      description: hasSurvivingGeneratedDeck
        ? "The managed source PDF, rendered pages, extracted structure, outline, and generation history for this document will be removed. The surviving generated deck will remain and its source will be preserved as a text citation instead of a file link. This cannot be undone."
        : "The managed source PDF, rendered pages, extracted structure, outline, and generation history for this document will be removed. No active generated deck remains, so no source citation will be preserved. This cannot be undone.",
      confirmLabel: "Delete PDF",
      destructive: true,
    });
    if (!accepted) return;

    setDeletingDocumentId(document.id);
    try {
      const result = await deletePdfDocument(document.id, document.revision);
      await Promise.all([refreshDocuments(), refreshDecks()]);
      showToast(
        "success",
        result.preservedDeckCitationCount > 0
          ? `PDF deleted. Preserved text source citations in ${result.preservedDeckCitationCount} generated deck${result.preservedDeckCitationCount === 1 ? "" : "s"}.`
          : "PDF deleted from Documents.",
      );
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
    } finally {
      setDeletingDocumentId(null);
    }
  }

  async function toggleArchived(document: DocumentSummary): Promise<void> {
    if (
      isUploading ||
      busyDocumentId ||
      deletingDocumentId ||
      document.lifecycleState === "processing"
    ) {
      return;
    }
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
    <section className={styles.screen} aria-label="Documents">
      <PageHeader
        title="Documents"
        icon="document"
        compact
        actions={
          <button
            type="button"
            className="primary-button"
            disabled={isUploading || busyDocumentId !== null || deletingDocumentId !== null}
            onClick={() => void uploadPdf()}
          >
            {isUploading ? "Processing PDF…" : "Upload PDF"}
          </button>
        }
      />

      {headerSupplement}

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
          <p>Upload a course handout, chapter, or study packet to create a deck from its contents.</p>
        </div>
      ) : (
        <div className={styles.list}>
          {documents.map((document) => {
            const busy = busyDocumentId === document.id || deletingDocumentId === document.id;
            const canceling = cancelingDocumentId === document.id;
            const live = generationProgress[document.id] ?? null;
            const canRetry =
              document.generatedDeckId === null &&
              document.lifecycleState !== "processing" &&
              document.archivedAt === null;
            const canGenerateMore =
              document.generatedDeckId !== null &&
              document.lifecycleState === "ready" &&
              document.archivedAt === null;
            return (
              <article key={document.id} className={styles.card}>
                <div className={styles.cardMain}>
                  <div className={styles.titleRow}>
                    <h2>{document.title}</h2>
                    <span className={styles.status} data-state={document.lifecycleState}>
                      {statusLabel(document)}
                    </span>
                    {document.lifecycleState === "processing" ? (
                      <span className={styles.processingPhase} aria-live="polite">
                        {processingPhaseLabel(document, canceling, live)}
                      </span>
                    ) : null}
                  </div>
                  <p className={styles.filename}>{document.originalFilename}</p>
                  <dl className={styles.metrics}>
                    <div>
                      <dt>File</dt>
                      <dd>{formatBytes(document.byteSize)}</dd>
                    </div>
                    <div>
                      <dt>Pages</dt>
                      <dd>{document.renderedPageCount}/{document.pageCount || "—"} rendered</dd>
                    </div>
                    <div>
                      <dt>Cards</dt>
                      <dd>{document.generatedCardCount}</dd>
                    </div>
                    <div>
                      <dt>{document.lifecycleState === "processing" ? "Running" : "Processing"}</dt>
                      <dd>
                        {document.lifecycleState === "processing"
                          ? formatLiveDuration(live?.startedAt ?? null, clockNow)
                          : formatProcessingDuration(document.processingDurationMs)}
                      </dd>
                    </div>
                    <div>
                      <dt>API tokens</dt>
                      <dd>{formatApiTokens(document, live)}</dd>
                    </div>
                    <div>
                      <dt>Updated</dt>
                      <dd>{formatDate(document.updatedAt)}</dd>
                    </div>
                  </dl>
                  {document.lifecycleState === "processing" ? (
                    <GenerationProgressPanel document={document} progress={live} now={clockNow} />
                  ) : null}
                  {document.safeErrorMessage ? (
                    <p className={styles.documentError}>{document.safeErrorMessage}</p>
                  ) : null}
                  {!document.generatedDeckId &&
                  document.generatedCardCount > 0 &&
                  document.lifecycleState === "ready" ? (
                    <p className={styles.deckNote}>
                      The previous generated deck was deleted. Retry generation to create a new deck
                      from the retained PDF.
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
                  {document.lifecycleState === "processing" ? (
                    <button
                      type="button"
                      className="secondary-button"
                      disabled={canceling}
                      onClick={() => void stopGeneration(document)}
                    >
                      {canceling ? "Stopping…" : "Stop generation"}
                    </button>
                  ) : null}
                  {canRetry ? (
                    <button
                      type="button"
                      className="primary-button"
                      disabled={busy || isUploading}
                      onClick={() => void retry(document)}
                    >
                      {busyDocumentId === document.id ? "Retrying…" : "Retry generation"}
                    </button>
                  ) : null}
                  {canGenerateMore ? (
                    <button
                      type="button"
                      className="primary-button"
                      disabled={busy || isUploading}
                      onClick={() => void generateMore(document)}
                    >
                      {busyDocumentId === document.id ? "Generating…" : "Generate more cards"}
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
                  {document.lifecycleState !== "processing" ? (
                    <button
                      type="button"
                      className="danger-button"
                      disabled={busy || isUploading}
                      onClick={() => void deleteDocument(document)}
                    >
                      {deletingDocumentId === document.id ? "Deleting…" : "Delete PDF"}
                    </button>
                  ) : null}
                </div>
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}

function GenerationProgressPanel({
  document,
  progress,
  now,
}: {
  document: DocumentSummary;
  progress: DocumentGenerationProgress | null;
  now: number;
}) {
  const stage = generationStage(document, progress);
  const percent = generationPercent(document, progress);
  const promptTokens = progress?.promptTokens ?? 0;
  const cacheRatio =
    promptTokens > 0 && progress
      ? Math.min(100, (progress.cachedPromptTokens / promptTokens) * 100)
      : null;
  return (
    <div className={progressStyles.panel} aria-live="polite">
      <div className={progressStyles.heading}>
        <span>{stage.title}</span>
        <span>{percent === null ? "Working…" : `${percent}%`}</span>
      </div>
      <div
        className={progressStyles.track}
        role="progressbar"
        aria-label="Document generation progress"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={percent ?? undefined}
      >
        <div className={progressStyles.fill} style={{ width: `${percent ?? 3}%` }} />
      </div>
      <div className={progressStyles.detail}>{stage.detail}</div>
      <dl className={progressStyles.stats}>
        <div>
          <dt>Elapsed</dt>
          <dd>{formatLiveDuration(progress?.startedAt ?? null, now)}</dd>
        </div>
        <div>
          <dt>Requests</dt>
          <dd>{progress ? progress.providerRequestCount.toLocaleString() : "—"}</dd>
        </div>
        <div>
          <dt>Retries</dt>
          <dd className={progress && progress.retryCount > 0 ? progressStyles.warning : undefined}>
            {progress ? progress.retryCount.toLocaleString() : "—"}
          </dd>
        </div>
        <div>
          <dt>Reasoning</dt>
          <dd>
            {progress
              ? `${progress.reasoningEffort} · ${formatTokenCount(progress.reasoningTokens)}`
              : "—"}
          </dd>
        </div>
        <div>
          <dt>Prompt / output</dt>
          <dd>
            {progress
              ? `${formatTokenCount(progress.promptTokens)} / ${formatTokenCount(progress.completionTokens)}`
              : "—"}
          </dd>
        </div>
        <div>
          <dt>Prompt cache</dt>
          <dd>
            {progress && cacheRatio !== null
              ? `${formatTokenCount(progress.cachedPromptTokens)} (${cacheRatio.toFixed(1)}%)`
              : "Awaiting usage"}
          </dd>
        </div>
      </dl>
    </div>
  );
}

function generationStage(
  document: DocumentSummary,
  progress: DocumentGenerationProgress | null,
): { title: string; detail: string } {
  if (document.pageCount === 0) {
    return { title: "Extracting document text", detail: "Reading the PDF and building semantic source chunks." };
  }
  if (document.renderedPageCount < document.pageCount) {
    return {
      title: `Rendering pages ${document.renderedPageCount}/${document.pageCount}`,
      detail: "Preparing local page derivatives for document analysis.",
    };
  }
  if (
    document.extractionMode === "ocr_fallback" &&
    document.ocrPageCount > document.ocrCompletedPageCount
  ) {
    return {
      title: `Recovering OCR ${document.ocrCompletedPageCount}/${document.ocrPageCount}`,
      detail: "Recovering text from pages whose native PDF text layer was unreliable.",
    };
  }
  if (document.outlineWindowCount > 0 && document.outlineCompletedWindowCount < document.outlineWindowCount) {
    return {
      title: "Building outline",
      detail:
        activeWindowDetail(progress, document.outlineWindowCount) ??
        "Analyzing bounded source windows for document structure.",
    };
  }
  if (
    document.generationWindowCount > 0 &&
    document.generationCompletedWindowCount < document.generationWindowCount
  ) {
    return {
      title: "Generating cards",
      detail:
        activeWindowDetail(progress, document.generationWindowCount) ??
        "Generating study cards from the current bounded source window.",
    };
  }
  if (progress?.phase === "curating") {
    return {
      title: "Curating generated cards",
      detail: "Removing duplicates and low-value candidates before committing the deck.",
    };
  }
  return {
    title: "Finalizing document generation",
    detail: "Completing the current bounded task and preparing durable study content.",
  };
}

function activeWindowDetail(
  progress: DocumentGenerationProgress | null,
  totalWindows: number,
): string | null {
  const start = progress?.currentWindow;
  if (start === null || start === undefined) return null;
  const end = progress?.currentWindowEnd ?? start;
  const pages =
    progress?.currentPageStart !== null && progress?.currentPageStart !== undefined
      ? ` · pages ${progress.currentPageStart}–${progress.currentPageEnd ?? progress.currentPageStart}`
      : "";
  if (end > start) {
    return `Active windows ${start}–${end} of ${totalWindows}${pages}.`;
  }
  return `Window ${start} of ${totalWindows}${pages}.`;
}

function generationPercent(
  document: DocumentSummary,
  progress: DocumentGenerationProgress | null,
): number | null {
  if (document.pageCount > 0 && document.renderedPageCount < document.pageCount) {
    return Math.round((document.renderedPageCount / document.pageCount) * 20);
  }
  const totalWindows = document.outlineWindowCount + document.generationWindowCount;
  if (totalWindows > 0) {
    const completed = document.outlineCompletedWindowCount + document.generationCompletedWindowCount;
    return Math.min(98, Math.round(20 + (completed / totalWindows) * 75));
  }
  if (progress?.phase === "curating") return 97;
  return null;
}

function hasCompletedOcrRecovery(document: DocumentSummary): boolean {
  return (
    document.extractionMode === "ocr_fallback" &&
    document.ocrPageCount > 0 &&
    document.ocrCompletedPageCount === document.ocrPageCount &&
    document.ocrMeanConfidence !== null
  );
}

function showGenerationSuccess(
  document: DocumentSummary,
  createdCardCount: number,
  showToast: ShowToast,
): void {
  const cards = `Created ${createdCardCount} flashcard${createdCardCount === 1 ? "" : "s"} in a new deck.`;
  if (hasCompletedOcrRecovery(document)) {
    showToast(
      "success",
      `${cards} OCR fallback recovered ${document.ocrPageCount} page${document.ocrPageCount === 1 ? "" : "s"}; review cards against the source because OCR can be less accurate.`,
    );
    return;
  }
  showToast("success", cards);
}

function statusLabel(document: DocumentSummary): string {
  if (document.archivedAt !== null) return "Archived";
  if (
    document.safeErrorCode === "document_generation_canceled" ||
    document.safeErrorCode === "ai_request_canceled"
  ) {
    return "Stopped";
  }
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

function processingPhaseLabel(
  document: DocumentSummary,
  canceling: boolean,
  progress: DocumentGenerationProgress | null,
): string {
  if (canceling) return "Stopping";
  if (document.generatedDeckId !== null) return "Generating additional cards";
  const stage = generationStage(document, progress);
  return stage.title;
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

function formatProcessingDuration(value: number | null): string {
  if (value === null) return "Not recorded";
  if (value < 1_000) return "<1 sec";
  if (value < 60_000) return Math.round(value / 1_000) + " sec";
  if (value < 3_600_000) return (value / 60_000).toFixed(value < 600_000 ? 1 : 0) + " min";
  return (value / 3_600_000).toFixed(1) + " hr";
}

function formatLiveDuration(startedAt: number | null, now: number): string {
  if (startedAt === null) return "Starting…";
  const elapsed = Math.max(0, now - startedAt);
  if (elapsed < 60_000) return `${Math.max(1, Math.floor(elapsed / 1_000))} sec`;
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 60) return `${minutes}m ${Math.floor((elapsed % 60_000) / 1_000)}s`;
  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

function formatApiTokens(
  document: DocumentSummary,
  progress: DocumentGenerationProgress | null,
): string {
  if (document.lifecycleState === "processing") {
    if (progress && progress.totalTokens > 0) return formatTokenCount(progress.totalTokens);
    return "Awaiting provider usage";
  }
  if (document.apiTokenCount === null) return "Not reported by provider";
  const formatted = document.apiTokenCount.toLocaleString();
  return document.apiTokenUsageComplete ? formatted : formatted + "+ reported";
}

function formatTokenCount(value: number): string {
  if (value < 1_000) return value.toLocaleString();
  if (value < 1_000_000) return `${(value / 1_000).toFixed(value < 10_000 ? 1 : 0)}k`;
  return `${(value / 1_000_000).toFixed(1)}m`;
}

function formatDate(timestamp: number): string {
  return new Date(timestamp).toLocaleString();
}