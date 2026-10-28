import { useCourseRun } from "../lessons/useSubjectCourse";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { Card } from "../../ui/primitives/Card";
import { Progress } from "../../ui/primitives/Progress";
import { useWaitingStudySet } from "./pendingStudySets";
import { useStudyTargetBundleRun } from "./useStudyTargetBundleController";

/** One kind of study material, as a subject's tab shows it while it is being made. */
export type MaterialKind = "lessons" | "podcast" | "studyGuide";

export interface MaterialProgress {
  title: string;
  /** What is happening now. */
  detail: string;
  /** Done and total; indeterminate when null. */
  value: number | null;
  max: number;
}

const TITLES: Record<MaterialKind, string> = {
  lessons: "Lessons being made",
  podcast: "Podcast being made",
  studyGuide: "Study guide being made",
};

/**
 * How far the subject's material of this kind is, while a New study set or a Make
 * has asked for it and it is not done: waiting for its documents to be read, then
 * being written. Null when none is on its way.
 */
export function useMaterialProgress(subjectId: string, kind: MaterialKind): MaterialProgress | null {
  const waiting = useWaitingStudySet(subjectId);
  const course = useCourseRun(subjectId);
  const bundle = useStudyTargetBundleRun({ kind: "collection", id: subjectId });
  const title = TITLES[kind];
  const asked = waiting !== null && (kind === "lessons" ? waiting.lessons : kind === "podcast" ? waiting.lessons && waiting.podcast === true : waiting.studyGuide);
  if (asked) {
    const count = waiting.documentIds.length;
    return { title, detail: `Reading ${count === 1 ? "the document" : `${count} documents`} first`, value: null, max: 100 };
  }
  if (kind === "studyGuide") {
    if (!bundle) return null;
    const guide = bundle === "starting" ? null : bundle.studyGuide;
    if (guide && guide.state !== "pending" && guide.state !== "running" && guide.state !== "committing") return null;
    if (guide && (guide.state === "running" || guide.state === "committing") && guide.totalUnits > 0) {
      return { title, detail: `Writing: ${guide.completedUnits} of ${guide.totalUnits} parts`, value: guide.completedUnits, max: guide.totalUnits };
    }
    return { title, detail: guide?.state === "pending" ? "Waiting for the flashcards" : "Writing…", value: null, max: 100 };
  }
  if (!course) return null;
  const planned = course.progress?.planned ?? null;
  const written = course.progress?.written ?? 0;
  if (kind === "podcast") {
    if (course.writingPodcast) return { title, detail: "Writing the podcast", value: null, max: 100 };
    if (!course.podcastPending) return null;
    if (course.audio) return { title, detail: "Waiting for the lessons", value: null, max: 100 };
    return {
      title,
      detail: planned === null ? "Waiting for the lessons to be planned" : `Waiting for the lessons: ${written} of ${planned} written`,
      value: planned === null ? null : written,
      max: planned ?? 100,
    };
  }
  if (course.audio) {
    return { title, detail: `Making lecture audio: ${course.audio.done} of ${course.audio.total} lessons`, value: course.audio.done, max: course.audio.total };
  }
  return {
    title,
    detail: planned === null ? "Planning the course…" : `Writing: ${written} of ${planned} lessons`,
    value: planned === null ? null : written,
    max: planned ?? 100,
  };
}

const ICONS: Record<MaterialKind, IconName> = { lessons: "monitor", podcast: "headphones", studyGuide: "study-guide" };

/** A tile saying a material is on its way, with its progress. */
export function MaterialBeingMade({ kind, progress }: { kind: MaterialKind; progress: MaterialProgress }) {
  return (
    <Card as="div" role="status" aria-live="polite" data-testid="material-being-made" className="tw:gap-3 tw:p-5 tw:compact:p-4">
      <div className="tw:flex tw:items-center tw:gap-3">
        <span aria-hidden="true" className="tw:grid tw:size-10 tw:shrink-0 tw:place-items-center tw:rounded-md tw:bg-accent tw:text-accent-foreground">
          <Icon name={ICONS[kind]} size={20} />
        </span>
        <span className="tw:grid tw:min-w-0 tw:flex-1 tw:gap-0.5">
          <strong className="tw:text-body tw:font-semibold">{progress.title}…</strong>
          <span className="tw:text-small tw:text-muted-foreground">{progress.detail}</span>
        </span>
      </div>
      <Progress value={progress.value} max={progress.max} label={`${progress.title} progress`} valueText={progress.detail} />
    </Card>
  );
}

/** The tile for the subject's material of this kind, while it is on its way. */
export function SubjectMaterialBeingMade({ subjectId, kind }: { subjectId: string; kind: MaterialKind }) {
  const progress = useMaterialProgress(subjectId, kind);
  return progress ? <MaterialBeingMade kind={kind} progress={progress} /> : null;
}
