import { useEffect, useMemo, useState } from "react";

import { useDocumentJobs } from "../../controllers/DocumentJobsProvider";
import { useHomeSummary } from "../../controllers/HomeSummaryProvider";
import type { DeckSummary } from "../../ipc/decks";
import type { LessonSummary } from "../../ipc/lessons";
import {
  getProgressForecast,
  getProgressOverview,
  getProgressSubjects,
  type ProgressOverview,
  type ProgressQuery,
} from "../../ipc/progress";
import { listStudyGuidePage, type StudyGuideSubjectFilter } from "../../ipc/studyGuides";
import type { LibrarySection, LibrarySubjectScope } from "../../navigation/appDestinations";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { EmptyState } from "../../ui/primitives/EmptyState";
import { ItemTrigger } from "../../ui/primitives/Item";
import { Panel } from "../../ui/primitives/Panel";
import { StatGrid, StatTile } from "../../ui/primitives/StatTile";
import { countLabel } from "../home/homeModel";
import { useHomeLessons } from "../home/HomeStudyPanels";
import { formatNarrowWeekday, formatWeekday, plural } from "../progress/progressFormat";
import { useLibrarySubjects } from "./LibrarySubjects";

/** The most decks "How each deck is going" reads; each is one small query. */
const STRENGTH_ROWS = 8;
/** A fall this large in remembered due reviews, against the 30 days before, puts a deck in Up next. */
const SLIPPING_DROP = 0.1;
const UP_NEXT_ROWS = 4;
/** Below this, a remembered rate shows in the warning color, with its number beside it. */
const WEAK_RETENTION = 0.7;

interface LibraryOverviewProps {
  subject: LibrarySubjectScope;
  decks: readonly DeckSummary[];
  onSelectView: (view: LibrarySection) => void;
  onOpenLesson: (subjectId: string, lessonId: string) => void;
  onOpenDeck: (deckId: string) => void;
  onPracticeDeck: (deckId: string) => void;
  /** Opens a document's study guide where the learner left off. */
  onOpenGuide: (documentId: string, title: string) => void;
  onOpenSubject: (subjectId: string) => void;
  onOpenProgress: () => void;
}

interface UpNextItem {
  key: string;
  icon: IconName;
  title: string;
  meta: string;
  action: string;
  onAction: () => void;
}

/**
 * The chosen subject's home: four facts about its reviews, what to do next
 * (a lesson or guide to pick up, a deck that is slipping, a document with
 * nothing made from it yet) and how each deck is going. The tabs hold the
 * subject's lessons, guides, documents and cards, so the overview does not
 * list them again. Everything shows the same for the whole library, by
 * subject.
 */
export function LibraryOverview({
  subject,
  decks,
  onSelectView,
  onOpenLesson,
  onOpenDeck,
  onPracticeDeck,
  onOpenGuide,
  onOpenSubject,
  onOpenProgress,
}: LibraryOverviewProps) {
  const { summary, library } = useHomeSummary();
  const jobs = useDocumentJobs();
  const subjects = useLibrarySubjects(subject);
  const refreshKey = summary?.today.scheduledReviews ?? 0;

  // The chosen subject's live decks, in the learner's order.
  const scopeDecks = useMemo(() => {
    if (!library) return [];
    const byId = new Map(decks.filter((deck) => deck.archivedAt === null).map((deck) => [deck.id, deck]));
    const order = new Map(library.subjects.map((entry) => [entry.id, entry.sortOrder]));
    return library.decks
      .filter((placement) =>
        subject.kind === "all" || (subject.kind === "none" ? placement.subjectId === null : placement.subjectId === subject.subjectId),
      )
      .sort((left, right) =>
        (order.get(left.subjectId ?? "") ?? Infinity) - (order.get(right.subjectId ?? "") ?? Infinity) || left.sortOrder - right.sortOrder,
      )
      .flatMap((placement) => byId.get(placement.deckId) ?? []);
  }, [decks, library, subject]);
  const scopeDocuments = useMemo(() => {
    if (!library || !jobs) return [];
    const filed = new Map(library.documents.map((placement) => [placement.documentId, placement.subjectId]));
    return jobs.documents.filter((document) => {
      const subjectId = filed.get(document.id) ?? null;
      return subject.kind === "all" || (subject.kind === "none" ? subjectId === null : subjectId === subject.subjectId);
    });
  }, [jobs, library, subject]);
  const subjectIds = useMemo(
    () =>
      subject.kind === "subject"
        ? [subject.subjectId]
        : subject.kind === "all"
          ? subjects.subjects.flatMap((tile) => (tile.subjectId === null ? [] : [tile.subjectId]))
          : [],
    [subject, subjects.subjects],
  );

  // No subject has no id to filter by, so its queries name its decks; without decks there is nothing to ask.
  const query = useMemo((): Omit<ProgressQuery, "range"> | null => {
    if (subject.kind === "all") return { subjectIds: [], deckIds: [], tagIds: [] };
    if (subject.kind === "subject") return { subjectIds: [subject.subjectId], deckIds: [], tagIds: [] };
    return scopeDecks.length > 0 ? { subjectIds: [], deckIds: scopeDecks.map((deck) => deck.id), tagIds: [] } : null;
  }, [scopeDecks, subject]);
  const queryKey = query === null ? null : `${JSON.stringify(query)}:${refreshKey}`;
  const month = useLatestRead(queryKey, () => getProgressOverview({ ...query!, range: "30d" }));
  const week = useLatestRead(queryKey, () => getProgressOverview({ ...query!, range: "7d" }));
  const forecast = useLatestRead(queryKey, () => getProgressForecast({ ...query!, range: "30d" }));
  const strengthDecks = scopeDecks.slice(0, STRENGTH_ROWS);
  const deckStrengthKey =
    subject.kind === "all" || strengthDecks.length === 0 ? null : `${strengthDecks.map((deck) => deck.id).join(",")}:${refreshKey}`;
  const deckStrength = useLatestRead(deckStrengthKey, () =>
    Promise.all(
      strengthDecks.map(async (deck) => ({
        deck,
        overview: await getProgressOverview({ range: "30d", subjectIds: [], deckIds: [deck.id], tagIds: [] }),
      })),
    ),
  );
  const subjectStrength = useLatestRead(subject.kind === "all" ? `subjects:${refreshKey}` : null, () =>
    getProgressSubjects({ range: "30d", subjectIds: [], deckIds: [], tagIds: [] }),
  );
  const lessons = useHomeLessons(subjectIds, refreshKey);
  const guideFilter: StudyGuideSubjectFilter = subject;
  const guides = useLatestRead(`${JSON.stringify(guideFilter)}:${refreshKey}`, () =>
    listStudyGuidePage({ sort: "recent", offset: 0, limit: 24, subject: guideFilter }),
  );

  const tiles = subject.kind === "all"
    ? [...subjects.subjects, ...(subjects.noSubject ? [subjects.noSubject] : [])]
    : subjects.current ? [subjects.current] : [];
  const due = tiles.reduce((sum, tile) => sum + tile.dueCount, 0);
  const minutes = summary && due > 0 ? Math.max(1, Math.round((due * summary.today.estimateMsPerCard) / 60_000)) : 0;
  // The forecast's first day is today, overdue included; the facts count the seven after it.
  const nextWeek = forecast ? forecast.days.slice(1, 8).reduce((sum, day) => sum + day.dueCount, 0) : null;

  const upNext = useMemo(() => {
    const names = new Map(subjects.subjects.map((tile) => [tile.subjectId, tile.name]));
    const items: UpNextItem[] = [];
    const started = subjectIds
      .flatMap((id) => lessons?.bySubject.get(id) ?? [])
      .filter((lesson) => lesson.written !== null && lesson.progress !== null && lesson.progress.finishedAt === null)
      .sort((left, right) => (right.progress?.updatedAt ?? 0) - (left.progress?.updatedAt ?? 0))[0];
    if (started) {
      items.push({
        key: `lesson:${started.id}`,
        icon: "book",
        title: `Resume ${started.title}`,
        meta: `Lesson · ${lessonMeta(started).toLowerCase()}`,
        action: "Resume",
        onAction: () => onOpenLesson(started.subjectId, started.id),
      });
    }
    const reading = (guides?.items ?? [])
      .filter((guide) => guide.targetKind === "document" && guide.readSectionCount > 0 && guide.readSectionCount < guide.sectionCount)
      .sort((left, right) => (right.lastOpenedAt ?? 0) - (left.lastOpenedAt ?? 0))[0];
    if (reading) {
      items.push({
        key: `guide:${reading.guideId}`,
        icon: "study-guide",
        title: `Keep reading ${reading.title}`,
        meta: `${Math.round((reading.readSectionCount / reading.sectionCount) * 100)}% read`,
        action: "Read",
        onAction: () => onOpenGuide(reading.targetId, reading.title),
      });
    }
    if (subject.kind === "all") {
      for (const row of subjectStrength ?? []) {
        if (row.subjectId === null || row.trueRetention === null || row.retentionDelta === null || row.retentionDelta > -SLIPPING_DROP) continue;
        const subjectId = row.subjectId;
        items.push({
          key: `subject:${subjectId}`,
          icon: "sparkles",
          title: `${row.name ?? names.get(subjectId) ?? "A subject"} is slipping`,
          meta: slippingMeta(row.trueRetention, row.trueRetention - row.retentionDelta),
          action: "Open",
          onAction: () => onOpenSubject(subjectId),
        });
      }
    } else {
      for (const { deck, overview } of deckStrength ?? []) {
        const now = overview.summary.trueRetention;
        const before = overview.previousSummary?.trueRetention ?? null;
        if (now === null || before === null || before - now < SLIPPING_DROP) continue;
        items.push({
          key: `deck:${deck.id}`,
          icon: "sparkles",
          title: `${deck.name} is slipping`,
          meta: slippingMeta(now, before),
          action: "Practice",
          onAction: () => onPracticeDeck(deck.id),
        });
      }
    }
    const untouched = scopeDocuments.find(
      (document) => document.lifecycleState === "ready" && document.generatedCardCount === 0 && document.generatedDeckId === null,
    );
    if (untouched) {
      items.push({
        key: `document:${untouched.id}`,
        icon: "document",
        title: `${untouched.title} has no cards yet`,
        meta: `Added ${new Date(untouched.createdAt).toLocaleDateString(undefined, { month: "short", day: "numeric" })}`,
        action: "Make cards",
        onAction: () => onSelectView("documents"),
      });
    }
    return items.slice(0, UP_NEXT_ROWS);
  }, [deckStrength, guides, lessons, onOpenGuide, onOpenLesson, onOpenSubject, onPracticeDeck, onSelectView, scopeDocuments, subject.kind, subjectIds, subjectStrength, subjects.subjects]);

  const empty =
    subjects.ready && library !== null && guides !== undefined &&
    scopeDecks.length === 0 && scopeDocuments.length === 0 && (guides?.totalCount ?? 0) === 0;
  if (empty) {
    return <EmptyState icon="library" title={subject.kind === "all" ? "Your library is empty" : "Nothing here yet"} />;
  }

  const pending = "…";
  const remembered = month?.summary.trueRetention ?? null;
  const strengthRows = subject.kind === "all"
    ? (subjectStrength ?? [])
        .filter((row) => row.subjectId !== null && row.cardCount > 0)
        .map((row) => ({
          key: row.subjectId!,
          name: row.name ?? "Subject",
          retention: row.trueRetention,
          onOpen: () => onOpenSubject(row.subjectId!),
        }))
    : (deckStrength ?? []).map(({ deck, overview }) => ({
        key: deck.id,
        name: deck.name,
        retention: overview.summary.trueRetention,
        onOpen: () => onOpenDeck(deck.id),
      }));
  const strengthLoading = subject.kind === "all" ? subjectStrength === undefined : deckStrength === undefined && strengthDecks.length > 0;
  return (
    <div data-testid="library-overview" className="tw:grid tw:min-w-0 tw:gap-5">
      <StatGrid className="tw:lg:grid-cols-4" data-testid="library-overview-facts">
        <StatTile label="Due now" value={summary ? due.toLocaleString() : pending} hint={due > 0 ? `About ${plural(minutes, "minute")}` : "Nothing due"} />
        <StatTile
          label="Remembered"
          value={month === undefined && query !== null ? pending : remembered === null ? "None yet" : `${Math.round(remembered * 100)}%`}
          hint="Last 30 days"
        />
        <StatTile
          label="Days studied"
          value={week === undefined && query !== null ? pending : <>{week?.studyDayCount ?? 0}<span className="tw:text-body tw:font-normal tw:text-muted-foreground"> of 7</span></>}
          hint="This week"
        />
        <StatTile label="Coming up" value={forecast === undefined && query !== null ? pending : (nextWeek ?? 0).toLocaleString()} hint="Due in the next 7 days" />
      </StatGrid>
      <div className="tw:grid tw:items-start tw:gap-5 tw:wide:grid-cols-2">
        {upNext.length > 0 ? (
          <Panel title="Up next" headingLevel={2} data-testid="library-overview-up-next">
            <ul className="tw:m-0 tw:grid tw:list-none tw:p-0">
              {upNext.map((item) => (
                <li
                  key={item.key}
                  className="tw:flex tw:min-w-0 tw:items-center tw:gap-3 tw:py-2.5 tw:not-last:border-0 tw:not-last:border-b tw:not-last:border-solid tw:not-last:border-border"
                >
                  <span aria-hidden="true" className="tw:grid tw:size-9 tw:shrink-0 tw:place-items-center tw:rounded-md tw:bg-primary/10 tw:text-primary">
                    <Icon name={item.icon} size={18} />
                  </span>
                  <span className="tw:grid tw:min-w-0 tw:flex-1">
                    <span className="tw:wrap-anywhere tw:text-body tw:font-medium">{item.title}</span>
                    <span className="tw:text-small tw:text-muted-foreground">{item.meta}</span>
                  </span>
                  <Button variant="text" size="small" className="tw:shrink-0" onClick={item.onAction}>{item.action}</Button>
                </li>
              ))}
            </ul>
          </Panel>
        ) : null}
        <Panel
          title={subject.kind === "all" ? "How each subject is going" : "How each deck is going"}
          headingLevel={2}
          data-testid="library-overview-strength"
          actions={
            <Button variant="text" size="small" onClick={onOpenProgress}>
              Progress <Icon name="chevron-right" size={16} />
            </Button>
          }
        >
          {strengthLoading ? (
            <p className="tw:m-0 tw:text-small tw:text-muted-foreground">Loading…</p>
          ) : strengthRows.length === 0 ? (
            <p className="tw:m-0 tw:text-small tw:text-muted-foreground">No decks yet</p>
          ) : (
            <StrengthRows rows={strengthRows} />
          )}
          {week ? <ReviewBars week={week} /> : null}
        </Panel>
      </div>
    </div>
  );
}

/** Each deck's (or subject's) remembered rate over the last 30 days; a row opens it. */
function StrengthRows({ rows }: { rows: { key: string; name: string; retention: number | null; onOpen: () => void }[] }) {
  return (
    <ul className="tw:m-0 tw:grid tw:list-none tw:p-0">
      {rows.map((row) => {
        const percent = row.retention === null ? null : Math.round(row.retention * 100);
        return (
          <li key={row.key} className="tw:not-last:border-0 tw:not-last:border-b tw:not-last:border-solid tw:not-last:border-border">
            <ItemTrigger
              onClick={row.onOpen}
              aria-label={`${row.name}, ${percent === null ? "no reviews in the last 30 days" : `${percent}% remembered`}`}
              className="tw:grid tw:min-h-11 tw:w-full tw:grid-cols-[minmax(0,1fr)_minmax(4rem,9rem)_3rem] tw:items-center tw:gap-3 tw:rounded-sm tw:text-start tw:outline-none tw:hover:bg-foreground/4 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50"
            >
              <span className="tw:truncate tw:text-body">{row.name}</span>
              <span aria-hidden="true" className="tw:h-2 tw:overflow-hidden tw:rounded-full tw:bg-progress-track">
                {percent !== null ? (
                  <span
                    className={cn("tw:block tw:h-full tw:rounded-full", row.retention! < WEAK_RETENTION ? "tw:bg-warning" : "tw:bg-primary")}
                    style={{ width: `${percent}%` }}
                  />
                ) : null}
              </span>
              <span className="tw:text-end tw:text-body tw:font-semibold tw:tabular-nums">{percent === null ? "–" : `${percent}%`}</span>
            </ItemTrigger>
          </li>
        );
      })}
    </ul>
  );
}

/** Reviews on each of the last seven days, today in front. */
function ReviewBars({ week }: { week: ProgressOverview }) {
  const days = week.timeline.slice(-7);
  if (days.length === 0) return null;
  const max = Math.max(1, ...days.map((day) => day.reviewCount));
  const name = (index: number, date: string) => (index === days.length - 1 ? "Today" : formatWeekday(date));
  return (
    <figure className="tw:m-0 tw:mt-3 tw:grid tw:gap-1.5">
      <figcaption className="tw:text-small tw:text-muted-foreground">Reviews, last 7 days</figcaption>
      <div
        role="img"
        aria-label={`Reviews: ${days.map((day, index) => `${name(index, day.startLocalDate)} ${day.reviewCount}`).join(", ")}.`}
        className="tw:grid tw:h-16 tw:grid-flow-col tw:auto-cols-fr tw:items-end tw:gap-0.5 tw:border-0 tw:border-b tw:border-solid tw:border-border"
      >
        {days.map((day, index) => (
          <span
            key={day.startLocalDate}
            title={`${name(index, day.startLocalDate)}: ${plural(day.reviewCount, "review")}`}
            className={cn("tw:w-full tw:rounded-t-[4px]", index === days.length - 1 ? "tw:bg-primary" : "tw:bg-chart-1/45")}
            style={{ height: day.reviewCount > 0 ? `${Math.max(6, (day.reviewCount / max) * 100)}%` : "2px" }}
          />
        ))}
      </div>
      <div aria-hidden="true" className="tw:grid tw:grid-flow-col tw:auto-cols-fr tw:gap-0.5 tw:text-center tw:text-caption tw:text-muted-foreground">
        {days.map((day, index) => (
          <span key={day.startLocalDate} className={cn(index === days.length - 1 && "tw:font-semibold tw:text-primary")}>
            {formatNarrowWeekday(day.startLocalDate)}
          </span>
        ))}
      </div>
    </figure>
  );
}

function slippingMeta(now: number, before: number): string {
  return `${Math.round(now * 100)}% remembered, down from ${Math.round(before * 100)}%`;
}

/**
 * Reads one value for `key`, again whenever the key changes. An answer for an
 * older key never replaces a newer one. It is undefined while reading and
 * null when the read failed, since Progress reports its own errors. A null
 * key reads nothing and stays undefined.
 */
function useLatestRead<T>(key: string | null, read: () => Promise<T>): T | null | undefined {
  const [state, setState] = useState<{ key: string; value: T | null } | null>(null);
  useEffect(() => {
    if (key === null) return;
    let current = true;
    void read().then(
      (value) => {
        if (current) setState({ key, value });
      },
      () => {
        if (current) setState({ key, value: null });
      },
    );
    return () => {
      current = false;
    };
    // The key names everything the read depends on.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return state !== null && state.key === key ? state.value : undefined;
}

export function lessonMeta(lesson: LessonSummary): string {
  const steps = lesson.written?.stepCount ?? 0;
  if (lesson.progress?.finishedAt) return "Done";
  if (lesson.progress) return `Step ${Math.min(steps, lesson.progress.currentStep + 1)} of ${steps}`;
  return countLabel(steps, "step");
}
