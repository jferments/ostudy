import { useId, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import type { LibrarySubjectScope } from "../../navigation/appDestinations";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { EmptyState, LoadingState } from "../../ui/primitives/EmptyState";
import { LessonPlayer } from "../lessons/LessonPlayer";
import { LessonsSection } from "../lessons/LessonsSection";
import { PodcastPlayer } from "../lessons/PodcastPlayer";
import { PodcastsSection } from "../lessons/PodcastsSection";
import { useSubjectCourse } from "../lessons/useSubjectCourse";
import { useLibraryFullPage } from "./LibraryFrame";
import { useLibrarySubjects, type LibrarySubjects } from "./LibrarySubjects";

interface LibraryLessonsProps {
  /** The subject's lessons, or its podcasts. */
  page: "lessons" | "podcasts";
  subject: LibrarySubjectScope;
  /** The lesson playing, if any; it belongs to the chosen subject. */
  openLessonId: string | null;
  onOpenLesson: (subjectId: string, lessonId: string) => void;
  onCloseLesson: () => void;
  showToast: ShowToast;
}

/**
 * The chosen subject's course (where to pick up, every lesson in order, and
 * writing more), or its podcasts. No subject has neither, since both belong
 * to a subject.
 */
export function LibraryLessons({ page, subject, openLessonId, onOpenLesson, onCloseLesson, showToast }: LibraryLessonsProps) {
  const subjects = useLibrarySubjects(subject);
  if (subject.kind === "subject") {
    return (
      <SubjectLessons
        key={subject.subjectId}
        page={page}
        subjectId={subject.subjectId}
        subjects={subjects}
        openLessonId={openLessonId}
        onOpenLesson={(lessonId) => onOpenLesson(subject.subjectId, lessonId)}
        onCloseLesson={onCloseLesson}
        showToast={showToast}
      />
    );
  }
  return (
    <EmptyState
      icon={page === "lessons" ? "book" : "headphones"}
      title={page === "lessons" ? "Lessons belong to a subject" : "Podcasts belong to a subject"}
    />
  );
}

function SubjectLessons({
  page,
  subjectId,
  subjects,
  openLessonId,
  onOpenLesson,
  onCloseLesson,
  showToast,
}: {
  page: "lessons" | "podcasts";
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
      <PageScaffold width="full" aria-label={`${tile.name} lesson`}>
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
  if (course.loading && !course.course) {
    return <LoadingState label={page === "lessons" ? "Loading lessons…" : "Loading podcasts…"} />;
  }
  if (page === "podcasts") {
    return (
      <div data-testid="library-podcasts" className="tw:flex tw:max-w-3xl tw:flex-col tw:gap-8">
        <PodcastsSection
          headingId={podcastsHeadingId}
          subjectId={subjectId}
          lessons={written}
          onOpenPodcast={setOpenPodcastId}
        />
      </div>
    );
  }
  const hasSources = (tile?.summary.sourceCount ?? 0) > 0;
  if (!hasSources && written.length === 0) return <EmptyState icon="book" title="No lessons yet" />;
  return (
    <div data-testid="library-lessons" className="tw:flex tw:max-w-3xl tw:flex-col tw:gap-8">
      <LessonsSection headingId={headingId} course={course} hasSources={hasSources} onOpenLesson={onOpenLesson} />
    </div>
  );
}
