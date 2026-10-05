import { useId, useMemo } from "react";

import { useHomeSummary } from "../../controllers/HomeSummaryProvider";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { PageHeader, PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { Button } from "../../ui/primitives/Button";
import { Card } from "../../ui/primitives/Card";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import {
  buildSubjectTiles,
  countLabel,
  formatStudyDate,
  sortSubjectTiles,
  SUBJECT_COLORS,
  type SubjectTile,
} from "../home/homeModel";
import { SubjectBadge } from "../home/SubjectBadge";
import { MAX_SESSION_DECKS } from "../home/TodayStrip";

const NO_PREPARATIONS = new Map();

interface ReviewScreenProps {
  /** Starts today's scheduled review over these decks. */
  onStartReview: (deckIds: string[]) => void;
  /** Opens Library's decks, where selected decks open the practice setup. */
  onChooseDecks: () => void;
}

/**
 * Review: what is due today in total and by subject, each with its own
 * Start, and the way into a custom practice. A running session shows here
 * in place of this page.
 */
export function ReviewScreen({ onStartReview, onChooseDecks }: ReviewScreenProps) {
  const { summary, library, summaryError, libraryError, isLoading, refresh } = useHomeSummary();
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const totalHeadingId = useId();
  const subjectsHeadingId = useId();
  const customHeadingId = useId();

  const dueSubjects = useMemo(() => {
    if (!summary || !library) return [];
    const tiles = buildSubjectTiles(summary, library, NO_PREPARATIONS);
    const subjects = sortSubjectTiles(tiles.subjects, "most-due");
    const all = tiles.noSubject ? [...subjects, tiles.noSubject] : subjects;
    return all.filter((tile) => tile.dueCount > 0);
  }, [library, summary]);

  const readError = summaryError ?? libraryError;
  const today = summary?.today;
  const due = today ? today.scheduledReviews + today.scheduledNew : 0;
  const limitsReached = today ? due === 0 && today.dueReviews + today.dueNew > 0 : false;
  const tooManyDecks = today ? today.deckIds.length > MAX_SESSION_DECKS : false;

  return (
    <PageScaffold width="reading" data-testid="review-screen">
      <PageHeader
        title="Review"
        description={summary ? formatStudyDate(summary.studyDate) : undefined}
      />
      {readError ? (
        <InlineNotice kind={summary && library ? "warning" : "error"} className="tw:mb-6">
          {summary && library ? "Review may be out of date" : "Review could not load"}: {readError}{" "}
          <Button size="small" variant="text" onClick={refresh}>
            Retry
          </Button>
        </InlineNotice>
      ) : null}
      {isLoading && (!summary || !library) ? <LoadingState label="Loading today's review…" /> : null}
      {summary && library && today ? (
        <div className="tw:flex tw:flex-col tw:gap-8 tw:compact:gap-6">
          <Card
            as="section"
            aria-labelledby={totalHeadingId}
            data-testid="review-today"
            className="tw:flex-row tw:flex-wrap tw:items-center tw:gap-x-4 tw:gap-y-3 tw:p-5 tw:compact:p-4"
          >
            <span
              aria-hidden="true"
              className="tw:grid tw:size-13 tw:shrink-0 tw:place-items-center tw:rounded-md tw:bg-accent tw:text-accent-foreground tw:compact:size-11"
            >
              <Icon name={due > 0 ? "calendar-check" : "check"} size={24} />
            </span>
            <div className="tw:min-w-0 tw:flex-[1_1_13.75rem]">
              <h2
                id={totalHeadingId}
                className="tw:m-0 tw:text-title-small tw:leading-snug tw:font-semibold tw:compact:text-body-large"
              >
                {due > 0
                  ? `${countLabel(due, "card")} due today`
                  : limitsReached
                    ? "Done for today"
                    : "Nothing due today"}
              </h2>
              <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
                {due > 0
                  ? `About ${countLabel(today.estimatedMinutes, "minute")} across ${countLabel(today.subjectCount, "subject")}`
                  : limitsReached
                    ? "Today's review limits are reached. More cards come due tomorrow."
                    : "Rest is part of spaced repetition. New reviews come due as you learn."}
              </p>
              {tooManyDecks ? (
                <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
                  Cards are due in more than {MAX_SESSION_DECKS} decks. Start one subject at a time.
                </p>
              ) : null}
            </div>
            {due > 0 ? (
              <Button
                variant="primary"
                size="large"
                className="tw:rounded-full tw:shadow-glow"
                disabled={tooManyDecks}
                onClick={() => onStartReview(today.deckIds)}
              >
                <Icon name="practice" size={15} /> Start review
              </Button>
            ) : null}
          </Card>

          {dueSubjects.length > 0 ? (
            <section aria-labelledby={subjectsHeadingId} className="tw:flex tw:flex-col tw:gap-3">
              <h2 id={subjectsHeadingId} className="tw:m-0 tw:text-body-large tw:font-semibold">
                By subject
              </h2>
              <Card as="div" className="tw:gap-0 tw:py-0">
                <ul className="tw:m-0 tw:list-none tw:p-0" data-testid="review-subjects">
                  {dueSubjects.map((tile) => (
                    <SubjectRow
                      key={tile.key}
                      tile={tile}
                      compact={compact}
                      onStart={() => onStartReview(tile.summary.dueDeckIds)}
                    />
                  ))}
                </ul>
              </Card>
              {dueSubjects.length > 1 ? (
                <p className="tw:m-0 tw:text-label tw:text-muted-foreground">
                  Each subject counts its own cards. Today&apos;s limits are shared, so reviewing one
                  subject can lower what is left in the others.
                </p>
              ) : null}
            </section>
          ) : null}

          <section aria-labelledby={customHeadingId} className="tw:flex tw:flex-col tw:gap-3">
            <h2 id={customHeadingId} className="tw:m-0 tw:text-body-large tw:font-semibold">
              Custom practice
            </h2>
            <Card
              as="div"
              className="tw:flex-row tw:flex-wrap tw:items-center tw:justify-between tw:gap-x-4 tw:gap-y-3 tw:p-5 tw:compact:p-4"
            >
              <p className="tw:m-0 tw:min-w-0 tw:flex-[1_1_16rem] tw:text-body tw:text-muted-foreground">
                Open a deck in Library and choose Practice with options to narrow by tags, study
                extra cards beyond today&apos;s schedule or turn shuffling off.
              </p>
              <Button onClick={onChooseDecks}>
                <Icon name="library" size={16} /> Choose decks
              </Button>
            </Card>
          </section>
        </div>
      ) : null}
    </PageScaffold>
  );
}

function SubjectRow({
  tile,
  compact,
  onStart,
}: {
  tile: SubjectTile;
  compact: boolean;
  onStart: () => void;
}) {
  const color = tile.colorIndex === null ? null : SUBJECT_COLORS[tile.colorIndex];
  const tooManyDecks = tile.summary.dueDeckIds.length > MAX_SESSION_DECKS;
  return (
    <li
      data-testid="review-subject"
      data-subject-id={tile.subjectId ?? "none"}
      className="tw:flex tw:items-center tw:gap-3 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:px-5 tw:py-3 tw:last:border-b-0 tw:compact:px-4"
    >
      <SubjectBadge subject={tile} size={compact ? "small" : "medium"} />
      <span className="tw:min-w-0 tw:flex-1 tw:font-medium tw:text-body tw:break-words">
        {tile.name}
      </span>
      <span
        className={cn(
          "tw:shrink-0 tw:rounded-full tw:px-2.5 tw:py-0.5 tw:text-label tw:font-semibold tw:tabular-nums",
          color ? color.soft : "tw:bg-accent tw:text-accent-foreground",
        )}
      >
        {tile.dueCount.toLocaleString()} due
      </span>
      <Button
        className="tw:rounded-full tw:px-3.5"
        aria-label={`Start ${tile.name}`}
        disabled={tooManyDecks}
        onClick={onStart}
      >
        Start
      </Button>
    </li>
  );
}
