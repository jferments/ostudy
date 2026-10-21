import { useId, useMemo, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { useHomeSummary } from "../../controllers/HomeSummaryProvider";
import type { LibrarySubjectScope } from "../../navigation/appDestinations";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { Button } from "../../ui/primitives/Button";
import { EmptyState, LoadingState } from "../../ui/primitives/EmptyState";
import { ListRow, ListRowMedia, ListRows } from "../../ui/primitives/ListRow";
import { Panel } from "../../ui/primitives/Panel";
import { useHomeLessons } from "../home/HomeStudyPanels";
import { LessonPlayer } from "../lessons/LessonPlayer";
import { LessonsSection } from "../lessons/LessonsSection";
import { PodcastPlayer } from "../lessons/PodcastPlayer";
import { PodcastsSection } from "../lessons/PodcastsSection";
import { useSubjectCourse } from "../lessons/useSubjectCourse";
import { useLibraryFullPage } from "./LibraryFrame";
import { lessonMeta } from "./LibraryOverview";
import { useLibrarySubjects, type LibrarySubjects } from "./LibrarySubjects";

interface LibraryLessonsProps {
  subject: LibrarySubjectScope;
  /** The lesson playing, if any; it belongs to the chosen subject. */
  openLessonId: string | null;
  onOpenLesson: (subjectId: string, lessonId: string) => void;
  onCloseLesson: () => void;
  showToast: ShowToast;
}

/**
 * The chosen subject's course: where to pick up, every lesson in order, and
 * writing more. Everything lists every subject's lessons; No subject has none,
 * since lessons belong to a subject.
 */
export function LibraryLessons({ subject, openLessonId, onOpenLesson, onCloseLesson, showToast }: LibraryLessonsProps) {
  const subjects = useLibrarySubjects(subject);
  if (subject.kind === "subject") {
    return (
      <SubjectLessons
        key={subject.subjectId}
        subjectId={subject.subjectId}
        subjects={subjects}
        openLessonId={openLessonId}
        onOpenLesson={(lessonId) => onOpenLesson(subject.subjectId, lessonId)}
        onCloseLesson={onCloseLesson}
        showToast={showToast}
      />
    );
  }
  if (subject.kind === "none") return <EmptyState icon="book" title="Lessons belong to a subject" />;
  return <AllLessons subjects={subjects} onOpenLesson={onOpenLesson} />;
}

function SubjectLessons({
  subjectId,
  subjects,
  openLessonId,
  onOpenLesson,
  onCloseLesson,
  showToast,
}: {
  subjectId: string;
  subjects: LibrarySubjects;
  openLessonId: string | null;
  onOpenLesson: (lessonId: string) => void;
  onCloseLesson: () => void;
  showToast: ShowToast;
}) {
  const course = useSubjectCourse(subjectId);
  const headingId = useId();
  const podcastsHeadingId = useId();
  const [openPodcastId, setOpenPodcastId] = useState<string | null>(null);
  const tile = subjects.current;
  const written = course.course?.lessons.filter((lesson) => lesson.written !== null) ?? [];
  const openIndex = written.findIndex((lesson) => lesson.id === openLessonId);
  const playing = openLessonId !== null && tile !== null && openIndex >= 0;
  const listening = openPodcastId !== null && tile !== null && !playing;
  // A playing lesson or podcast is a page of its own; closing it returns to the course.
  useLibraryFullPage(playing || listening);

  if (playing && tile && openLessonId) {
    return (
      <PageScaffold width="workspace" aria-label={`${tile.name} lesson`}>
        <LessonPlayer
          lessonId={openLessonId}
          subjectName={tile.name}
          lessonNumber={openIndex + 1}
          lessonCount={written.length}
          showToast={showToast}
          onClose={() => {
            onCloseLesson();
            course.refresh();
          }}
        />
      </PageScaffold>
    );
  }
  if (listening && tile && openPodcastId) {
    return (
      <PageScaffold width="workspace" aria-label={`${tile.name} podcast`}>
        <PodcastPlayer podcastId={openPodcastId} subjectName={tile.name} onClose={() => setOpenPodcastId(null)} />
      </PageScaffold>
    );
  }
  if (course.loading && !course.course) return <LoadingState label="Loading lessons…" />;
  const hasSources = (tile?.summary.sourceCount ?? 0) > 0;
  if (!hasSources && written.length === 0) return <EmptyState icon="book" title="No lessons yet" />;
  return (
    <div data-testid="library-lessons" className="tw:flex tw:max-w-3xl tw:flex-col tw:gap-8">
      <LessonsSection headingId={headingId} course={course} hasSources={hasSources} onOpenLesson={onOpenLesson} />
      <PodcastsSection
        headingId={podcastsHeadingId}
        subjectId={subjectId}
        lessons={written}
        onOpenPodcast={setOpenPodcastId}
      />
    </div>
  );
}

function AllLessons({
  subjects,
  onOpenLesson,
}: {
  subjects: LibrarySubjects;
  onOpenLesson: (subjectId: string, lessonId: string) => void;
}) {
  const { summary } = useHomeSummary();
  const subjectIds = useMemo(
    () => subjects.subjects.flatMap((tile) => (tile.subjectId === null ? [] : [tile.subjectId])),
    [subjects.subjects],
  );
  const lessons = useHomeLessons(subjectIds, summary?.today.scheduledReviews ?? 0);
  const names = new Map(subjects.subjects.map((tile) => [tile.subjectId, tile.name]));
  if (!lessons) return <LoadingState label="Loading lessons…" />;
  // In each subject's course order, subjects in the learner's order.
  const all = subjectIds.flatMap((id) => (lessons.bySubject.get(id) ?? []).filter((lesson) => lesson.written !== null));
  if (all.length === 0) return <EmptyState icon="book" title="No lessons yet" />;
  return (
    <Panel data-testid="library-lessons" className="tw:max-w-4xl">
      <ListRows>
        {all.map((lesson) => (
          <ListRow
            key={lesson.id}
            leading={<ListRowMedia icon="book" />}
            title={lesson.title}
            meta={`${names.get(lesson.subjectId) ?? "Subject"} · ${lessonMeta(lesson)}`}
            trailing={
              <Button variant="secondary" size="small" aria-label={`Open ${lesson.title}`} onClick={() => onOpenLesson(lesson.subjectId, lesson.id)}>
                {lesson.progress && lesson.progress.finishedAt === null ? "Resume" : "Open"}
              </Button>
            }
          />
        ))}
      </ListRows>
    </Panel>
  );
}
