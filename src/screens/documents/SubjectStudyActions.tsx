import type { StudyTargetBundleStatus } from "../../ipc/studyMaterials";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Progress } from "../../ui/primitives/Progress";
import { useStudyTargetBundleController } from "./useStudyTargetBundleController";

interface SubjectStudyActionsProps {
  subjectId: string;
  /** Changes whenever the subject's documents or reading ranges do, so the status rereads. */
  refreshKey: string | number;
  hasDocuments: boolean;
  refreshDecks?: () => Promise<unknown>;
  /** Studies the decks the subject's materials made. */
  onStudy?: (subjectId: string) => void;
  onOpenGuide?: (subjectId: string, title: string) => void;
}

/**
 * The subject's study materials, made from all of its documents together:
 * Study and Read guide once they exist; otherwise the one action that makes
 * them, and progress with Stop while they are being made.
 */
export function SubjectStudyActions({
  subjectId,
  refreshKey,
  hasDocuments,
  refreshDecks,
  onStudy,
  onOpenGuide,
}: SubjectStudyActionsProps) {
  const bundle = useStudyTargetBundleController({
    target: { kind: "collection", id: subjectId },
    changed: refreshDecks,
    refreshKey,
  });
  const status = bundle.status;
  if (!hasDocuments && !status) return null;
  if (bundle.busy) {
    return (
      <div className="tw:flex tw:min-w-60 tw:flex-1 tw:items-center tw:gap-3" data-testid="subject-study-progress">
        <BundleProgress status={status} />
        <Button
          size="small"
          variant="dangerSecondary"
          disabled={bundle.canceling}
          aria-label={bundle.canceling ? "Stopping study materials" : "Stop making study materials"}
          onClick={() => void bundle.stop()}
        >
          <Icon name="stop" size={16} />
          {bundle.canceling ? "Stopping…" : "Stop"}
        </Button>
      </div>
    );
  }
  const study = status?.flashcards.state === "succeeded" && onStudy ? () => onStudy(subjectId) : undefined;
  const guide = status?.guideId && onOpenGuide ? () => onOpenGuide(subjectId, status.title) : undefined;
  const update = updateLabel(status);
  const needsUpdate = status !== null && (status.sourceChanged || update !== "Update study materials");
  return (
    <>
      {study ? <Button onClick={study}>Study</Button> : null}
      {guide ? <Button onClick={guide}>Read guide</Button> : null}
      {!status ? (
        <Button data-testid="subject-make-materials" disabled={bundle.loading} onClick={() => void bundle.run()}>
          <Icon name="sparkles" size={16} /> Make study materials
        </Button>
      ) : needsUpdate && update ? (
        <Button disabled={bundle.loading} onClick={() => void bundle.run()}>{update}</Button>
      ) : null}
      {bundle.error ? (
        <InlineNotice kind="warning" className="tw:basis-full">{bundle.error}</InlineNotice>
      ) : null}
    </>
  );
}

function BundleProgress({ status }: { status: StudyTargetBundleStatus | null }) {
  const active = bundleProgress(status);
  return (
    <div className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-1.5" role="status" aria-live="polite">
      <div className="tw:flex tw:items-center tw:justify-between tw:gap-3 tw:text-small">
        <span className="tw:truncate">{active.title}</span>
        <span className="tw:shrink-0 tw:text-muted-foreground">{active.label}</span>
      </div>
      <Progress value={active.value} max={active.max} label="Study materials progress" valueText={active.label} />
    </div>
  );
}

function bundleProgress(status: StudyTargetBundleStatus | null): {
  title: string;
  label: string;
  value: number | null;
  max: number;
} {
  if (!status) return { title: "Preparing study materials", label: "Starting…", value: null, max: 100 };
  const guide = status.studyGuide;
  if (guide.state === "running" || guide.state === "committing") {
    const determinate = guide.totalUnits > 0;
    return {
      title: "Writing the study guide",
      label: determinate ? `${guide.completedUnits} / ${guide.totalUnits}` : "Working…",
      value: determinate ? guide.completedUnits : null,
      max: determinate ? guide.totalUnits : 100,
    };
  }
  const cards = status.flashcards;
  if (cards.state === "running" || cards.state === "committing") {
    const determinate = cards.totalUnits > 0;
    return {
      title: "Making flashcards",
      label: determinate ? `${cards.completedUnits} / ${cards.totalUnits}` : "Working…",
      value: determinate ? cards.completedUnits : null,
      max: determinate ? cards.totalUnits : 100,
    };
  }
  if (cards.state === "succeeded" && guide.state === "pending") {
    return { title: "Preparing the study guide", label: "Working…", value: null, max: 100 };
  }
  return { title: "Preparing study materials", label: "Working…", value: null, max: 100 };
}

function updateLabel(status: StudyTargetBundleStatus | null): string | null {
  if (!status) return null;
  if (status.flashcards.state === "suppressed" || status.studyGuide.state === "suppressed") return "Recreate study materials";
  if (["failed", "interrupted", "canceled"].includes(status.latestBatchState ?? "")
    || status.flashcards.state === "failed" || status.studyGuide.state === "failed") return "Retry study materials";
  return "Update study materials";
}
