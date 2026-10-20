import { useEffect, useMemo, useState } from "react";

import { useDocumentJobs } from "../../controllers/DocumentJobsProvider";
import { useHomeSummary } from "../../controllers/HomeSummaryProvider";
import type { DeckSummary } from "../../ipc/decks";
import type { DocumentSummary } from "../../ipc/documents";
import type { LessonSummary } from "../../ipc/lessons";
import { listStudyGuidePage, type StudyGuidePageItem } from "../../ipc/studyGuides";
import type { SubjectLibrary } from "../../ipc/subjects";
import type { LibrarySection, LibrarySubjectScope } from "../../navigation/appDestinations";
import { Button } from "../../ui/primitives/Button";
import { EmptyState } from "../../ui/primitives/EmptyState";
import { ListRow, ListRowMedia, ListRows } from "../../ui/primitives/ListRow";
import { Panel } from "../../ui/primitives/Panel";
import { formatDocumentType } from "../documents/DocumentProgress";
import { countLabel } from "../home/homeModel";
import { useHomeLessons } from "../home/HomeStudyPanels";
import { useLibrarySubjects } from "./LibrarySubjects";

/** The most rows each group shows; its page has the rest. */
const GROUP_ROWS = 3;

interface LibraryOverviewProps {
  subject: LibrarySubjectScope;
  decks: readonly DeckSummary[];
  onSelectView: (view: LibrarySection) => void;
  onOpenLesson: (subjectId: string, lessonId: string) => void;
  onOpenDeck: (deckId: string) => void;
}

/**
 * Everything the chosen subject holds, a few of each kind: lessons, study
 * guides, documents and flashcards, each with a way to its own page.
 * Everything groups every subject's.
 */
export function LibraryOverview({ subject, decks, onSelectView, onOpenLesson, onOpenDeck }: LibraryOverviewProps) {
  const { summary, library } = useHomeSummary();
  const jobs = useDocumentJobs();
  const subjects = useLibrarySubjects(subject);
  const subjectIds = useMemo(
    () =>
      subject.kind === "subject"
        ? [subject.subjectId]
        : subject.kind === "all"
          ? subjects.subjects.flatMap((tile) => (tile.subjectId === null ? [] : [tile.subjectId]))
          : [],
    [subject, subjects.subjects],
  );
  const refreshKey = summary?.today.scheduledReviews ?? 0;
  const lessons = useHomeLessons(subjectIds, refreshKey);
  const guides = useSubjectGuides(subject, refreshKey);
  const names = new Map(subjects.subjects.map((tile) => [tile.subjectId, tile.name]));

  const subjectLessons = useMemo(() => {
    if (!lessons) return [];
    return subjectIds
      .flatMap((id) => lessons.bySubject.get(id) ?? [])
      .filter((lesson) => lesson.written !== null)
      .sort((left, right) => (right.progress?.updatedAt ?? 0) - (left.progress?.updatedAt ?? 0));
  }, [lessons, subjectIds]);
  const documents = useMemo(
    () => (library && jobs ? inSubject(jobs.documents, library.documents, "documentId", subject) : []),
    [jobs, library, subject],
  );
  const subjectDecks = useMemo(
    () =>
      library
        ? inSubject(
            decks.filter((deck) => deck.archivedAt === null),
            library.decks,
            "deckId",
            subject,
          )
        : [],
    [decks, library, subject],
  );
  const showSubject = subject.kind === "all";
  const empty =
    subjects.ready &&
    lessons !== null &&
    guides !== null &&
    subjectLessons.length === 0 &&
    guides.totalCount === 0 &&
    documents.length === 0 &&
    subjectDecks.length === 0;

  if (empty) {
    return <EmptyState icon="library" title={subject.kind === "all" ? "Your library is empty" : "Nothing here yet"} />;
  }

  return (
    <div data-testid="library-overview" className="tw:grid tw:items-start tw:gap-5 tw:wide:grid-cols-2">
      <div className="tw:grid tw:min-w-0 tw:gap-5 tw:empty:hidden">
        {subjectLessons.length > 0 ? (
          <OverviewGroup
            title="Lessons"
            count={subjectLessons.length}
            countNoun="lesson"
            action="All lessons"
            onAction={() => onSelectView("lessons")}
            testId="library-overview-lessons"
          >
            {subjectLessons.slice(0, GROUP_ROWS).map((lesson) => (
              <ListRow
                key={lesson.id}
                leading={<ListRowMedia icon="book" />}
                title={lesson.title}
                meta={withSubject(lessonMeta(lesson), showSubject ? names.get(lesson.subjectId) : undefined)}
                trailing={
                  <Button variant="secondary" size="small" aria-label={`Open ${lesson.title}`} onClick={() => onOpenLesson(lesson.subjectId, lesson.id)}>
                    {lesson.progress && lesson.progress.finishedAt === null ? "Resume" : "Open"}
                  </Button>
                }
              />
            ))}
          </OverviewGroup>
        ) : null}
        {guides && guides.totalCount > 0 ? (
          <OverviewGroup
            title="Study guides"
            count={guides.totalCount}
            countNoun="guide"
            action="All guides"
            onAction={() => onSelectView("study-guides")}
            testId="library-overview-guides"
          >
            {guides.items.map((guide) => (
              <ListRow
                key={guide.guideId}
                leading={<ListRowMedia icon={guide.targetKind === "collection" ? "folder" : "study-guide"} />}
                title={guide.title}
                meta={guideMeta(guide)}
                chevron
                onActivate={() => onSelectView("study-guides")}
              />
            ))}
          </OverviewGroup>
        ) : null}
      </div>
      <div className="tw:grid tw:min-w-0 tw:gap-5 tw:empty:hidden">
        {documents.length > 0 ? (
          <OverviewGroup
            title="Documents"
            count={documents.length}
            countNoun="document"
            action="All documents"
            onAction={() => onSelectView("documents")}
            testId="library-overview-documents"
          >
            {documents.slice(0, GROUP_ROWS).map((document) => (
              <ListRow
                key={document.id}
                leading={<ListRowMedia icon="document" />}
                title={document.title}
                meta={documentMeta(document)}
                chevron
                onActivate={() => onSelectView("documents")}
              />
            ))}
          </OverviewGroup>
        ) : null}
        {subjectDecks.length > 0 ? (
          <OverviewGroup
            title="Flashcards"
            count={subjectDecks.length}
            countNoun="deck"
            action="Open cards"
            onAction={() => onSelectView("decks")}
            testId="library-overview-decks"
          >
            {subjectDecks.slice(0, GROUP_ROWS).map((deck) => (
              <ListRow
                key={deck.id}
                leading={<ListRowMedia icon="cards" />}
                title={deck.name}
                meta={deckMeta(deck)}
                chevron
                onActivate={() => onOpenDeck(deck.id)}
              />
            ))}
          </OverviewGroup>
        ) : null}
      </div>
    </div>
  );
}

function OverviewGroup({
  title,
  count,
  countNoun,
  action,
  onAction,
  testId,
  children,
}: {
  title: string;
  count: number;
  countNoun: string;
  action: string;
  onAction: () => void;
  testId: string;
  children: React.ReactNode;
}) {
  return (
    <Panel
      title={title}
      description={countLabel(count, countNoun)}
      data-testid={testId}
      headingLevel={2}
      actions={
        <Button variant="text" size="small" onClick={onAction}>
          {action}
        </Button>
      }
    >
      <ListRows>{children}</ListRows>
    </Panel>
  );
}

/** A page of the chosen subject's newest guides, or null while it loads. */
function useSubjectGuides(subject: LibrarySubjectScope, refreshKey: number) {
  const [guides, setGuides] = useState<{ items: StudyGuidePageItem[]; totalCount: number } | null>(null);
  const key = subject.kind === "subject" ? `subject:${subject.subjectId}` : subject.kind;
  useEffect(() => {
    let current = true;
    const filter = key.startsWith("subject:")
      ? ({ kind: "subject", subjectId: key.slice("subject:".length) } as const)
      : ({ kind: key === "none" ? "none" : "all" } as const);
    void listStudyGuidePage({ sort: "recent", offset: 0, limit: GROUP_ROWS, subject: filter })
      .then((page) => {
        if (current) setGuides({ items: page.items, totalCount: page.totalCount });
      })
      // The Study guides page reports its own errors.
      .catch(() => {
        if (current) setGuides({ items: [], totalCount: 0 });
      });
    return () => {
      current = false;
    };
  }, [key, refreshKey]);
  return guides;
}

/** The items filed in the chosen subject; No subject holds the unfiled ones. */
function inSubject<T extends { id: string }, K extends "documentId" | "deckId">(
  items: readonly T[],
  placements: SubjectLibrary["documents"] | SubjectLibrary["decks"],
  idKey: K,
  subject: LibrarySubjectScope,
): T[] {
  if (subject.kind === "all") return [...items];
  const filed = new Map<string, string | null>(
    placements.map((placement) => [(placement as unknown as Record<K, string>)[idKey], placement.subjectId]),
  );
  return items.filter((item) => {
    const subjectId = filed.get(item.id) ?? null;
    return subject.kind === "none" ? subjectId === null : subjectId === subject.subjectId;
  });
}

function withSubject(meta: string, subjectName: string | undefined): string {
  return subjectName ? `${subjectName} · ${meta}` : meta;
}

export function lessonMeta(lesson: LessonSummary): string {
  const steps = lesson.written?.stepCount ?? 0;
  if (lesson.progress?.finishedAt) return "Done";
  if (lesson.progress) return `Step ${Math.min(steps, lesson.progress.currentStep + 1)} of ${steps}`;
  return countLabel(steps, "step");
}

function guideMeta(guide: StudyGuidePageItem): string {
  if (guide.targetKind === "collection") return `Across ${countLabel(guide.sourceCount, "document")}`;
  return (guide.sourceFormat ?? "document").replaceAll("_", " ").toUpperCase();
}

function documentMeta(document: DocumentSummary): string {
  return [
    formatDocumentType(document.sourceFormat),
    document.generatedCardCount > 0 ? countLabel(document.generatedCardCount, "card") : null,
  ]
    .filter(Boolean)
    .join(" · ");
}

function deckMeta(deck: DeckSummary): string {
  return [
    countLabel(deck.generatedCardCount, "card"),
    deck.studyableDueCount > 0 ? `${deck.studyableDueCount.toLocaleString()} due` : null,
  ]
    .filter(Boolean)
    .join(" · ");
}
