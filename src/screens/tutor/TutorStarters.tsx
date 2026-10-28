import { useMemo } from "react";

import { useOptionalHomeSummary } from "../../controllers/HomeSummaryProvider";
import type { StudyScope } from "../../ipc/knowledge";
import type { TutorMode } from "../../ipc/tutor";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { ItemTrigger } from "../../ui/primitives/Item";
import { useHomeLessons } from "../home/HomeStudyPanels";
import { useDocumentPreparations } from "../home/useDocumentPreparations";
import { styles } from "./presentation";
import { useSubjectsPreparing } from "./useSubjectsPreparing";

const MAX_STARTERS = 4;

export interface TutorStarter {
  key: string;
  icon: IconName;
  title: string;
  meta: string;
  scope: StudyScope;
  mode: TutorMode;
  text: string;
  /** The new chat's name. */
  chatTitle: string;
}

/**
 * Ways into a new chat from real study state: the subject with the most cards
 * due, a lesson started and not finished, and subjects with study guides. Each
 * subject appears once, and only once its study materials are done being made.
 * Empty when the library has nothing to start from.
 */
export function useTutorStarters(): TutorStarter[] {
  const home = useOptionalHomeSummary();
  const librarySubjects = home?.library?.subjects;
  const subjects = useMemo(() => librarySubjects ?? [], [librarySubjects]);
  const subjectIds = useMemo(() => subjects.map((subject) => subject.id), [subjects]);
  const lessons = useHomeLessons(subjectIds, 0);
  const documentSubjects = home?.library?.documents;
  const placements = useMemo(() => documentSubjects ?? [], [documentSubjects]);
  const withDocuments = useMemo(
    () => (home?.summary?.subjects ?? []).flatMap((row) => (row.subjectId !== null && row.sourceCount > 0 ? [row.subjectId] : [])),
    [home?.summary?.subjects],
  );
  const preparing = useSubjectsPreparing(placements, useDocumentPreparations(), withDocuments);

  return useMemo(() => {
    const summary = home?.summary;
    if (!summary) return [];
    const names = new Map(subjects.filter((subject) => !preparing.has(subject.id)).map((subject) => [subject.id, subject.name]));
    const used = new Set<string>();
    const starters: TutorStarter[] = [];
    const scopeOf = (subjectId: string): StudyScope => ({ kind: "collection", id: subjectId });

    const due = summary.subjects
      .filter((row) => row.subjectId !== null && row.dueReviews > 0 && names.has(row.subjectId))
      .sort((a, b) => b.dueReviews - a.dueReviews)[0];
    if (due?.subjectId) {
      const name = names.get(due.subjectId) ?? "";
      used.add(due.subjectId);
      starters.push({
        key: `quiz-${due.subjectId}`,
        icon: "help",
        title: `Quiz me on ${name}`,
        meta: due.dueReviews === 1 ? "1 card due" : `${due.dueReviews.toLocaleString()} cards due`,
        scope: scopeOf(due.subjectId),
        mode: "quiz",
        text: "",
        chatTitle: `Quiz: ${name}`,
      });
    }

    const lesson = lessons
      ? [...lessons.bySubject.values()]
        .flat()
        .filter((entry) => entry.written && entry.progress && entry.progress.finishedAt === null)
        .sort((a, b) => (b.progress?.updatedAt ?? 0) - (a.progress?.updatedAt ?? 0))[0]
      : undefined;
    if (lesson && names.has(lesson.subjectId)) {
      used.add(lesson.subjectId);
      starters.push({
        key: `lesson-${lesson.id}`,
        icon: "monitor",
        title: `Talk through ${lesson.title}`,
        meta: names.get(lesson.subjectId) ?? "",
        scope: scopeOf(lesson.subjectId),
        mode: "explain",
        text: `Walk me through the lesson "${lesson.title}": its key ideas, simply, one at a time.`,
        chatTitle: lesson.title,
      });
    }

    for (const row of summary.subjects) {
      if (starters.length >= MAX_STARTERS) break;
      const subjectId = row.subjectId;
      if (subjectId === null || used.has(subjectId) || !names.has(subjectId)) continue;
      if (row.guideCount === 0 && row.sourceCount === 0) continue;
      const name = names.get(subjectId) ?? "";
      used.add(subjectId);
      starters.push({
        key: `ideas-${subjectId}`,
        icon: "book",
        title: `Sum up ${name}`,
        meta: row.guideCount > 0
          ? (row.guideCount === 1 ? "1 study guide" : `${row.guideCount.toLocaleString()} study guides`)
          : (row.sourceCount === 1 ? "1 document" : `${row.sourceCount.toLocaleString()} documents`),
        scope: scopeOf(subjectId),
        mode: "explain",
        text: `What are the key ideas in ${name}? Give me a short summary I can build on.`,
        chatTitle: name,
      });
    }
    return starters.slice(0, MAX_STARTERS);
  }, [home?.summary, subjects, lessons, preparing]);
}

export function TutorStarters({
  starters,
  disabled,
  onStart,
}: {
  starters: TutorStarter[];
  disabled: boolean;
  onStart: (starter: TutorStarter) => void;
}) {
  if (!starters.length) return null;
  return (
    <ul className={styles.starters} aria-label="Start from your study">
      {starters.map((starter) => (
        <li key={starter.key} className="tw:min-w-0">
          <ItemTrigger className={styles.starter} disabled={disabled} onClick={() => onStart(starter)}>
            <span className={styles.starterIcon} aria-hidden="true"><Icon name={starter.icon} size={18} /></span>
            <span className="tw:min-w-0">
              <strong>{starter.title}</strong>
              <small>{starter.meta}</small>
            </span>
          </ItemTrigger>
        </li>
      ))}
    </ul>
  );
}
