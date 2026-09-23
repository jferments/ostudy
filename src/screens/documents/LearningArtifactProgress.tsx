import type { DocumentGenerationProgress, DocumentSummary } from "../../ipc/documents";
import type { DocumentArtifactJob } from "../../ipc/studyGuides";
import { Progress } from "../../ui/primitives/Progress";
import { isActiveArtifactJob } from "../../controllers/useDocumentArtifactProgress";
import { activeWindowDetail, GenerationDiagnostics } from "./DocumentProgress";
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
  if (document.lifecycleState === "failed") {
    return { value, status: "Needs attention", complete: false, failed: true };
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
  const status = job.state === "committing" ? "Saving guide"
    : job.state === "generating" ? `${job.completedUnits} / ${job.totalUnits} sections`
      : "Preparing";
  return { value, status, complete: false, failed: false };
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
export function LearningArtifactProgressPanel({ document, progress, guideJob, guideStarting, readFailed, now }: {
  document: DocumentSummary;
  progress: DocumentGenerationProgress | null;
  guideJob: DocumentArtifactJob | null;
  guideStarting: boolean;
  readFailed: boolean;
  now: number;
}) {
  const activeWindows = progress?.phase === "generating"
    ? activeWindowDetail(progress, document.generationWindowCount, document.sourceUnitKind) : null;
  return <div className={progressStyles.panel} aria-live="polite">
    <ArtifactRow label="Flashcards" progress={flashcardProgress(document, progress)} />
    <ArtifactRow label="Study Guide" progress={guideProgress(guideJob, guideStarting)} />
    {activeWindows ? <div className={progressStyles.detail}>{activeWindows}</div> : null}
    {progress ? <GenerationDiagnostics progress={progress} now={now} /> : null}
    {guideJob?.safeErrorMessage ? <div className="tw:text-sm tw:text-destructive">{guideJob.safeErrorMessage}</div> : null}
    {readFailed ? <div className={progressStyles.detail}>Progress unavailable; showing the last saved status.</div> : null}
  </div>;
}
