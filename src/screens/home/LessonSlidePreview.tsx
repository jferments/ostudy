import { useEffect, useState } from "react";

import { getLessonLecture, type Lecture, type LectureSlide, type LessonSummary } from "../../ipc/lessons";
import { Button } from "../../ui/primitives/Button";
import { Panel } from "../../ui/primitives/Panel";
import { readSlideTheme, useSlideDesigns } from "../lessons/useSlideDesigns";
import type { HomeLessons } from "./HomeStudyPanels";
import type { SubjectTile } from "./homeModel";

interface LessonSlidePreviewProps {
  lessons: HomeLessons | null;
  subjects: readonly SubjectTile[];
  onOpenLesson: (subjectId: string, lessonId: string) => void;
}

/**
 * The slide the learner's lesson is on: the most recently studied unfinished
 * lesson, or else the first written lesson not yet started, in subject order.
 * Hidden when there is no such lesson or its slides cannot be read.
 */
export function LessonSlidePreview({ lessons, subjects, onOpenLesson }: LessonSlidePreviewProps) {
  const lesson = lessons ? previewLesson(lessons, subjects) : null;
  const lecture = useLecture(lesson?.id ?? null, lesson?.written?.revisionNumber ?? 0);
  if (!lesson || !lecture || lecture.lessonId !== lesson.id || lecture.revisionNumber !== lesson.written?.revisionNumber) return null;
  const name = subjects.find((tile) => tile.subjectId === lesson.subjectId)?.name ?? "Subject";
  const started = lesson.progress !== null;
  const steps = lesson.written?.stepCount ?? 0;
  const step = Math.min(steps, (lesson.progress?.currentStep ?? 0) + 1);
  const slide = shownSlide(lesson, lecture);
  if (!slide) return null;

  return (
    <Panel
      title={lesson.title}
      description={started ? `${name} · step ${step} of ${steps}` : `${name} · ${steps === 1 ? "1 step" : `${steps} steps`}`}
      data-testid="home-lesson-slide"
      actions={
        <Button
          variant="secondary"
          aria-label={`${started ? "Resume" : "Start"} ${lesson.title}`}
          onClick={() => onOpenLesson(lesson.subjectId, lesson.id)}
        >
          {started ? "Resume" : "Start"}
        </Button>
      }
    >
      <SlidePicture lessonId={lecture.lessonId} revisionNumber={lecture.revisionNumber} slide={slide} />
    </Panel>
  );
}

function SlidePicture({ lessonId, revisionNumber, slide }: { lessonId: string; revisionNumber: number; slide: LectureSlide }) {
  const [theme] = useState(readSlideTheme);
  const designs = useSlideDesigns(lessonId, revisionNumber, theme);
  const image = designs.images.get(slide.key);
  return (
    <figure
      data-testid="home-lesson-slide-picture"
      className={
        image
          ? "tw:m-0 tw:aspect-video tw:w-full tw:overflow-hidden tw:rounded-lg tw:shadow-sm tw:ring-1 tw:ring-border"
          : "tw:m-0 tw:flex tw:aspect-video tw:w-full tw:flex-col tw:gap-3 tw:overflow-hidden tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-6 tw:compact:p-4"
      }
    >
      {image ? <img src={image} alt="" className="tw:block tw:size-full" /> : null}
      {/* The designed slide is a picture of this text, which stays for screen readers. */}
      <div className={image ? "tw:sr-only" : "tw:contents"}>
        <strong className="tw:text-section tw:font-semibold tw:break-words">{slide.title}</strong>
        {slide.points.length > 0 ? (
          <ul className="tw:m-0 tw:flex tw:flex-col tw:gap-1.5 tw:ps-5">
            {slide.points.map((point, position) => (
              <li key={position} className="tw:break-words">
                {point}
              </li>
            ))}
          </ul>
        ) : null}
      </div>
    </figure>
  );
}

export function previewLesson(lessons: HomeLessons, subjects: readonly SubjectTile[]): LessonSummary | null {
  const all = [...lessons.bySubject.values()].flat().filter((lesson) => lesson.written);
  const started = all
    .filter((lesson) => lesson.progress && lesson.progress.finishedAt === null)
    .sort((a, b) => (b.progress?.updatedAt ?? 0) - (a.progress?.updatedAt ?? 0));
  if (started[0]) return started[0];
  for (const tile of subjects) {
    if (tile.subjectId === null) continue;
    const next = lessons.bySubject.get(tile.subjectId)?.find((lesson) => lesson.written && !lesson.progress);
    if (next) return next;
  }
  return null;
}

/** The slide of the step the learner is on; the title slide before they start or after a rewrite. */
function shownSlide(lesson: LessonSummary, lecture: Lecture): LectureSlide | undefined {
  const progress = lesson.progress;
  if (progress && progress.revisionNumber === lecture.revisionNumber) {
    const stepSlides = lecture.slides.filter((slide) => slide.kind === "step");
    const current = stepSlides[progress.currentStep];
    if (current) return current;
  }
  return lecture.slides[0];
}

/** The latest revision's lecture, read again when the lesson is rewritten. */
function useLecture(lessonId: string | null, revisionNumber: number): Lecture | null {
  const [lecture, setLecture] = useState<Lecture | null>(null);
  useEffect(() => {
    if (lessonId === null) return;
    let current = true;
    void getLessonLecture(lessonId, revisionNumber).then(
      (value) => {
        if (current) setLecture(value);
      },
      () => {
        if (current) setLecture(null);
      },
    );
    return () => {
      current = false;
    };
  }, [lessonId, revisionNumber]);
  return lecture;
}
