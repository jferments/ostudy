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
import {
  cancelDocumentStudyGuideGeneration,
  generateDocumentStudyGuide,
} from "../ipc/studyGuides";
import { selectExternalImport } from "../platform/files";
import { useConfirm } from "../ui/overlays/overlayContext";
import { PageHeader } from "../ui/patterns/WorkspacePatterns";
import progressStyles from "./DocumentGenerationProgress.module.css";
import styles from "./DocumentsScreen.module.css";
import { Button } from "../ui/primitives/Button";

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
  const [guideGeneratingDocumentId, setGuideGeneratingDocumentId] = useState<string | null>(null);
  const [cancelingGuideDocumentId, setCancelingGuideDocumentId] = useState<string | null>(null);
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

  async function uploadDocument(): Promise<void> {
    if (isUploading || busyDocumentId || deletingDocumentId || guideGeneratingDocumentId) return;
    const selected = await selectExternalImport("document");
    if (!selected) return;

    showToast(
      "info",
      "Document processing started. Live stage, source-unit, request, and token progress will appear here.",
    );
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
    if (isUploading || busyDocumentId || deletingDocumentId || guideGeneratingDocumentId) return;
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
    if (
      isUploading ||
      busyDocumentId ||
      deletingDocumentId ||
      guideGeneratingDocumentId ||
      !document.generatedDeckId
    ) {
      return;
    }
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

  async function generateStudyGuide(document: DocumentSummary): Promise<void> {
    if (
      isUploading ||
      busyDocumentId ||
      deletingDocumentId ||
      guideGeneratingDocumentId ||
      document.archivedAt !== null ||
      document.lifecycleState === "processing"
    ) {
      return;
    }
    setGuideGeneratingDocumentId(document.id);
    setCancelingGuideDocumentId(null);
    setErrorMessage(null);
    try {
      const guide = await generateDocumentStudyGuide(document.id);
      await refreshDocuments();
      showToast(
        "success",
        `Study guide revision ${guide.revision} created. Open it from the Practice screen.`,
      );
    } catch (error: unknown) {
      const code = getErrorCode(error);
      if (code === "document_study_guide_canceled" || code === "ai_request_canceled") {
        setErrorMessage(null);
        showToast("info", "Study-guide generation stopped. Saved topic progress can be reused on retry.");
      } else {
        const message = getErrorMessage(error);
        setErrorMessage(message);
        showToast("error", message);
      }
      await refreshDocuments().catch(() => undefined);
    } finally {
      setGuideGeneratingDocumentId(null);
      setCancelingGuideDocumentId(null);
    }
  }

  async function stopStudyGuide(document: DocumentSummary): Promise<void> {
    if (guideGeneratingDocumentId !== document.id || cancelingGuideDocumentId) return;
    setCancelingGuideDocumentId(document.id);
    try {
      await cancelDocumentStudyGuideGeneration(document.id);
      showToast("info", "Stopping study-guide generation…");
    } catch (error: unknown) {
      setCancelingGuideDocumentId(null);
      const code = getErrorCode(error);
      if (code !== "document_generation_not_running") {
        showToast("error", getErrorMessage(error));
      }
    }
  }

  async function handleGenerationFailure(error: unknown): Promise<void> {
    const code = getErrorCode(error);
    if (code === "document_generation_canceled" || code === "ai_request_canceled") {
      showToast("info", "Generation stopped. The document is still available to retry.");
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

  async function openSource(document: DocumentSummary): Promise<void> {
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
      guideGeneratingDocumentId ||
      document.lifecycleState === "processing"
    ) {
      return;
    }
    const hasSurvivingGeneratedDeck = document.generatedDeckId !== null;
    const accepted = await confirm({
      title: "Delete this document permanently?",
      description: hasSurvivingGeneratedDeck
        ? "The managed source, extracted structure, local derivatives, outline, study guides, annotations, and generation history for this document will be removed. The surviving generated deck will remain and its source will be preserved as a text citation instead of a file link. This cannot be undone."
        : "The managed source, extracted structure, local derivatives, outline, study guides, annotations, and generation history for this document will be removed. No active generated deck remains, so no source citation will be preserved. This cannot be undone.",
      confirmLabel: "Delete document",
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
          ? `Document deleted. Preserved text source citations in ${result.preservedDeckCitationCount} generated deck${result.preservedDeckCitationCount === 1 ? "" : "s"}.`
          : "Document deleted from Documents.",
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
      guideGeneratingDocumentId ||
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
          <Button
            type="button"
            variant="primary"
            disabled={
              isUploading ||
              busyDocumentId !== null ||
              deletingDocumentId !== null ||
              guideGeneratingDocumentId !== null
            }
            onClick={() => void uploadDocument()}
          >
            {isUploading ? "Processing document…" : "Upload document"}
          </Button>
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
          <h2>No documents yet</h2>
          <p>Upload a course handout, chapter, presentation, or study packet to create learning materials.</p>
        </div>
      ) : (
        <div className={styles.list}>
          {documents.map((document) => {
            const guideGenerating = guideGeneratingDocumentId === document.id;
            const busy =
              busyDocumentId === document.id ||
              deletingDocumentId === document.id ||
              guideGenerating;
            const canceling = cancelingDocumentId === document.id;
            const cancelingGuide = cancelingGuideDocumentId === document.id;
            const live = generationProgress[document.id] ?? null;
            const canRetry =
              document.generatedDeckId === null &&
              document.lifecycleState !== "processing" &&
              document.archivedAt === null;
            const canGenerateMore =
              document.generatedDeckId !== null &&
              document.lifecycleState === "ready" &&
              document.archivedAt === null;
            const canGenerateGuide =
              document.lifecycleState !== "processing" && document.archivedAt === null;
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
                    ) : guideGenerating ? (
                      <span className={styles.processingPhase} aria-live="polite">
                        {cancelingGuide ? "Stopping study guide" : "Generating study guide"}
                      </span>
                    ) : null}
                  </div>
                  <p className={styles.filename}>{document.originalFilename}</p>
                  <dl className={styles.metrics}>
                    <div>
                      <dt>Format</dt>
                      <dd>
                        {formatDocumentType(document.sourceFormat)} · {formatBytes(document.byteSize)}
                      </dd>
                    </div>
                    <div>
                      <dt>{sourceUnitHeading(document.sourceUnitKind)}</dt>
                      <dd>{formatSourceUnitMetric(document)}</dd>
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
                      from the retained document.
                    </p>
                  ) : null}
                </div>
                <div className={styles.actions}>
                  <Button
                    type="button"
                    variant="secondary"
                    disabled={busy}
                    onClick={() => void openSource(document)}
                  >
                    Open source
                  </Button>
                  {document.lifecycleState === "processing" ? (
                    <Button
                      type="button"
                      variant="secondary"
                      disabled={canceling}
                      onClick={() => void stopGeneration(document)}
                    >
                      {canceling ? "Stopping…" : "Stop generation"}
                    </Button>
                  ) : null}
                  {canGenerateGuide ? (
                    <Button
                      type="button"
                      variant="primary"
                      disabled={busy || isUploading}
                      onClick={() => void generateStudyGuide(document)}
                    >
                      {guideGenerating ? "Generating study guide…" : "Generate study guide"}
                    </Button>
                  ) : null}
                  {guideGenerating ? (
                    <Button
                      type="button"
                      variant="secondary"
                      disabled={cancelingGuide}
                      onClick={() => void stopStudyGuide(document)}
                    >
                      {cancelingGuide ? "Stopping…" : "Stop study guide"}
                    </Button>
                  ) : null}
                  {canRetry ? (
                    <Button
                      type="button"
                      variant="primary"
                      disabled={busy || isUploading}
                      onClick={() => void retry(document)}
                    >
                      {busyDocumentId === document.id ? "Retrying…" : "Retry generation"}
                    </Button>
                  ) : null}
                  {canGenerateMore ? (
                    <Button
                      type="button"
                      variant="primary"
                      disabled={busy || isUploading}
                      onClick={() => void generateMore(document)}
                    >
                      {busyDocumentId === document.id ? "Generating…" : "Generate more cards"}
                    </Button>
                  ) : null}
                  <Button
                    type="button"
                    variant="secondary"
                    disabled={busy || document.lifecycleState === "processing"}
                    onClick={() => void toggleArchived(document)}
                  >
                    {document.archivedAt === null ? "Archive" : "Restore"}
                  </Button>
                  {document.lifecycleState !== "processing" ? (
                    <Button
                      type="button"
                      variant="danger"
                      disabled={busy || isUploading}
                      onClick={() => void deleteDocument(document)}
                    >
                      {deletingDocumentId === document.id ? "Deleting…" : "Delete document"}
                    </Button>
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
          <dt>Provider</dt>
          <dd>{formatProviderActivity(progress, now)}</dd>
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

function formatProviderActivity(
  progress: DocumentGenerationProgress | null,
  now: number,
): string {
  if (!progress) return "Preparing";
  const providerAge = activityAgeSeconds(progress.lastProviderActivityAt, now);
  const modelAge = activityAgeSeconds(progress.lastModelActivityAt, now);
  switch (progress.providerActivity) {
    case "streaming":
      if (modelAge === null) return `Responsive, waiting${activityAgeSuffix(providerAge)}`;
      if (modelAge < 2) return "Receiving output · now";
      if (providerAge !== null && providerAge < modelAge) {
        return `Responsive · output ${formatActivityAge(modelAge)}`;
      }
      return `Receiving output · ${formatActivityAge(modelAge)}`;
    case "responsive":
      return `Responsive, waiting${activityAgeSuffix(providerAge)}`;
    case "connecting":
      return "Connecting";
    default:
      return "Preparing";
  }
}

function activityAgeSeconds(timestamp: number | null, now: number): number | null {
  return timestamp === null ? null : Math.max(0, Math.floor((now - timestamp) / 1_000));
}

function activityAgeSuffix(ageSeconds: number | null): string {
  return ageSeconds === null ? "" : ` · ${formatActivityAge(ageSeconds)}`;
}

function formatActivityAge(ageSeconds: number): string {
  if (ageSeconds < 2) return "now";
  if (ageSeconds < 60) return `${ageSeconds}s ago`;
  const minutes = Math.floor(ageSeconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m ago`;
}

function generationStage(
  document: DocumentSummary,
  progress: DocumentGenerationProgress | null,
): { title: string; detail: string } {
  if (document.sourceUnitCount === 0) {
    return ingestionStage(progress?.phase) ?? {
      title: "Extracting document content",
      detail: `Reading the ${formatDocumentType(document.sourceFormat)} source and building semantic chunks.`,
    };
  }
  if (isPdf(document) && document.renderedPageCount < document.sourceUnitCount) {
    return {
      title: `Rendering pages ${document.renderedPageCount}/${document.sourceUnitCount}`,
      detail: "Preparing local page derivatives for document analysis.",
    };
  }
  if (
    isPdf(document) &&
    document.extractionMode === "ocr_fallback" &&
    document.ocrPageCount > document.ocrCompletedPageCount
  ) {
    return {
      title: `Recovering OCR ${document.ocrCompletedPageCount}/${document.ocrPageCount}`,
      detail: "Recovering text from pages whose native PDF text layer was unreliable.",
    };
  }
  if (
    document.outlineWindowCount > 0 &&
    document.outlineCompletedWindowCount < document.outlineWindowCount
  ) {
    return {
      title: "Building outline",
      detail:
        activeWindowDetail(
          progress,
          document.outlineWindowCount,
          document.sourceUnitKind,
        ) ?? "Analyzing bounded source windows for document structure.",
    };
  }
  if (
    document.generationWindowCount > 0 &&
    document.generationCompletedWindowCount < document.generationWindowCount
  ) {
    return {
      title: "Generating cards",
      detail:
        activeWindowDetail(
          progress,
          document.generationWindowCount,
          document.sourceUnitKind,
        ) ?? "Generating study cards from the current bounded source window.",
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

function ingestionStage(phase: string | undefined): { title: string; detail: string } | null {
  switch (phase) {
    case "document_detecting":
      return {
        title: "Checking document format",
        detail: "Validating the managed source before local extraction begins.",
      };
    case "document_extracting":
      return {
        title: "Reading document structure",
        detail: "Extracting text, tables, slides, sheets, and embedded images locally.",
      };
    case "document_mapping":
      return {
        title: "Mapping document structure",
        detail: "Converting extracted headings and content into oStudy source blocks.",
      };
    case "document_saving_assets":
      return {
        title: "Saving embedded images",
        detail: "Retaining bounded source images as immutable local evidence.",
      };
    case "document_unitizing":
      return {
        title: "Building source sections",
        detail: "Creating deterministic source sections for study generation.",
      };
    case "document_chunking":
      return {
        title: "Building semantic chunks",
        detail: "Preparing bounded, source-linked chunks for document analysis.",
      };
    case "document_validating":
      return {
        title: "Validating extracted document",
        detail: "Checking canonical source records before they are committed.",
      };
    default:
      return null;
  }
}

function activeWindowDetail(
  progress: DocumentGenerationProgress | null,
  totalWindows: number,
  sourceUnitKind: string,
): string | null {
  const start = progress?.currentWindow;
  if (start === null || start === undefined) return null;
  const end = progress?.currentWindowEnd ?? start;
  const sourceUnits =
    progress?.currentPageStart !== null && progress?.currentPageStart !== undefined
      ? ` · ${sourceUnitPluralLabel(sourceUnitKind)} ${progress.currentPageStart}–${progress.currentPageEnd ?? progress.currentPageStart}`
      : "";
  if (end > start) {
    return `Active windows ${start}–${end} of ${totalWindows}${sourceUnits}.`;
  }
  return `Window ${start} of ${totalWindows}${sourceUnits}.`;
}

function generationPercent(
  document: DocumentSummary,
  progress: DocumentGenerationProgress | null,
): number | null {
  if (
    isPdf(document) &&
    document.sourceUnitCount > 0 &&
    document.renderedPageCount < document.sourceUnitCount
  ) {
    return Math.round((document.renderedPageCount / document.sourceUnitCount) * 20);
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
    isPdf(document) &&
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
      return document.generatedDeckId ? "Deck created" : "Ready";
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
  return generationStage(document, progress).title;
}

function isPdf(document: DocumentSummary): boolean {
  return document.sourceFormat === "pdf";
}

function formatDocumentType(sourceFormat: string): string {
  switch (sourceFormat) {
    case "pdf":
      return "PDF";
    case "doc":
    case "docx":
      return "Word";
    case "ppt":
    case "pptx":
      return "PowerPoint";
    case "odt":
      return "OpenDocument text";
    case "odp":
      return "OpenDocument presentation";
    case "rtf":
      return "RTF";
    case "epub":
      return "EPUB";
    case "excel":
    case "xls":
    case "xlsx":
      return "Excel";
    case "ods":
      return "OpenDocument spreadsheet";
    case "csv":
      return "CSV";
    default:
      return sourceFormat || "Document";
  }
}

function sourceUnitHeading(sourceUnitKind: string): string {
  const plural = sourceUnitPluralLabel(sourceUnitKind);
  return plural.charAt(0).toUpperCase() + plural.slice(1);
}

function sourceUnitPluralLabel(sourceUnitKind: string): string {
  switch (sourceUnitKind) {
    case "page":
      return "pages";
    case "slide":
      return "slides";
    case "chapter":
      return "chapters";
    case "section":
      return "sections";
    case "segment":
      return "segments";
    case "image":
      return "images";
    case "time_segment":
      return "time segments";
    default:
      return "source units";
  }
}

function formatSourceUnitMetric(document: DocumentSummary): string {
  if (document.sourceUnitCount <= 0) return "—";
  if (isPdf(document)) {
    return `${document.renderedPageCount}/${document.sourceUnitCount} rendered`;
  }
  return document.sourceUnitCount.toLocaleString();
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
