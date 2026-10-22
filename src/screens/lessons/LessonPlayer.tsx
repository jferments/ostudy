import { useEffect, useId, useRef, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import { getLesson, saveLessonProgress, type Lesson, type LessonProgress } from "../../ipc/lessons";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ItemTrigger } from "../../ui/primitives/Item";
import { LectureView } from "./LectureView";
import { LessonBlocks } from "./LessonBlocks";
import { StepCheck } from "./StepCheck";
import { StepNarration } from "./StepNarration";

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
 * One lesson, a step at a time: the step's blocks, then its check. Wide
 * screens add the step outline beside it. Moving between steps saves where
 * the learner is; answering a check never writes a review.
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
  const [step, setStep] = useState(0);
  const [progress, setProgress] = useState<LessonProgress | null>(null);
  const [finishing, setFinishing] = useState(false);
  const [lecture, setLecture] = useState(false);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const titleId = useId();

  useEffect(() => {
    void latestResult(lane, () => getLesson(lessonId)).then(
      (value) => {
        if (value === undefined) return;
        const current =
          value.lesson.progress?.revisionNumber === value.revisionNumber ? value.lesson.progress : null;
        setLesson(value);
        setProgress(current);
        // A finished lesson opens at its start; one in progress where it stopped.
        setStep(current && !current.finishedAt ? Math.min(current.currentStep, value.content.steps.length - 1) : 0);
        setReadError(null);
      },
      (error: unknown) => setReadError(getErrorMessage(error)),
    );
  }, [lane, lessonId]);

  const record = (saved: LessonProgress) => setProgress(saved);

  const goTo = (next: number, finished = false) => {
    if (!lesson) return;
    if (!finished) {
      setStep(next);
      headingRef.current?.focus();
    }
    // Saving is a whole state, so a lost or repeated request does no harm.
    return saveLessonProgress(lessonId, lesson.revisionNumber, next, finished).then(record, (error: unknown) => {
      showToast("error", `Your place in this lesson was not saved: ${getErrorMessage(error)}`);
    });
  };

  if (readError) {
    return (
      <PlayerFrame subjectName={subjectName} onClose={onClose}>
        <InlineNotice kind="error">This lesson could not open: {readError}</InlineNotice>
      </PlayerFrame>
    );
  }
  if (!lesson) {
    return (
      <PlayerFrame subjectName={subjectName} onClose={onClose}>
        <LoadingState label="Opening the lesson…" />
      </PlayerFrame>
    );
  }

  const steps = lesson.content.steps;
  const current = steps[step];
  if (!current) return null;
  if (lecture) {
    return (
      <LectureView
        lessonId={lessonId}
        revisionNumber={lesson.revisionNumber}
        lessonTitle={lesson.content.title}
        subjectId={lesson.lesson.subjectId}
        stepKey={current.key}
        steps={steps}
        answered={progress?.checks ?? {}}
        onAnswered={(result) => record(result.progress)}
        onClose={() => setLecture(false)}
        showToast={showToast}
      />
    );
  }
  const last = step === steps.length - 1;
  const finish = () => {
    setFinishing(true);
    void goTo(step, true)?.finally(() => {
      setFinishing(false);
      onClose();
    });
  };

  return (
    <PlayerFrame subjectName={subjectName} onClose={onClose}>
      <div className="tw:grid tw:items-start tw:gap-8 tw:compact:gap-5 tw:wide:grid-cols-[17rem_minmax(0,1fr)] tw:medium:grid-cols-[15rem_minmax(0,1fr)]">
        <nav
          aria-label="Steps"
          className="tw:hidden tw:rounded-lg tw:bg-(--ostudy-color-canvas-inset) tw:p-3 tw:wide:block tw:medium:block"
        >
          <p className="tw:m-0 tw:px-2 tw:pb-1 tw:text-small tw:text-muted-foreground">
            Lesson {lessonNumber} of {lessonCount}
          </p>
          <p className="tw:m-0 tw:px-2 tw:pb-3 tw:font-semibold tw:text-body tw:break-words">{lesson.content.title}</p>
          <ol className="tw:m-0 tw:flex tw:list-none tw:flex-col tw:gap-0.5 tw:p-0">
            {steps.map((item, index) => (
              <li key={item.key}>
                <ItemTrigger
                  aria-current={index === step ? "step" : undefined}
                  onClick={() => void goTo(index)}
                  className={cn(
                    "tw:w-full tw:items-center tw:gap-2.5 tw:rounded-md tw:px-2 tw:py-2 tw:text-start tw:text-small tw:outline-none tw:hover:bg-accent/40 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50",
                    index === step && "tw:bg-card tw:font-semibold tw:shadow-sm",
                  )}
                >
                  <StepMark index={index} done={index < (progress?.furthestStep ?? -1) || Boolean(progress?.finishedAt)} />
                  <span className="tw:min-w-0 tw:flex-1 tw:break-words">{item.title}</span>
                </ItemTrigger>
              </li>
            ))}
          </ol>
        </nav>
        <article aria-labelledby={titleId} className="tw:flex tw:min-w-0 tw:flex-col tw:gap-6 tw:compact:gap-5">
          <header className="tw:flex tw:flex-col tw:gap-1">
            <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
              Lesson {lessonNumber} · Step {step + 1} of {steps.length}
            </p>
            <h1
              id={titleId}
              ref={headingRef}
              tabIndex={-1}
              className="tw:m-0 tw:text-title tw:leading-tight tw:font-semibold tw:tracking-tight tw:break-words tw:outline-none tw:compact:text-section"
            >
              {current.title}
            </h1>
            <div className="tw:flex tw:flex-wrap tw:items-start tw:gap-2 tw:pt-2">
              {current.narration.trim() ? (
                <StepNarration
                  key={`${lesson.revisionNumber}-${current.key}`}
                  lessonId={lessonId}
                  revisionNumber={lesson.revisionNumber}
                  stepKey={current.key}
                />
              ) : null}
              <Button size="small" variant="text" className="tw:rounded-full" onClick={() => setLecture(true)}>
                <Icon name="view" size={16} /> View as lecture
              </Button>
            </div>
          </header>
          <LessonBlocks blocks={current.blocks} />
          {current.check ? (
            <StepCheck
              key={`${lesson.revisionNumber}-${current.key}`}
              lessonId={lessonId}
              revisionNumber={lesson.revisionNumber}
              step={current}
              answered={progress?.checks[current.key] ?? null}
              onAnswered={(result) => record(result.progress)}
            />
          ) : null}
          <div className="tw:sticky tw:bottom-0 tw:flex tw:items-center tw:justify-between tw:gap-3 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:bg-background tw:py-3">
            <Button className="tw:rounded-full" disabled={step === 0} onClick={() => void goTo(step - 1)}>
              <Icon name="chevron-left" size={16} /> Back
            </Button>
            {last ? (
              <Button variant="primary" className="tw:rounded-full" disabled={finishing} onClick={finish}>
                <Icon name="check" size={16} /> Finish lesson
              </Button>
            ) : (
              <Button variant="primary" className="tw:rounded-full" onClick={() => void goTo(step + 1)}>
                Next <Icon name="chevron-right" size={16} />
              </Button>
            )}
          </div>
        </article>
      </div>
    </PlayerFrame>
  );
}

function PlayerFrame({
  subjectName,
  onClose,
  children,
}: {
  subjectName: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  return (
    <div data-testid="lesson-player" className="tw:flex tw:flex-col tw:gap-4">
      <nav aria-label="Breadcrumb">
        <Button size="small" variant="text" className="tw:ps-0" onClick={onClose}>
          <Icon name="chevron-left" size={16} /> {subjectName}
        </Button>
      </nav>
      {children}
    </div>
  );
}

function StepMark({ index, done }: { index: number; done: boolean }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "tw:grid tw:size-6 tw:shrink-0 tw:place-items-center tw:rounded-full tw:text-caption tw:font-semibold tw:tabular-nums",
        done ? "tw:bg-primary tw:text-primary-foreground" : "tw:bg-card tw:text-muted-foreground",
      )}
    >
      {done ? <Icon name="check" size={12} /> : index + 1}
    </span>
  );
}
