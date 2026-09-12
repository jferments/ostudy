import type { DocumentGenerationProgress, DocumentSummary } from "../../ipc/documents";
import { progressStyles } from "./presentation";
import { Progress } from "../../ui/primitives/Progress";

export function GenerationProgressPanel({ document, progress, now }: {
  document: DocumentSummary; progress: DocumentGenerationProgress | null; now: number;
}) {
  const stage = generationStage(document, progress);
  const percent = generationPercent(document, progress);
  const promptTokens = progress?.promptTokens ?? 0;
  const cacheRatio = promptTokens > 0 && progress ? Math.min(100, (progress.cachedPromptTokens / promptTokens) * 100) : null;
  return <div className={progressStyles.panel} aria-live="polite">
    <div className={progressStyles.heading}><span>{stage.title}</span><span>{percent === null ? "Working…" : `${percent}%`}</span></div>
    <Progress value={percent} max={100} label="Document generation progress" />
    <div className={progressStyles.detail}>{stage.detail}</div>
    <dl className={progressStyles.stats}>
      <div><dt>Elapsed</dt><dd>{formatLiveDuration(progress?.startedAt ?? null, now)}</dd></div>
      <div><dt>Requests</dt><dd>{progress ? progress.providerRequestCount.toLocaleString() : "—"}</dd></div>
      <div><dt>Provider</dt><dd>{formatProviderActivity(progress, now)}</dd></div>
      <div><dt>Retries</dt><dd className={progress && progress.retryCount > 0 ? progressStyles.warning : undefined}>{progress ? progress.retryCount.toLocaleString() : "—"}</dd></div>
      <div><dt>Reasoning</dt><dd>{progress ? `${progress.reasoningEffort} · ${formatTokenCount(progress.reasoningTokens)}` : "—"}</dd></div>
      <div><dt>Prompt / output</dt><dd>{progress ? `${formatTokenCount(progress.promptTokens)} / ${formatTokenCount(progress.completionTokens)}` : "—"}</dd></div>
      <div><dt>Prompt cache</dt><dd>{progress && cacheRatio !== null ? `${formatTokenCount(progress.cachedPromptTokens)} (${cacheRatio.toFixed(1)}%)` : "Awaiting usage"}</dd></div>
    </dl>
  </div>;
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
function generationStage(document: DocumentSummary, progress: DocumentGenerationProgress | null): { title: string; detail: string } {
  if (document.sourceUnitCount === 0) return ingestionStage(progress?.phase) ?? {
    title: "Extracting document content", detail: `Reading the ${formatDocumentType(document.sourceFormat)} source and building semantic chunks.`,
  };
  if (isPdf(document) && document.renderedPageCount < document.sourceUnitCount) return {
    title: `Rendering pages ${document.renderedPageCount}/${document.sourceUnitCount}`,
    detail: "Preparing local page derivatives for document analysis.",
  };
  if (isPdf(document) && document.extractionMode === "ocr_fallback" && document.ocrPageCount > document.ocrCompletedPageCount) return {
    title: `Recovering OCR ${document.ocrCompletedPageCount}/${document.ocrPageCount}`,
    detail: "Recovering text from pages whose native PDF text layer was unreliable.",
  };
  if (progress?.phase === "understanding material") return {
    title: "Understanding material",
    detail: `${progress.completedWindows} / ${progress.totalWindows} source chunks prepared${progress.currentPageStart === null ? "" : ` · ${sourceUnitPluralLabel(document.sourceUnitKind)} ${progress.currentPageStart}–${progress.currentPageEnd ?? progress.currentPageStart}`}. Validated knowledge is saved incrementally.`,
  };
  if (progress?.phase === "organizing knowledge") return {
    title: "Organizing shared knowledge", detail: "Building the learning outline from saved interpretations, not rereading the raw document.",
  };
  if (document.outlineWindowCount > 0 && document.outlineCompletedWindowCount < document.outlineWindowCount) return {
    title: "Building outline", detail: activeWindowDetail(progress, document.outlineWindowCount, document.sourceUnitKind) ?? "Analyzing bounded source windows for document structure.",
  };
  if (document.generationWindowCount > 0 && document.generationCompletedWindowCount < document.generationWindowCount) return {
    title: "Generating cards", detail: activeWindowDetail(progress, document.generationWindowCount, document.sourceUnitKind) ?? "Generating study cards from the current bounded source window.",
  };
  if (progress?.phase === "curating") return {
    title: "Curating generated cards", detail: "Removing duplicates and low-value candidates before committing the deck.",
  };
  return { title: "Finalizing document generation", detail: "Completing the current bounded task and preparing durable study content." };
}
function ingestionStage(phase: string | undefined): { title: string; detail: string } | null {
  switch (phase) {
    case "document_detecting": return { title: "Checking document format", detail: "Validating the managed source before local extraction begins." };
    case "document_extracting": return { title: "Reading document structure", detail: "Extracting text, tables, slides, sheets, and embedded images locally." };
    case "document_mapping": return { title: "Mapping document structure", detail: "Converting extracted headings and content into oStudy source blocks." };
    case "document_saving_assets": return { title: "Saving embedded images", detail: "Retaining bounded source images as immutable local evidence." };
    case "document_unitizing": return { title: "Building source sections", detail: "Creating deterministic source sections for study generation." };
    case "document_chunking": return { title: "Building semantic chunks", detail: "Preparing bounded, source-linked chunks for document analysis." };
    case "document_validating": return { title: "Validating extracted document", detail: "Checking canonical source records before they are committed." };
    default: return null;
  }
}
function activeWindowDetail(progress: DocumentGenerationProgress | null, totalWindows: number, sourceUnitKind: string): string | null {
  const start = progress?.currentWindow;
  if (start === null || start === undefined) return null;
  const end = progress?.currentWindowEnd ?? start;
  const sourceUnits = progress?.currentPageStart !== null && progress?.currentPageStart !== undefined
    ? ` · ${sourceUnitPluralLabel(sourceUnitKind)} ${progress.currentPageStart}–${progress.currentPageEnd ?? progress.currentPageStart}` : "";
  if (end > start) return `Active windows ${start}–${end} of ${totalWindows}${sourceUnits}.`;
  return `Window ${start} of ${totalWindows}${sourceUnits}.`;
}
function generationPercent(document: DocumentSummary, progress: DocumentGenerationProgress | null): number | null {
  if (isPdf(document) && document.sourceUnitCount > 0 && document.renderedPageCount < document.sourceUnitCount) {
    return Math.round((document.renderedPageCount / document.sourceUnitCount) * 20);
  }
  // Outline counts do not describe semantic-analysis progress. Show the real chunk count without inventing a percentage.
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
export function processingPhaseLabel(document: DocumentSummary, canceling: boolean, progress: DocumentGenerationProgress | null): string {
  if (canceling) return "Stopping";
  if (document.generatedDeckId !== null) return "Generating additional cards";
  return generationStage(document, progress).title;
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
  const units = ["KB", "MB", "GB"];
  let value = bytes / 1024;
  let unit = units[0];
  for (let index = 1; index < units.length && value >= 1024; index += 1) { value /= 1024; unit = units[index]; }
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
  if (document.lifecycleState === "processing") return progress && progress.totalTokens > 0 ? formatTokenCount(progress.totalTokens) : "Awaiting provider usage";
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
