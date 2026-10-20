import { useState } from "react";

import type { StudyTargetBundleStatus, StudyTargetOutputPolicy } from "../../ipc/studyMaterials";
import { Sheet } from "../../ui/overlays/Dialog";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { Checkbox } from "../../ui/primitives/FormControls";
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
 * them, and progress with Stop while they are being made. Make study materials
 * opens a sheet that chooses which of the guide and the flashcards to make.
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
  const [choosing, setChoosing] = useState(false);
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
        <Button data-testid="subject-make-materials" disabled={bundle.loading} onClick={() => setChoosing(true)}>
          <Icon name="sparkles" size={16} /> Make study materials
        </Button>
      ) : needsUpdate && update ? (
        <Button disabled={bundle.loading} onClick={() => void bundle.run()}>{update}</Button>
      ) : null}
      {bundle.error ? (
        <InlineNotice kind="warning" className="tw:basis-full">{bundle.error}</InlineNotice>
      ) : null}
      {choosing ? (
        <MakeMaterialsSheet
          onClose={() => setChoosing(false)}
          onMake={(policy) => {
            setChoosing(false);
            void bundle.run(policy);
          }}
        />
      ) : null}
    </>
  );
}

/** What to make for the subject; at least one output, both by default. */
function MakeMaterialsSheet({ onClose, onMake }: {
  onClose: () => void;
  onMake: (policy: StudyTargetOutputPolicy) => void;
}) {
  const [policy, setPolicy] = useState<StudyTargetOutputPolicy>({ flashcards: true, studyGuide: true });
  const nothing = !policy.flashcards && !policy.studyGuide;
  const options: { key: keyof StudyTargetOutputPolicy; label: string; detail: string }[] = [
    { key: "studyGuide", label: "Study guide", detail: "One guide written from all of the subject's documents." },
    { key: "flashcards", label: "Flashcards", detail: "Each document's own deck, filed in the subject." },
  ];
  return (
    <Sheet
      title="Make study materials"
      width="small"
      onClose={onClose}
      onSubmit={(event) => {
        event.preventDefault();
        if (!nothing) onMake(policy);
      }}
      actions={<>
        <Button variant="secondary" onClick={onClose}>Cancel</Button>
        <Button type="submit" disabled={nothing}>Make</Button>
      </>}
    >
      <fieldset className="tw:m-0 tw:grid tw:gap-2 tw:border-0 tw:p-0">
        <legend className="tw:mb-2 tw:p-0 tw:text-label tw:font-medium">Make</legend>
        {options.map((option) => (
          <label key={option.key} className="tw:flex tw:min-h-11 tw:cursor-pointer tw:items-start tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:px-4 tw:py-3">
            <Checkbox
              checked={policy[option.key]}
              onCheckedChange={(checked) => setPolicy((current) => ({ ...current, [option.key]: checked }))}
            />
            <span className="tw:grid tw:gap-0.5">
              <span className="tw:text-label tw:font-medium">{option.label}</span>
              <span className="tw:text-small tw:text-muted-foreground">{option.detail}</span>
            </span>
          </label>
        ))}
      </fieldset>
    </Sheet>
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
