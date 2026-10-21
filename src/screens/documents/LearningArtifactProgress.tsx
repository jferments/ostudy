import type { DocumentGenerationProgress, DocumentSummary } from "../../ipc/documents";
import type { DocumentArtifactJob } from "../../ipc/studyGuides";
import { Progress } from "../../ui/primitives/Progress";
import { isActiveArtifactJob } from "../../controllers/useDocumentArtifactProgress";
import { activeWindowDetail } from "./DocumentProgress";
import { progressStyles } from "./presentation";

type ArtifactProgress = {
  value: number | null;
  status: string;
  complete: boolean;
  failed: boolean;
};

function ratio(completed: number, total: number): number | null {
  return total > 0 ? Math.round(100 * Math.max(0, Math.min(completed, total)) / total) : null;
}

export function flashcardProgress(
  document: DocumentSummary,
  progress: DocumentGenerationProgress | null,
): ArtifactProgress {
  const value = ratio(document.generationCompletedWindowCount, document.generationWindowCount);
  if (document.lifecycleState !== "processing" && document.generatedDeckId) {
    return { value: 100, status: "Complete", complete: true, failed: false };
  }
  // A successful sibling guide keeps the document available as Ready. It does
  // not erase a terminal flashcard error or turn unfinished windows into cards.
  if (document.lifecycleState !== "processing"
    && (document.lifecycleState === "failed" || document.safeErrorCode)) {
    const stopped = document.safeErrorCode === "document_generation_canceled"
      || document.safeErrorCode === "ai_request_canceled";
    return { value, status: stopped ? "Stopped" : "Needs attention", complete: false, failed: true };
  }
  const status = progress?.phase === "curating" ? "Reviewing cards"
    : document.generationWindowCount > 0
      ? `${document.generationCompletedWindowCount} / ${document.generationWindowCount} windows`
      : "Preparing";
  return { value, status, complete: false, failed: false };
}

export function guideProgress(job: DocumentArtifactJob | null, starting: boolean): ArtifactProgress {
  if (!job) return { value: null, status: starting ? "Preparing" : "Pending", complete: false, failed: false };
  const value = ratio(job.completedUnits, job.totalUnits);
  if (job.state === "succeeded") return { value: 100, status: "Complete", complete: true, failed: false };
  if (!isActiveArtifactJob(job)) {
    return { value, status: job.state === "canceled" ? "Stopped" : "Needs attention", complete: false, failed: true };
  }
  // Counts finished sections. Once all are written, the chapters and the
  // introduction are still to come before the guide is saved.
  const status = job.state === "committing" ? "Saving guide"
    : job.state !== "generating" ? "Preparing"
      : job.completedUnits >= job.totalUnits ? "Organizing the guide"
        : `${job.completedUnits} / ${job.totalUnits} sections completed`;
  return { value, status, complete: false, failed: false };
}

export function guideUsageDetail(job: DocumentArtifactJob): string {
  if (job.apiTokenCount === null) {
    return isActiveArtifactJob(job)
      ? "Study Guide usage · Awaiting provider usage"
      : "Study Guide usage · Provider usage not reported";
  }
  const tokens = job.apiTokenCount.toLocaleString();
  if (isActiveArtifactJob(job)) return `Study Guide usage · ${tokens} reported tokens so far`;
  return job.apiTokenUsageComplete
    ? `Study Guide usage · ${tokens} tokens`
    : `Study Guide usage · ${tokens}+ reported tokens`;
}

/** Flashcards and the study guide as one row bar while both are on track; null once either needs attention. */
export function combinedArtifactProgress(
  document: DocumentSummary,
  progress: DocumentGenerationProgress | null,
  guideJob: DocumentArtifactJob | null,
  guideStarting: boolean,
): { label: string; value: number | null } | null {
  const cards = flashcardProgress(document, progress);
  const guide = guideProgress(guideJob, guideStarting);
  if (cards.failed || guide.failed || guideJob?.safeErrorMessage) return null;
  // Only what is actually being made: cards while the document is processing, a guide once one is asked for.
  const active = [
    document.lifecycleState === "processing" && !cards.complete ? { name: "flashcards", progress: cards } : null,
    (guideJob !== null || guideStarting) && !guide.complete ? { name: "a study guide", progress: guide } : null,
  ].filter((item): item is { name: string; progress: ArtifactProgress } => item !== null);
  const making = active.map((item) => item.name);
  const values = active.map((item) => item.progress.value);
  const value = values.length > 0 && values.every((item) => item !== null)
    ? Math.round((values as number[]).reduce((sum, item) => sum + item, 0) / values.length) : null;
  return { label: making.length === 0 ? "Saving" : `Making ${making.join(" and ")}`, value };
}

function ArtifactRow({ label, progress }: { label: string; progress: ArtifactProgress }) {
  return <div className="tw:grid tw:min-w-0 tw:gap-2">
    <div className={progressStyles.heading}>
      <span>{label}</span>
      <span className={progress.complete ? "tw:text-success" : progress.failed ? "tw:text-destructive" : undefined}>
        {progress.complete ? <span aria-hidden="true">✓ </span> : null}{progress.status}
      </span>
    </div>
    <Progress value={progress.value} max={100} label={`${label} generation progress`} />
  </div>;
}

/** Counts come from committed native windows/sections, never an elapsed-time guess. */
export function LearningArtifactProgressPanel({ document, progress, guideJob, guideStarting, readFailed }: {
  document: DocumentSummary;
  progress: DocumentGenerationProgress | null;
  guideJob: DocumentArtifactJob | null;
  guideStarting: boolean;
  readFailed: boolean;
}) {
  const activeWindows = progress?.phase === "generating"
    ? activeWindowDetail(progress, document.generationWindowCount, document.sourceUnitKind) : null;
  return <div className={progressStyles.panel} aria-live="polite">
    <ArtifactRow label="Flashcards" progress={flashcardProgress(document, progress)} />
    <ArtifactRow label="Study Guide" progress={guideProgress(guideJob, guideStarting)} />
    {guideJob ? <div className={progressStyles.detail}>{guideUsageDetail(guideJob)}</div> : null}
    {activeWindows ? <div className={progressStyles.detail}>{activeWindows}</div> : null}
    {guideJob?.safeErrorMessage ? <div className="tw:text-small tw:text-destructive">{guideJob.safeErrorMessage}</div> : null}
    {readFailed ? <div className={progressStyles.detail}>Progress unavailable; showing the last saved status.</div> : null}
  </div>;
}
