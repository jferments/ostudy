import type { HomeToday } from "../../ipc/home";
import { Icon } from "../../ui/icons/Icon";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { Button } from "../../ui/primitives/Button";
import { ButtonGroup } from "../../ui/primitives/ButtonGroup";
import { Card } from "../../ui/primitives/Card";
import { cn } from "../../ui/internal/cn";
import { countLabel, formatStudyDate } from "./homeModel";

/** The most decks one practice session can study. */
export const MAX_SESSION_DECKS = 100;

interface TodayStripProps {
  studyDate: string;
  today: HomeToday;
  headingId: string;
  onStartReview: () => void;
  onChooseSubjects: () => void;
}

/**
 * The slim Today strip: what Start review studies today, about how long it
 * takes and across how many subjects, with Start review and a way to choose
 * subjects first. The count is exactly what Start review schedules.
 */
export function TodayStrip({ studyDate, today, headingId, onStartReview, onChooseSubjects }: TodayStripProps) {
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const due = today.scheduledReviews + today.scheduledNew;
  const date = formatStudyDate(studyDate);
  const tooManyDecks = today.deckIds.length > MAX_SESSION_DECKS;
  const limitsReached = due === 0 && today.dueReviews + today.dueNew > 0;

  const heading =
    due > 0
      ? `${countLabel(due, "card")} due today`
      : limitsReached
        ? "Done for today"
        : "Nothing due today";
  const detail =
    due > 0
      ? compact
        ? `About ${countLabel(today.estimatedMinutes, "minute")}`
        : `About ${countLabel(today.estimatedMinutes, "minute")} across ${countLabel(today.subjectCount, "subject")} · ${date}`
      : limitsReached
        ? `Today's review limits are reached · ${date}`
        : date;

  return (
    <Card
      as="section"
      aria-labelledby={headingId}
      data-testid="home-today"
      data-tour-anchor="home.today"
      className="tw:flex-row tw:flex-wrap tw:items-center tw:gap-x-4 tw:gap-y-3 tw:p-3.5 tw:pl-4 tw:compact:gap-3 tw:compact:p-3 tw:compact:pl-3.5"
    >
      <span
        aria-hidden="true"
        className="tw:grid tw:size-11 tw:shrink-0 tw:place-items-center tw:rounded-md tw:bg-accent tw:text-accent-foreground tw:compact:size-10"
      >
        <Icon name="calendar-check" size={22} />
      </span>
      <div className="tw:min-w-0 tw:flex-[1_1_13.75rem] tw:compact:flex-1">
        <h2 id={headingId} className="tw:m-0 tw:text-body-large tw:leading-snug tw:font-semibold tw:compact:text-body">
          {heading}
        </h2>
        <p className="tw:m-0 tw:text-small tw:text-muted-foreground">{detail}</p>
        {tooManyDecks ? (
          <p className="tw:m-0 tw:text-small tw:text-muted-foreground">
            Cards are due in more than {MAX_SESSION_DECKS} decks. Choose subjects to review them in parts.
          </p>
        ) : null}
      </div>
      {due > 0 ? (
        <ButtonGroup
          aria-label="Review today's cards"
          className={cn("tw:rounded-full tw:shadow-glow", compact && "tw:w-full")}
        >
          <Button
            variant="primary"
            className="tw:h-11 tw:flex-1 tw:rounded-l-full tw:pr-4 tw:pl-5 tw:text-body tw:shadow-none"
            disabled={tooManyDecks}
            onClick={onStartReview}
          >
            <Icon name="practice" size={15} /> Start review
          </Button>
          <Button
            variant="primary"
            size="icon"
            aria-label="Choose subjects to review"
            aria-haspopup="dialog"
            className="tw:h-11 tw:w-10 tw:rounded-r-full tw:border-0 tw:border-l tw:border-solid tw:border-l-primary-foreground/35 tw:shadow-none"
            onClick={onChooseSubjects}
          >
            <Icon name="chevron-down" size={16} />
          </Button>
        </ButtonGroup>
      ) : null}
    </Card>
  );
}
