import { useMemo } from "react";

import type { LessonSummary } from "../../ipc/lessons";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { Card } from "../../ui/primitives/Card";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ItemTrigger } from "../../ui/primitives/Item";
import { Progress } from "../../ui/primitives/Progress";
import type { SubjectCourse } from "./useSubjectCourse";

interface LessonsSectionProps {
  headingId: string;
  course: SubjectCourse;
  /** Lessons are written from processed sources; without any there is nothing to offer. */
  hasSources: boolean;
  onOpenLesson: (lessonId: string) => void;
}

/** The subject's course: where to pick up, every lesson in order, and writing it. */
export function LessonsSection({ headingId, course, hasSources, onOpenLesson }: LessonsSectionProps) {
  const lessons = useMemo(
    () => course.course?.lessons.filter((lesson) => lesson.written !== null) ?? [],
    [course.course],
  );
  const next = lessons.find((lesson) => !lesson.progress?.finishedAt) ?? null;
  const done = lessons.filter((lesson) => lesson.progress?.finishedAt).length;

  if (!hasSources && lessons.length === 0) return null;
  return (
    <section aria-labelledby={headingId} data-testid="subject-lessons" className="tw:flex tw:flex-col tw:gap-3">
      <div className="tw:flex tw:items-baseline tw:justify-between tw:gap-3">
        <h2 id={headingId} className="tw:m-0 tw:text-body-large tw:font-semibold">
          Lessons
        </h2>
        {lessons.length > 0 ? (
          <span className="tw:text-small tw:text-muted-foreground">
            {done} of {lessons.length} done
          </span>
        ) : null}
      </div>
      {course.readError ? (
        <InlineNotice kind="error">
          Lessons could not load: {course.readError}{" "}
          <Button size="small" variant="text" onClick={course.refresh}>
            Retry
          </Button>
        </InlineNotice>
      ) : null}
      {course.writeError ? (
        <InlineNotice kind="error">
          The lessons were not written: {course.writeError}{" "}
          <Button size="small" variant="text" onClick={course.write}>
            Try again
          </Button>
        </InlineNotice>
      ) : null}
      {course.writing ? (
        <Card as="div" role="status" className="tw:gap-3 tw:p-5 tw:compact:p-4">
          <div className="tw:flex tw:items-center tw:gap-3">
            <Icon name="sparkles" size={20} className="tw:shrink-0 tw:text-accent-foreground" />
            <span className="tw:min-w-0 tw:flex-1 tw:text-body">
              {course.stopping ? "Stopping…" : "Writing lessons…"}
            </span>
            <Button size="small" className="tw:rounded-full" disabled={course.stopping} onClick={course.stop}>
              <Icon name="stop" size={14} /> Stop
            </Button>
          </div>
          <WritingProgress planned={course.progress?.planned ?? null} written={course.progress?.written ?? 0} />
        </Card>
      ) : null}
      {lessons.length > 0 ? (
        <>
          {next ? <ContinueCard lesson={next} number={lessons.indexOf(next) + 1} count={lessons.length} onOpen={onOpenLesson} /> : null}
          <Card as="div" className="tw:gap-0 tw:py-0">
            <ol aria-label="Course" className="tw:m-0 tw:list-none tw:p-0">
              {lessons.map((lesson, index) => (
                <li
                  key={lesson.id}
                  data-testid="subject-lesson"
                  className="tw:border-0 tw:border-b tw:border-solid tw:border-border tw:last:border-b-0"
                >
                  <ItemTrigger
                    onClick={() => onOpenLesson(lesson.id)}
                    className="tw:w-full tw:items-center tw:gap-3 tw:px-5 tw:py-3 tw:text-start tw:outline-none tw:hover:bg-accent/40 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:compact:px-4"
                  >
                    <LessonNumber index={index} finished={Boolean(lesson.progress?.finishedAt)} />
                    <span className="tw:min-w-0 tw:flex-1 tw:font-medium tw:text-body tw:break-words">{lesson.title}</span>
                    <span className="tw:shrink-0 tw:text-label tw:text-muted-foreground">{lessonState(lesson)}</span>
                  </ItemTrigger>
                </li>
              ))}
            </ol>
          </Card>
        </>
      ) : !course.writing && !course.loading && course.course ? (
        <Card as="div" className="tw:flex-row tw:flex-wrap tw:items-center tw:gap-x-4 tw:gap-y-3 tw:p-5 tw:compact:p-4">
          <span
            aria-hidden="true"
            className="tw:grid tw:size-11 tw:shrink-0 tw:place-items-center tw:rounded-md tw:bg-accent tw:text-accent-foreground"
          >
            <Icon name="lightbulb" size={22} />
          </span>
          <p className="tw:m-0 tw:min-w-0 tw:flex-[1_1_12rem] tw:text-body tw:text-muted-foreground">No lessons yet.</p>
          <Button variant="primary" className="tw:rounded-full" onClick={course.write}>
            <Icon name="sparkles" size={16} /> Create lessons
          </Button>
        </Card>
      ) : null}
    </section>
  );
}

function ContinueCard({
  lesson,
  number,
  count,
  onOpen,
}: {
  lesson: LessonSummary;
  number: number;
  count: number;
  onOpen: (lessonId: string) => void;
}) {
  const steps = lesson.written?.stepCount ?? 0;
  const progress = lesson.progress?.revisionNumber === lesson.written?.revisionNumber ? lesson.progress : null;
  const detail = progress
    ? `Lesson ${number} of ${count} · Step ${progress.currentStep + 1} of ${steps}`
    : `Lesson ${number} of ${count} · ${minutes(lesson)}`;
  return (
    <Card
      as="div"
      data-testid="lesson-continue"
      className="tw:flex-row tw:flex-wrap tw:items-center tw:gap-x-4 tw:gap-y-3 tw:bg-accent/40 tw:p-5 tw:compact:p-4"
    >
      <div className="tw:flex tw:min-w-0 tw:flex-[1_1_14rem] tw:flex-col tw:gap-0.5">
        <span className="tw:text-small tw:text-muted-foreground">{detail}</span>
        <span className="tw:text-body-large tw:font-semibold tw:break-words">{lesson.title}</span>
      </div>
      <Button variant="primary" className="tw:rounded-full tw:shadow-glow" onClick={() => onOpen(lesson.id)}>
        <Icon name="book" size={16} /> {progress ? "Continue lesson" : "Start lesson"}
      </Button>
    </Card>
  );
}

function LessonNumber({ index, finished }: { index: number; finished: boolean }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "tw:grid tw:size-8 tw:shrink-0 tw:place-items-center tw:rounded-full tw:text-label tw:font-semibold tw:tabular-nums",
        finished ? "tw:bg-primary tw:text-primary-foreground" : "tw:bg-(--ostudy-color-canvas-inset) tw:text-muted-foreground",
      )}
    >
      {finished ? <Icon name="check" size={14} /> : index + 1}
    </span>
  );
}

function minutes(lesson: LessonSummary): string {
  const value = lesson.written?.estimatedMinutes ?? 0;
  return `${Math.max(1, value)} min`;
}

function lessonState(lesson: LessonSummary): string {
  if (lesson.progress?.finishedAt) return "Done";
  if (lesson.progress && lesson.progress.revisionNumber === lesson.written?.revisionNumber) return "In progress";
  if (lesson.written?.sources === "changed") return `Source changed · ${minutes(lesson)}`;
  return minutes(lesson);
}

/** How far the course has got; until the outline is planned the bar has no end yet. */
function WritingProgress({ planned, written }: { planned: number | null; written: number }) {
  if (planned === null) {
    return <Progress value={null} max={1} label="Course writing progress" text="Planning the course…" />;
  }
  const text = `${written} of ${planned} ${planned === 1 ? "lesson" : "lessons"} written`;
  return <Progress value={written} max={planned} label="Course writing progress" valueText={text} text={text} />;
}
