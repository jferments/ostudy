import type { DocumentGenerationProgress, DocumentSummary } from "../../ipc/documents";
import { progressStyles } from "./presentation";
import { Progress } from "../../ui/primitives/Progress";
import { Disclosure } from "../../ui/primitives/FormControls";

/** A document's processing as one bar in its row, with the current stage beside it. */
export function DocumentRowProgress({ label, value }: { label: string; value: number | null }) {
  return <span className="tw:flex tw:min-w-0 tw:items-center tw:gap-3" data-testid="document-row-progress">
    <Progress value={value} max={100} label="Document progress" valueText={label} className="tw:max-w-75 tw:flex-1" />
    <span className="tw:min-w-0 tw:truncate tw:text-small tw:text-muted-foreground tw:tabular-nums">{label}</span>
  </span>;
}

export function GenerationProgressPanel({ document, progress }: {
  document: DocumentSummary; progress: DocumentGenerationProgress | null;
}) {
  const stage = generationStage(document, progress);
  const percent = generationPercent(document, progress);
  return <div className={progressStyles.panel} aria-live="polite">
    <div className={progressStyles.heading}><span>{stage.title}</span><span>{percent === null ? "Working…" : `${percent}%`}</span></div>
    <Progress value={percent} max={100} label="Document generation progress" />
    <div className={progressStyles.detail}>{stage.detail}</div>
  </div>;
}

export function GenerationDiagnostics({ progress, now }: {
  progress: DocumentGenerationProgress | null; now: number;
}) {
  const hasUsage = progress !== null && progress.usageReportedRequests > 0;
  const promptTokens = progress?.promptTokens ?? 0;
  const cacheRatio = hasUsage && promptTokens > 0 && progress
    ? Math.min(100, (progress.cachedPromptTokens / promptTokens) * 100) : null;
  const retryDetails = progress ? [
    { label: "Response contract repair", count: progress.contractRepairRetryCount, explanation: "Retry after a response failed its required structure or content contract." },
    { label: "Smaller output", count: progress.outputCompactionRetryCount, explanation: "Retry with a reduced output size after a completion limit." },
    { label: "Temporary transport failure", count: progress.transientRetryCount, explanation: "Retry after a transient provider or network failure." },
    { label: "Provider compatibility", count: progress.compatibilityRetryCount, explanation: "Retry using the supported provider request format." },
  ] : [];
  return <>
    <dl className={progressStyles.stats}>
      <div><dt>Elapsed</dt><dd>{formatLiveDuration(progress?.startedAt ?? null, now)}</dd></div>
      <div><dt>Stage elapsed</dt><dd>{formatLiveDuration(progress?.phaseStartedAt ?? null, now)}</dd></div>
      <div><dt>Requests</dt><dd>{progress ? progress.providerRequestCount.toLocaleString() : "—"}</dd></div>
      <div><dt>Provider activity</dt><dd>{formatProviderActivity(progress, now)}</dd></div>
      <div><dt>Retries</dt><dd className={progress && progress.retryCount > 0 ? progressStyles.warning : undefined}>{progress ? progress.retryCount.toLocaleString() : "—"}</dd></div>
      <div><dt>Reasoning</dt><dd>{progress ? `${formatReasoning(progress)} · ${hasUsage ? formatTokenCount(progress.reasoningTokens) : "Awaiting usage"}` : "—"}</dd></div>
      <div><dt>Prompt / output</dt><dd>{progress && hasUsage ? `${formatTokenCount(progress.promptTokens)} / ${formatTokenCount(progress.completionTokens)}` : "Awaiting usage"}</dd></div>
      <div><dt>Prompt cache</dt><dd>{progress && hasUsage ? `${formatTokenCount(progress.cachedPromptTokens)}${cacheRatio !== null ? ` (${cacheRatio.toFixed(1)}%)` : ""}` : "Awaiting usage"}</dd></div>
    </dl>
    {progress && progress.usageUnreportedRequests > 0 ? <p className={progressStyles.detail}>
      Usage incomplete: {progress.usageUnreportedRequests.toLocaleString()} requests did not report token counts. Totals and cache ratios describe reported usage only.
    </p> : null}
    {progress ? <Disclosure title="Provider and request" className="tw:border-0">
      <div className="tw:grid tw:gap-3 tw:p-3">
        <dl className={progressStyles.stats}>
          <div><dt>Provider</dt><dd>{progress.providerKind ?? "Not selected yet"}</dd></div>
          <div><dt>Model</dt><dd className="tw:break-all">{progress.modelId ?? "Not selected yet"}</dd></div>
          <div><dt>Requested reasoning</dt><dd>{progress.requestedReasoningEffort ?? "Not resolved yet"}</dd></div>
          <div><dt>Effective reasoning</dt><dd>{progress.reasoningEffort}</dd></div>
          <div><dt>Usage reports</dt><dd>{progress.usageReportedRequests.toLocaleString()} reported requests</dd></div>
          <div><dt>Cache writes</dt><dd>{hasUsage ? `${formatTokenCount(progress.cacheWriteTokens)} tokens` : "Awaiting usage"}</dd></div>
        </dl>
        {progress.currentRequestStartedAt !== null ? <section className="tw:grid tw:gap-2">
          <strong className="tw:text-small">Current provider request</strong>
          <dl className={progressStyles.stats}>
            <div><dt>Stage</dt><dd>{formatDiagnosticStage(progress.currentRequestStage)}</dd></div>
            <div><dt>Request elapsed</dt><dd>{formatLiveDuration(progress.currentRequestStartedAt, now)}</dd></div>
            <div><dt>Wire size</dt><dd>{formatOptionalBytes(progress.currentRequestBytes)}</dd></div>
            <div><dt>Text/schema</dt><dd>{formatOptionalBytes(progress.currentRequestTextBytes)}</dd></div>
            <div><dt>Evidence payload</dt><dd>{formatOptionalBytes(progress.currentRequestEvidenceBytes)}</dd></div>
            <div><dt>Rendered images</dt><dd>{progress.currentRequestImageCount?.toLocaleString() ?? "Unknown"}</dd></div>
            <div><dt>Output ceiling</dt><dd>{progress.currentRequestMaxOutputTokens === null ? "Unknown" : `${progress.currentRequestMaxOutputTokens.toLocaleString()} tokens`}</dd></div>
          </dl>
        </section> : null}
        <dl className="tw:m-0 tw:grid tw:gap-2 tw:text-small">
          {retryDetails.map(({ label, count, explanation }) => <div key={label}>
            <dt className="tw:font-medium">{label}</dt>
            <dd className="tw:m-0 tw:text-muted-foreground">{count === undefined ? "Not reported" : `${count.toLocaleString()} retries`}. {explanation}</dd>
          </div>)}
        </dl>
      </div>
    </Disclosure> : null}
  </>;
}

function formatReasoning(progress: DocumentGenerationProgress): string {
  const requested = progress.requestedReasoningEffort;
  return requested && requested !== progress.reasoningEffort
    ? `${requested} → ${progress.reasoningEffort}`
    : progress.reasoningEffort;
}
function formatOptionalBytes(value: number | null): string {
  return value === null ? "Unknown" : formatBytes(value);
}
function formatDiagnosticStage(stage: string | null): string {
  if (!stage) return "Unknown";
  return stage.replaceAll("_", " ").replace(/\b\w/gu, (letter) => letter.toUpperCase());
}

function formatProviderActivity(progress: DocumentGenerationProgress | null, now: number): string {
  if (!progress) return "Preparing";
  const providerAge = activityAgeSeconds(progress.lastProviderActivityAt, now);
  const modelAge = activityAgeSeconds(progress.lastModelActivityAt, now);
  switch (progress.providerActivity) {
    case "streaming":
      if (modelAge === null) return `Responsive, waiting${activityAgeSuffix(providerAge)}`;
      if (modelAge < 2) return "Receiving output · now";
      if (providerAge !== null && providerAge < modelAge) return `Responsive · output ${formatActivityAge(modelAge)}`;
      return `Receiving output · ${formatActivityAge(modelAge)}`;
    case "responsive": return `Responsive, waiting${activityAgeSuffix(providerAge)}`;
    case "connecting": return "Connecting";
    default: return "Preparing";
  }
}
function activityAgeSeconds(timestamp: number | null, now: number): number | null {
  return timestamp === null ? null : Math.max(0, Math.floor((now - timestamp) / 1_000));
}
function activityAgeSuffix(ageSeconds: number | null): string { return ageSeconds === null ? "" : ` · ${formatActivityAge(ageSeconds)}`; }
function formatActivityAge(ageSeconds: number): string {
  if (ageSeconds < 2) return "now";
  if (ageSeconds < 60) return `${ageSeconds}s ago`;
  const minutes = Math.floor(ageSeconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m ago`;
}
export function generationStage(document: DocumentSummary, progress: DocumentGenerationProgress | null): { title: string; detail: string } {
  if (document.sourceUnitCount === 0) return ingestionStage(progress?.phase) ?? {
    title: "Opening your document", detail: `Reading the ${formatDocumentType(document.sourceFormat)} source and building semantic chunks.`,
  };
  if (isPdf(document) && document.renderedPageCount < document.sourceUnitCount) return {
    title: `Preparing pages · ${document.renderedPageCount} of ${document.sourceUnitCount}`,
    detail: "Preparing local page derivatives for document analysis.",
  };
  if (isPdf(document) && document.extractionMode === "ocr_fallback" && document.ocrPageCount > document.ocrCompletedPageCount) return {
    title: `Reading scanned pages · ${document.ocrCompletedPageCount} of ${document.ocrPageCount}`,
    detail: "Recovering text from pages whose native PDF text layer was unreliable.",
  };
  if (progress?.phase === "understanding material") return {
    title: "Reading your document", detail: readingDetail(document, progress),
  };
  if (progress?.phase === "indexing saved material") return {
    title: "Indexing what it learned",
    detail: progress.totalWindows > 0 ? `${progress.completedWindows} of ${progress.totalWindows}` : "Making saved knowledge searchable.",
  };
  if (progress?.phase === "organizing knowledge") return {
    title: "Putting what it learned in order", detail: "Building the learning outline from saved interpretations, not rereading the raw document.",
  };
  if (document.outlineWindowCount > 0 && document.outlineCompletedWindowCount < document.outlineWindowCount) return {
    title: "Planning the study material", detail: activeWindowDetail(progress, document.outlineWindowCount, document.sourceUnitKind) ?? "Analyzing bounded source windows for document structure.",
  };
  if (document.generationWindowCount > 0 && document.generationCompletedWindowCount < document.generationWindowCount) return {
    title: "Writing flashcards", detail: activeWindowDetail(progress, document.generationWindowCount, document.sourceUnitKind) ?? "Generating study cards from the current bounded source window.",
  };
  if (progress?.phase === "curating") return {
    title: "Checking the flashcards", detail: "Removing duplicates and low-value candidates before committing the deck.",
  };
  return { title: "Finishing up", detail: "Completing the current bounded task and preparing durable study content." };
}
/** Where reading has reached, in the document's own units rather than internal pieces. */
export function readingDetail(document: DocumentSummary, progress: DocumentGenerationProgress): string {
  const total = document.sourceUnitCount;
  const start = progress.currentPageStart;
  if (start === null) return `${total.toLocaleString()} ${sourceUnitPluralLabel(document.sourceUnitKind)}`;
  const end = Math.max(start, progress.currentPageEnd ?? start);
  if (end === start) return `${sourceUnitSingularHeading(document.sourceUnitKind)} ${start} of ${total}`;
  return `${sourceUnitHeading(document.sourceUnitKind)} ${start}–${end} of ${total}`;
}
function ingestionStage(phase: string | undefined): { title: string; detail: string } | null {
  switch (phase) {
    case "document_detecting": return { title: "Checking the file", detail: "Validating the managed source before local extraction begins." };
    case "document_extracting": return { title: "Opening your document", detail: "Extracting text, tables, slides, sheets, and embedded images locally." };
    case "document_mapping": return { title: "Finding headings and sections", detail: "Converting extracted headings and content into oStudy source blocks." };
    case "document_saving_assets": return { title: "Saving the pictures", detail: "Retaining bounded source images as immutable local evidence." };
    case "document_unitizing": return { title: "Splitting it into sections", detail: "Creating deterministic source sections for study generation." };
    case "document_chunking": return { title: "Splitting it into passages", detail: "Preparing bounded, source-linked chunks for document analysis." };
    case "document_validating": return { title: "Double-checking what was read", detail: "Checking canonical source records before they are committed." };
    default: return null;
  }
}
export function activeWindowDetail(progress: DocumentGenerationProgress | null, totalWindows: number, sourceUnitKind: string): string | null {
  const start = progress?.currentWindow;
  if (start === null || start === undefined) return null;
  const end = progress?.currentWindowEnd ?? start;
  const sourceUnits = progress?.currentPageStart !== null && progress?.currentPageStart !== undefined
    ? ` · ${sourceUnitPluralLabel(sourceUnitKind)} ${progress.currentPageStart}–${progress.currentPageEnd ?? progress.currentPageStart}` : "";
  if (end > start) return `Active windows ${start}–${end} of ${totalWindows}${sourceUnits}.`;
  return `Window ${start} of ${totalWindows}${sourceUnits}.`;
}
export function generationPercent(document: DocumentSummary, progress: DocumentGenerationProgress | null): number | null {
  if (isPdf(document) && document.sourceUnitCount > 0 && document.renderedPageCount < document.sourceUnitCount) {
    return Math.round((document.renderedPageCount / document.sourceUnitCount) * 20);
  }
  // Reading follows page preparation on the same bar: 20% to 95% as passages are read.
  const reading = readingFraction(progress);
  if (reading !== null) return Math.min(95, Math.round(20 + reading * 75));
  if (progress?.phase === "understanding material" || progress?.phase === "organizing knowledge") return null;
  const totalWindows = document.outlineWindowCount + document.generationWindowCount;
  if (totalWindows > 0) {
    const completed = document.outlineCompletedWindowCount + document.generationCompletedWindowCount;
    return Math.min(98, Math.round(20 + (completed / totalWindows) * 75));
  }
  if (progress?.phase === "curating") return 97;
  return null;
}
export function statusLabel(document: DocumentSummary): string {
  if (document.safeErrorCode === "document_generation_canceled" || document.safeErrorCode === "ai_request_canceled") return "Stopped";
  switch (document.lifecycleState) {
    case "processing": return "Processing";
    case "ready": return document.generatedDeckId ? "Deck created" : "Ready";
    case "failed": return "Needs attention";
  }
}
/** The bar's words: the step in plain language, how far it has got and roughly how long is left. */
export function processingPhaseLabel(
  document: DocumentSummary, canceling: boolean, progress: DocumentGenerationProgress | null, now: number = Date.now(),
): string {
  const title = processingStepLabel(document, canceling, progress);
  if (canceling || document.generatedDeckId !== null) return title;
  const reading = readingFraction(progress);
  if (reading === null || progress === null || readingStageHidden(document)) return title;
  const left = timeLeft(reading, progress.phaseStartedAt, now);
  return [title, `${Math.floor(reading * 100)}% read`, left].filter(Boolean).join(" · ");
}
/** The step alone, for announcements that should not repeat on every passage read. */
export function processingStepLabel(document: DocumentSummary, canceling: boolean, progress: DocumentGenerationProgress | null): string {
  if (canceling) return "Stopping";
  if (document.generatedDeckId !== null) return "Generating additional cards";
  return generationStage(document, progress).title;
}
/** Share of the document's passages read so far, while the reading step runs. */
function readingFraction(progress: DocumentGenerationProgress | null): number | null {
  if (progress?.phase !== "understanding material" || progress.totalWindows <= 0) return null;
  return Math.min(1, Math.max(0, progress.completedWindows / progress.totalWindows));
}
/** Page rendering and OCR show their own step even while a reading report is current. */
function readingStageHidden(document: DocumentSummary): boolean {
  return document.sourceUnitCount === 0
    || (isPdf(document) && document.renderedPageCount < document.sourceUnitCount)
    || (isPdf(document) && document.extractionMode === "ocr_fallback" && document.ocrPageCount > document.ocrCompletedPageCount);
}
/** A rough estimate from the pace so far, once there is enough of it to go on. */
export function timeLeft(fraction: number, startedAt: number, now: number): string | null {
  const elapsed = now - startedAt;
  if (fraction < 0.05 || fraction >= 1 || elapsed < 30_000) return null;
  const minutes = Math.round((elapsed * (1 - fraction)) / fraction / 60_000);
  if (minutes < 1) return "less than a minute left";
  if (minutes < 60) return `about ${minutes} min left`;
  const hours = Math.floor(minutes / 60);
  return `about ${hours} hr ${minutes % 60} min left`;
}
function isPdf(document: DocumentSummary): boolean { return document.sourceFormat === "pdf"; }
export function formatDocumentType(sourceFormat: string): string {
  switch (sourceFormat) {
    case "pdf": return "PDF";
    case "doc": case "docx": return "Word";
    case "ppt": case "pptx": return "PowerPoint";
    case "odt": return "OpenDocument text";
    case "odp": return "OpenDocument presentation";
    case "rtf": return "RTF";
    case "epub": return "EPUB";
    case "excel": case "xls": case "xlsx": return "Excel";
    case "ods": return "OpenDocument spreadsheet";
    case "csv": return "CSV";
    default: return sourceFormat || "Document";
  }
}
export function sourceUnitHeading(sourceUnitKind: string): string {
  const plural = sourceUnitPluralLabel(sourceUnitKind);
  return plural.charAt(0).toUpperCase() + plural.slice(1);
}
function sourceUnitSingularHeading(sourceUnitKind: string): string {
  switch (sourceUnitKind) {
    case "time_segment": return "Time segment";
    case "page": case "slide": case "chapter": case "section": case "segment": case "image":
      return sourceUnitHeading(sourceUnitKind).slice(0, -1);
    default: return "Source unit";
  }
}
function sourceUnitPluralLabel(sourceUnitKind: string): string {
  switch (sourceUnitKind) {
    case "page": return "pages";
    case "slide": return "slides";
    case "chapter": return "chapters";
    case "section": return "sections";
    case "segment": return "segments";
    case "image": return "images";
    case "time_segment": return "time segments";
    default: return "source units";
  }
}
export function formatSourceUnitMetric(document: DocumentSummary): string {
  if (document.sourceUnitCount <= 0) return "—";
  if (isPdf(document)) return `${document.renderedPageCount}/${document.sourceUnitCount} rendered`;
  return document.sourceUnitCount.toLocaleString();
}
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  let value = bytes / 1024;
  let unit = "KB";
  for (const nextUnit of ["MB", "GB"]) {
    if (value < 1024) break;
    value /= 1024;
    unit = nextUnit;
  }
  return `${value >= 10 ? value.toFixed(0) : value.toFixed(1)} ${unit}`;
}
export function formatProcessingDuration(value: number | null): string {
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
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}
export function formatApiTokens(document: DocumentSummary, progress: DocumentGenerationProgress | null): string {
  if (document.lifecycleState === "processing") {
    if (!progress || progress.usageReportedRequests === 0) return "Awaiting provider usage";
    const formatted = formatTokenCount(progress.totalTokens);
    return progress.usageUnreportedRequests > 0 ? `${formatted}+ reported` : formatted;
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
export function formatDate(timestamp: number): string { return new Date(timestamp).toLocaleString(); }
