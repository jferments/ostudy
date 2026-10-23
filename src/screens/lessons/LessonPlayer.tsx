import { useEffect, useRef, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import { getLesson, saveLessonProgress, type Lesson, type LessonProgress } from "../../ipc/lessons";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { LectureView } from "./LectureView";

interface LessonPlayerProps {
  lessonId: string;
  subjectName: string;
  /** Where this lesson sits in the course, from 1. */
  lessonNumber: number;
  lessonCount: number;
  onClose: () => void;
  showToast: ShowToast;
}

/**
 * One lesson, taught as its lecture. Showing a step's slide saves where the
 * learner is, and Finish lesson on the last slide marks it done; answering a
 * check never writes a review.
 */
export function LessonPlayer({
  lessonId,
  subjectName,
  lessonNumber,
  lessonCount,
  onClose,
  showToast,
}: LessonPlayerProps) {
  const lane = useRequestLane();
  const [lesson, setLesson] = useState<Lesson | null>(null);
  const [readError, setReadError] = useState<string | null>(null);
  const [openingStep, setOpeningStep] = useState(0);
  const [progress, setProgress] = useState<LessonProgress | null>(null);
  const [finishing, setFinishing] = useState(false);
  // The step last saved, so showing the same slide again writes nothing.
  const savedStep = useRef<number | null>(null);

  useEffect(() => {
    void latestResult(lane, () => getLesson(lessonId)).then(
      (value) => {
        if (value === undefined) return;
        const current =
          value.lesson.progress?.revisionNumber === value.revisionNumber ? value.lesson.progress : null;
        // A finished lesson opens at its start; one in progress where it stopped.
        const opening =
          current && !current.finishedAt ? Math.min(current.currentStep, value.content.steps.length - 1) : 0;
        savedStep.current = current && !current.finishedAt ? current.currentStep : null;
        setLesson(value);
        setProgress(current);
        setOpeningStep(opening);
        setReadError(null);
      },
      (error: unknown) => setReadError(getErrorMessage(error)),
    );
  }, [lane, lessonId]);

  const save = (step: number, finished = false) => {
    if (!lesson) return Promise.resolve();
    savedStep.current = step;
    // Saving is a whole state, so a lost or repeated request does no harm.
    return saveLessonProgress(lessonId, lesson.revisionNumber, step, finished).then(setProgress, (error: unknown) => {
      showToast("error", `Your place in this lesson was not saved: ${getErrorMessage(error)}`);
    });
  };

  if (readError || !lesson) {
    return (
      <div data-testid="lesson-player" className="tw:flex tw:flex-col tw:gap-4">
        <nav aria-label="Breadcrumb">
          <Button size="small" variant="text" className="tw:ps-0" onClick={onClose}>
            <Icon name="chevron-left" size={16} /> {subjectName}
          </Button>
        </nav>
        {readError ? (
          <InlineNotice kind="error">This lesson could not open: {readError}</InlineNotice>
        ) : (
          <LoadingState label="Opening the lesson…" />
        )}
      </div>
    );
  }

  const steps = lesson.content.steps;
  const finish = () => {
    setFinishing(true);
    void save(Math.max(steps.length - 1, 0), true).finally(() => {
      setFinishing(false);
      onClose();
    });
  };

  return (
    <div data-testid="lesson-player">
      <LectureView
        lessonId={lessonId}
        revisionNumber={lesson.revisionNumber}
        lessonTitle={lesson.content.title}
        subjectId={lesson.lesson.subjectId}
        stepKey={steps[openingStep]?.key ?? null}
        steps={steps}
        answered={progress?.checks ?? {}}
        onAnswered={(result) => setProgress(result.progress)}
        onStepShown={(stepKey) => {
          const step = steps.findIndex((item) => item.key === stepKey);
          if (step >= 0 && step !== savedStep.current) void save(step);
        }}
        onFinish={finish}
        finishing={finishing}
        backLabel={subjectName}
        position={`Lesson ${lessonNumber} of ${lessonCount}`}
        onClose={onClose}
        showToast={showToast}
      />
    </div>
  );
}
