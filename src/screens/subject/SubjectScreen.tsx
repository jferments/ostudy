import { useId, useMemo, useState } from "react";

import type { ShowToast } from "../../components/Toast";
import { useDocumentJobs } from "../../controllers/DocumentJobsProvider";
import { useHomeSummary } from "../../controllers/HomeSummaryProvider";
import type { DeckSummary } from "../../ipc/decks";
import type { DocumentGenerationProgress, DocumentSummary } from "../../ipc/documents";
import type { LibraryScope } from "../../navigation/appDestinations";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { Button } from "../../ui/primitives/Button";
import { Card } from "../../ui/primitives/Card";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ItemTrigger } from "../../ui/primitives/Item";
import { CollectionStudyGuideDialog } from "../CollectionStudyGuideDialog";
import { LessonPlayer } from "../lessons/LessonPlayer";
import { LessonsSection } from "../lessons/LessonsSection";
import { useSubjectCourse } from "../lessons/useSubjectCourse";
import { formatDocumentType, processingPhaseLabel, sourceUnitHeading } from "../documents/DocumentProgress";
import {
  buildSubjectTiles,
  countLabel,
  SUBJECT_COLORS,
  type SubjectTile,
} from "../home/homeModel";
import { SubjectBadge } from "../home/SubjectBadge";
import { MAX_SESSION_DECKS } from "../home/TodayStrip";

const NO_PREPARATIONS = new Map();

interface SubjectScreenProps {
  subjectId: string;
  /** Every deck the app knows; the page shows the ones filed in this subject. */
  decks: readonly DeckSummary[];
  onBack: () => void;
  /** Starts the subject's scheduled review over these decks. */
  onStartReview: (deckIds: string[]) => void;
  /** An extra, shuffled session over the whole subject. */
  onPracticeSubject: (subjectId: string) => void;
  onOpenLibrary: (scope: LibraryScope) => void;
  /** Library's Documents view, where sources are added and prepared. */
  onOpenDocuments: () => void;
  showToast: ShowToast;
}

/**
 * One subject's page: what is due and its own Start, the overview guide
 * across its sources, its lessons, the sources themselves and its decks, with Manage
 * opening Library scoped to the subject.
 */
export function SubjectScreen({
  subjectId,
  decks,
  onBack,
  onStartReview,
  onPracticeSubject,
  onOpenLibrary,
  onOpenDocuments,
  showToast,
}: SubjectScreenProps) {
  const { summary, library, summaryError, libraryError, isLoading, refresh } = useHomeSummary();
  const jobs = useDocumentJobs();
  const course = useSubjectCourse(subjectId);
  const [readingOverview, setReadingOverview] = useState(false);
  const [openLessonId, setOpenLessonId] = useState<string | null>(null);
  const overviewHeadingId = useId();
  const lessonsHeadingId = useId();
  const sourcesHeadingId = useId();
  const cardsHeadingId = useId();

  const tile = useMemo<SubjectTile | null>(() => {
    if (!summary || !library) return null;
    return (
      buildSubjectTiles(summary, library, NO_PREPARATIONS).subjects.find(
        (candidate) => candidate.subjectId === subjectId,
      ) ?? null
    );
  }, [library, subjectId, summary]);

  const sources = useMemo<DocumentSummary[]>(() => {
    if (!library || !jobs) return [];
    const filed = new Set(
      library.documents
        .filter((placement) => placement.subjectId === subjectId)
        .map((placement) => placement.documentId),
    );
    return jobs.documents.filter((document) => filed.has(document.id));
  }, [jobs, library, subjectId]);

  const subjectDecks = useMemo<DeckSummary[]>(() => {
    if (!library) return [];
    const order = new Map(
      library.decks
        .filter((placement) => placement.subjectId === subjectId)
        .map((placement) => [placement.deckId, placement.sortOrder] as const),
    );
    return decks
      .filter((deck) => deck.archivedAt === null && order.has(deck.id))
      .sort((left, right) => (order.get(left.id) ?? 0) - (order.get(right.id) ?? 0));
  }, [decks, library, subjectId]);

  const readError = summaryError ?? libraryError;
  const writtenLessons = course.course?.lessons.filter((lesson) => lesson.written !== null) ?? [];
  const openLessonIndex = writtenLessons.findIndex((lesson) => lesson.id === openLessonId);

  if (openLessonId && tile && openLessonIndex >= 0) {
    return (
      <PageScaffold width="workspace" aria-label={`${tile.name} lesson`}>
        <LessonPlayer
          lessonId={openLessonId}
          subjectName={tile.name}
          lessonNumber={openLessonIndex + 1}
          lessonCount={writtenLessons.length}
          showToast={showToast}
          onClose={() => {
            setOpenLessonId(null);
            course.refresh();
          }}
        />
      </PageScaffold>
    );
  }

  if (readingOverview && tile) {
    // The overview guide reads like any other guide, then returns here.
    return (
      <PageScaffold width="full" inset aria-label={`${tile.name} overview guide`}>
        <CollectionStudyGuideDialog
          collectionId={subjectId}
          collectionTitle={tile.name}
          showToast={showToast}
          onClose={() => setReadingOverview(false)}
        />
      </PageScaffold>
    );
  }

  return (
    <PageScaffold width="workspace" data-testid="subject-screen">
      <nav aria-label="Breadcrumb" className="tw:mb-4 tw:compact:mb-3">
        <Button size="small" variant="text" className="tw:ps-0" onClick={onBack}>
          <Icon name="chevron-left" size={16} /> Home
        </Button>
      </nav>
      {readError ? (
        <InlineNotice kind={tile ? "warning" : "error"} className="tw:mb-6">
          {tile ? "This page may be out of date" : "This subject could not load"}: {readError}{" "}
          <Button size="small" variant="text" onClick={refresh}>
            Retry
          </Button>
        </InlineNotice>
      ) : null}
      {isLoading && !tile ? <LoadingState label="Loading the subject…" /> : null}
      {summary && library && !tile && !isLoading ? (
        <InlineNotice kind="info">
          This subject no longer exists.{" "}
          <Button size="small" variant="text" onClick={onBack}>
            Back to Home
          </Button>
        </InlineNotice>
      ) : null}
      {tile ? (
        <div className="tw:flex tw:flex-col tw:gap-8 tw:compact:gap-6">
          <SubjectHeader
            tile={tile}
            onManage={() => onOpenLibrary({ subjectId })}
            onStartReview={() => onStartReview(tile.summary.dueDeckIds)}
          />
          <div className="tw:grid tw:items-start tw:gap-8 tw:compact:gap-6 tw:wide:grid-cols-[minmax(0,1fr)_20rem] tw:medium:grid-cols-[minmax(0,1fr)_17.5rem]">
            <div className="tw:flex tw:min-w-0 tw:flex-col tw:gap-8 tw:compact:gap-6">
              <OverviewSection
                headingId={overviewHeadingId}
                tile={tile}
                onRead={() => setReadingOverview(true)}
                onOpenDocuments={onOpenDocuments}
              />
              <LessonsSection
                headingId={lessonsHeadingId}
                course={course}
                hasSources={sources.length > 0}
                onOpenLesson={setOpenLessonId}
              />
              <SourcesSection
                headingId={sourcesHeadingId}
                sources={sources}
                loading={jobs?.isLoading ?? false}
                cancelingIds={jobs?.cancelingDocumentIds ?? new Set()}
                guideWritingIds={jobs?.guideGeneratingDocumentIds ?? new Set()}
                generationProgress={jobs?.generationProgress ?? {}}
                onOpenDocuments={onOpenDocuments}
              />
            </div>
            <CardsSection
              headingId={cardsHeadingId}
              tile={tile}
              decks={subjectDecks}
              onOpenDeck={(deckId) => onOpenLibrary({ deckId })}
              onBrowse={() => onOpenLibrary({ subjectId })}
              onPractice={() => onPracticeSubject(subjectId)}
            />
          </div>
        </div>
      ) : null}
    </PageScaffold>
  );
}

function SubjectHeader({
  tile,
  onManage,
  onStartReview,
}: {
  tile: SubjectTile;
  onManage: () => void;
  onStartReview: () => void;
}) {
  const { sourceCount, cardCount, dueDeckIds } = tile.summary;
  return (
    <header className="tw:flex tw:flex-wrap tw:items-center tw:gap-x-4 tw:gap-y-3">
      <SubjectBadge subject={tile} size="large" />
      <div className="tw:min-w-0 tw:flex-[1_1_14rem]">
        <h1 className="tw:m-0 tw:text-title tw:leading-tight tw:font-semibold tw:tracking-tight tw:break-words tw:compact:text-section">
          {tile.name}
        </h1>
        <p className="tw:m-0 tw:text-body tw:text-muted-foreground tw:compact:text-small">
          {countLabel(sourceCount, "source")} · {countLabel(cardCount, "card")}
        </p>
      </div>
      <div className="tw:flex tw:flex-wrap tw:items-center tw:gap-2">
        <Button className="tw:rounded-full" onClick={onManage}>
          <Icon name="library" size={16} /> Manage
        </Button>
        {tile.dueCount > 0 ? (
          <Button
            variant="primary"
            className="tw:rounded-full tw:shadow-glow"
            disabled={dueDeckIds.length > MAX_SESSION_DECKS}
            onClick={onStartReview}
          >
            <Icon name="practice" size={15} /> Review {tile.dueCount.toLocaleString()} due
          </Button>
        ) : null}
      </div>
    </header>
  );
}

function SectionHeading({ id, title, detail }: { id: string; title: string; detail?: string }) {
  return (
    <div className="tw:flex tw:flex-col tw:gap-0.5">
      <h2 id={id} className="tw:m-0 tw:text-body-large tw:font-semibold">
        {title}
      </h2>
      {detail ? <p className="tw:m-0 tw:text-small tw:text-muted-foreground">{detail}</p> : null}
    </div>
  );
}

function OverviewSection({
  headingId,
  tile,
  onRead,
  onOpenDocuments,
}: {
  headingId: string;
  tile: SubjectTile;
  onRead: () => void;
  onOpenDocuments: () => void;
}) {
  const { hasOverviewGuide, sourceCount } = tile.summary;
  return (
    <section aria-labelledby={headingId} data-testid="subject-overview" className="tw:flex tw:flex-col tw:gap-3">
      <SectionHeading id={headingId} title="Overview across your sources" />
      <Card
        as="div"
        className="tw:flex-row tw:flex-wrap tw:items-center tw:gap-x-4 tw:gap-y-3 tw:p-5 tw:compact:p-4"
      >
        <span
          aria-hidden="true"
          className="tw:grid tw:size-11 tw:shrink-0 tw:place-items-center tw:rounded-md tw:bg-accent tw:text-accent-foreground"
        >
          <Icon name="study-guide" size={22} />
        </span>
        <p className="tw:m-0 tw:min-w-0 tw:flex-[1_1_15rem] tw:text-body tw:text-muted-foreground">
          {hasOverviewGuide
            ? "One guide that brings together what your sources say about this subject."
            : sourceCount === 0
              ? "Add sources to this subject. Each source gets its own guide and cards, and an overview can then bring them together."
              : "No overview guide yet. Write one from this subject's sources on Library's Documents page."}
        </p>
        {hasOverviewGuide ? (
          <Button variant="primary" className="tw:rounded-full" onClick={onRead}>
            <Icon name="book" size={16} /> Read overview
          </Button>
        ) : (
          <Button className="tw:rounded-full" onClick={onOpenDocuments}>
            <Icon name="document" size={16} /> {sourceCount === 0 ? "Add sources" : "Open Documents"}
          </Button>
        )}
      </Card>
    </section>
  );
}

function SourcesSection({
  headingId,
  sources,
  loading,
  cancelingIds,
  guideWritingIds,
  generationProgress,
  onOpenDocuments,
}: {
  headingId: string;
  sources: readonly DocumentSummary[];
  loading: boolean;
  cancelingIds: ReadonlySet<string>;
  guideWritingIds: ReadonlySet<string>;
  generationProgress: Readonly<Record<string, DocumentGenerationProgress | null | undefined>>;
  onOpenDocuments: () => void;
}) {
  if (sources.length === 0 && !loading) return null;
  return (
    <section aria-labelledby={headingId} data-testid="subject-sources" className="tw:flex tw:flex-col tw:gap-3">
      <SectionHeading
        id={headingId}
        title="Sources"
        detail="Each source gets its own guide and cards first, then joins the overview."
      />
      {sources.length === 0 ? (
        <LoadingState label="Loading sources…" />
      ) : (
        <Card as="div" className="tw:gap-0 tw:py-0">
          <ul className="tw:m-0 tw:list-none tw:p-0">
            {sources.map((document) => (
              <li
                key={document.id}
                data-testid="subject-source"
                className="tw:border-0 tw:border-b tw:border-solid tw:border-border tw:last:border-b-0"
              >
                <ItemTrigger
                  onClick={onOpenDocuments}
                  className="tw:w-full tw:items-center tw:gap-3 tw:px-5 tw:py-3 tw:text-start tw:outline-none tw:hover:bg-accent/40 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:compact:px-4"
                >
                  <span
                    aria-hidden="true"
                    className="tw:grid tw:h-8 tw:min-w-11 tw:shrink-0 tw:place-items-center tw:rounded-sm tw:bg-(--ostudy-color-canvas-inset) tw:px-1.5 tw:text-caption tw:font-semibold tw:text-muted-foreground"
                  >
                    {formatBadge(document.sourceFormat)}
                  </span>
                  <span className="tw:flex tw:min-w-0 tw:flex-1 tw:flex-col">
                    <span className="tw:font-medium tw:text-body tw:break-words">{document.title}</span>
                    <span className="tw:text-small tw:text-muted-foreground">
                      {[unitsLabel(document), formatDocumentType(document.sourceFormat)]
                        .filter(Boolean)
                        .join(" · ")}
                    </span>
                  </span>
                  <span className="tw:flex tw:shrink-0 tw:flex-col tw:items-end tw:gap-0.5 tw:text-end">
                    <SourceStatus
                      document={document}
                      canceling={cancelingIds.has(document.id)}
                      writingGuide={guideWritingIds.has(document.id)}
                      progress={generationProgress[document.id] ?? null}
                    />
                    {document.generatedCardCount > 0 ? (
                      <span className="tw:text-label tw:text-muted-foreground">
                        {countLabel(document.generatedCardCount, "card")}
                      </span>
                    ) : null}
                  </span>
                </ItemTrigger>
              </li>
            ))}
          </ul>
        </Card>
      )}
    </section>
  );
}

function SourceStatus({
  document,
  canceling,
  writingGuide,
  progress,
}: {
  document: DocumentSummary;
  canceling: boolean;
  writingGuide: boolean;
  progress: DocumentGenerationProgress | null;
}) {
  const label =
    document.lifecycleState === "processing"
      ? `Preparing · ${processingPhaseLabel(document, canceling, progress)}`
      : writingGuide
        ? "Writing a study guide"
        : document.lifecycleState === "failed"
          ? "Needs attention"
          : "Ready";
  return (
    <span
      className={cn(
        "tw:rounded-full tw:px-2.5 tw:py-0.5 tw:text-label tw:font-medium",
        document.lifecycleState === "failed"
          ? "tw:bg-destructive/10 tw:text-destructive"
          : document.lifecycleState === "processing" || writingGuide
            ? "tw:bg-accent tw:text-accent-foreground"
            : "tw:bg-(--ostudy-color-canvas-inset) tw:text-muted-foreground",
      )}
    >
      {label}
    </span>
  );
}

function CardsSection({
  headingId,
  tile,
  decks,
  onOpenDeck,
  onBrowse,
  onPractice,
}: {
  headingId: string;
  tile: SubjectTile;
  decks: readonly DeckSummary[];
  onOpenDeck: (deckId: string) => void;
  onBrowse: () => void;
  onPractice: () => void;
}) {
  const { cardCount, scheduledNew, scheduledReviews } = tile.summary;
  const color = tile.colorIndex === null ? null : SUBJECT_COLORS[tile.colorIndex];
  return (
    <section aria-labelledby={headingId} data-testid="subject-cards" className="tw:flex tw:min-w-0 tw:flex-col tw:gap-3">
      <SectionHeading id={headingId} title="Cards" detail={countLabel(cardCount, "card")} />
      <Card as="div" className="tw:gap-4 tw:p-5 tw:compact:p-4">
        <dl className="tw:m-0 tw:grid tw:grid-cols-3 tw:gap-3">
          <Stat label="Due today" value={tile.dueCount} highlight={color?.soft ?? null} />
          <Stat label="Reviews" value={scheduledReviews} />
          <Stat label="New" value={scheduledNew} />
        </dl>
        {decks.length > 0 ? (
          <ul className="tw:m-0 tw:flex tw:list-none tw:flex-col tw:gap-1 tw:p-0" aria-label="Decks">
            {decks.map((deck) => (
              <li key={deck.id}>
                <ItemTrigger
                  data-testid="subject-deck"
                  onClick={() => onOpenDeck(deck.id)}
                  className="tw:w-full tw:items-center tw:gap-2.5 tw:rounded-md tw:px-2 tw:py-2 tw:text-start tw:outline-none tw:hover:bg-accent/40 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50"
                >
                  <Icon name="cards" size={16} className="tw:shrink-0 tw:text-muted-foreground" />
                  <span className="tw:min-w-0 tw:flex-1 tw:text-body tw:break-words">{deck.name}</span>
                  <span className="tw:shrink-0 tw:text-label tw:text-muted-foreground tw:tabular-nums">
                    {deck.generatedCardCount.toLocaleString()}
                  </span>
                </ItemTrigger>
              </li>
            ))}
          </ul>
        ) : (
          <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
            No decks yet. Cards made from this subject&apos;s sources, or decks you move here in
            Library, show up here.
          </p>
        )}
        <div className="tw:flex tw:flex-wrap tw:gap-2">
          <Button className="tw:rounded-full" onClick={onBrowse}>
            Browse cards
          </Button>
          {cardCount > 0 ? (
            <Button variant="text" className="tw:rounded-full" onClick={onPractice}>
              Practice a mix
            </Button>
          ) : null}
        </div>
      </Card>
    </section>
  );
}

function Stat({ label, value, highlight = null }: { label: string; value: number; highlight?: string | null }) {
  return (
    <div
      className={cn(
        "tw:flex tw:min-w-0 tw:flex-col tw:gap-0.5 tw:rounded-md tw:px-3 tw:py-2",
        highlight ?? "tw:bg-(--ostudy-color-canvas-inset)",
      )}
    >
      <dt className="tw:text-label tw:text-muted-foreground">{label}</dt>
      <dd className="tw:m-0 tw:font-heading tw:text-title-small tw:font-semibold tw:tabular-nums">
        {value.toLocaleString()}
      </dd>
    </div>
  );
}

function formatBadge(sourceFormat: string): string {
  switch (sourceFormat) {
    case "doc":
    case "docx":
    case "odt":
      return "DOC";
    case "ppt":
    case "pptx":
    case "odp":
      return "PPT";
    case "image_bundle":
      return "IMG";
    default:
      return sourceFormat.replaceAll("_", " ").slice(0, 4).toUpperCase();
  }
}

function unitsLabel(document: DocumentSummary): string {
  if (document.sourceUnitCount <= 0) return "";
  const plural = sourceUnitHeading(document.sourceUnitKind).toLocaleLowerCase();
  const unit = document.sourceUnitCount === 1 ? plural.replace(/s$/u, "") : plural;
  return `${document.sourceUnitCount.toLocaleString()} ${unit}`;
}
