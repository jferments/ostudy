import { useState } from "react";

import { getSessionSummary, type SessionSummary, type Streak } from "../../ipc/progress";
import { useArtwork } from "../../ui/artwork/Artwork";
import { Icon } from "../../ui/icons/Icon";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { Button } from "../../ui/primitives/Button";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Skeleton } from "../../ui/primitives/Skeleton";
import { formatLongDate, formatPercent, formatStudyTime, formatWeekday, plural } from "../progress/progressFormat";
import { useLatestRead } from "../progress/useProgressData";
import type { ConfidenceCounts } from "./confidence";

interface PracticeCompleteProps {
  sessionId: string;
  ratedCount: number;
  confidenceCounts: ConfidenceCounts;
  isStarting: boolean;
  canRestart: boolean;
  errorMessage: string | null;
  onDone: () => void;
  onViewProgress: () => void;
  onPracticeCards: (cardIds: string[]) => void;
  onRestart: () => void;
}

const MODE_LABELS: Record<SessionSummary["mode"], string> = {
  scheduled: "Review",
  extra: "Extra practice",
  custom: "Practice",
};

const longMonthFormatter = new Intl.DateTimeFormat(undefined, { month: "long", timeZone: "UTC" });

/** "9 minutes", "under a minute" or "1 hr 5 min", for the opening sentence. */
export function sessionDuration(ms: number): string {
  if (ms < 60_000) return "under a minute";
  const minutes = Math.round(ms / 60_000);
  if (minutes < 60) return plural(minutes, "minute");
  const rest = minutes % 60;
  return `${Math.floor(minutes / 60)} hr${rest > 0 ? ` ${rest} min` : ""}`;
}

export function dueSentence(dueTodayCount: number | null): string {
  if (dueTodayCount === null) return "";
  return dueTodayCount === 0 ? " Everything due today is done." : ` ${plural(dueTodayCount, "more card")} due today.`;
}

/** "13-day streak, your longest since July"; null without a streak. */
export function streakLine(current: Streak | null, lastAsLong: Streak | null, today: string): string | null {
  if (!current) return null;
  if (current.length === 1) return "Day one of a new streak. Study tomorrow to keep it going.";
  const length = `${current.length}-day streak`;
  if (!lastAsLong) return `${length}, your longest yet`;
  const sameMonth = lastAsLong.endLocalDate.slice(0, 7) === today.slice(0, 7);
  const since = sameMonth
    ? formatLongDate(lastAsLong.endLocalDate)
    : longMonthFormatter.format(new Date(`${lastAsLong.endLocalDate}T12:00:00Z`));
  return `${length}, your longest since ${since}`;
}

/** What the session got wrong, and the tag they share when they share one. */
export function missedSentence(summary: SessionSummary): { lead: string; rest: string } | null {
  const missed = summary.missedCardIds.length;
  if (missed === 0) return null;
  const topic = summary.missedTopic;
  if (topic && topic.cardCount === missed) {
    return missed === 1
      ? { lead: "The one you missed", rest: ` is on ${topic.name}.` }
      : { lead: `The ${missed.toLocaleString()} you missed`, rest: ` are all on ${topic.name}.` };
  }
  const lead = `You missed ${plural(missed, "card")}`;
  return topic && topic.cardCount > 1
    ? { lead, rest: `, ${topic.cardCount.toLocaleString()} of them on ${topic.name}.` }
    : { lead, rest: "." };
}

export function PracticeComplete({
  sessionId, ratedCount, confidenceCounts, isStarting, canRestart, errorMessage, onDone, onViewProgress, onPracticeCards, onRestart,
}: PracticeCompleteProps) {
  const [attempt, setAttempt] = useState(0);
  const summaryState = useLatestRead(`${sessionId}:${attempt}`, () => getSessionSummary(sessionId));
  const summary = summaryState.data?.sessionId === sessionId ? summaryState.data : null;
  const mascot = useArtwork("practice.complete");
  const image = mascot.definition.image;

  // Until the summary arrives, the counts this screen kept while answering stand in.
  const rated = summary?.ratedCount ?? ratedCount;
  const remembered = summary?.rememberedCount ?? ratedCount - confidenceCounts[0];
  const rememberedRate = rated === 0 ? null : remembered / rated;
  const eyebrow = summary ? `${MODE_LABELS[summary.mode]} · ${formatWeekday(summary.today.localDate)}` : "Study session saved";
  const streak = summary ? streakLine(summary.currentStreak, summary.lastAsLongStreak, summary.today.localDate) : null;
  const missed = summary ? missedSentence(summary) : null;
  const studiedEarlierToday = summary !== null && summary.today.reviewCount > summary.ratedCount;
  const keepGoing = canRestart && (summary?.dueTodayCount ?? 0) > 0;

  return (
    <PageScaffold data-testid="practice-complete">
      <div className="tw:mx-auto tw:grid tw:w-full tw:max-w-md tw:justify-items-center tw:gap-4 tw:text-center">
        <p className="tw:m-0 tw:justify-self-start tw:text-small tw:text-muted-foreground">{eyebrow}</p>
        {mascot.enabled && image ? (
          <img src={image.src} srcSet={image.srcSet} alt="" width={240} height={240}
            className="tw:size-48 tw:object-contain tw:compact:size-40" />
        ) : (
          <span aria-hidden="true" className="tw:grid tw:size-12 tw:place-items-center tw:rounded-full tw:bg-success-soft tw:text-success">
            <Icon name="check" size={24} />
          </span>
        )}
        <div className="tw:grid tw:gap-1">
          <h1 className="tw:m-0 tw:text-title tw:font-semibold tw:tracking-tight">Session complete</h1>
          <p className="tw:m-0 tw:text-body-large tw:text-muted-foreground tw:compact:text-body">
            {summary
              ? `${plural(rated, "card")} in ${sessionDuration(summary.studyTimeMs)}.${dueSentence(summary.dueTodayCount)}`
              : `${plural(rated, "card")} reviewed. Every answer is saved.`}
          </p>
        </div>
        {streak ? (
          <p className="tw:m-0 tw:flex tw:w-full tw:items-center tw:justify-center tw:gap-2 tw:rounded-full tw:bg-accent tw:px-4 tw:py-2 tw:text-small tw:font-semibold tw:text-primary">
            <Icon name="calendar" size={16} />{streak}
          </p>
        ) : null}
        <dl className="tw:m-0 tw:grid tw:w-full tw:grid-cols-3 tw:gap-2" aria-label="This session">
          <SessionFact value={formatPercent(rememberedRate)} label="remembered" />
          <SessionFact value={summary ? summary.nowSolidCount.toLocaleString() : null} label={summary?.nowSolidCount === 1 ? "card now solid" : "cards now solid"} />
          <SessionFact value={summary ? formatStudyTime(summary.studyTimeMs) : null} label="focused time" />
        </dl>
        {studiedEarlierToday ? (
          <p className="tw:m-0 tw:text-caption tw:text-muted-foreground">
            Today so far: {plural(summary.today.reviewCount, "review")} in {formatStudyTime(summary.today.studyTimeMs)}.
          </p>
        ) : null}
        {summary && missed ? (
          <section aria-label="Missed cards" className="tw:flex tw:w-full tw:items-center tw:gap-3 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:p-4 tw:text-left">
            <p className="tw:m-0 tw:min-w-0 tw:flex-1 tw:text-small"><strong>{missed.lead}</strong>{missed.rest}</p>
            <Button variant="secondary" disabled={isStarting} onClick={() => onPracticeCards(summary.missedCardIds)}>
              {isStarting ? "Starting…" : `Practice ${summary.missedCardIds.length.toLocaleString()}`}
            </Button>
          </section>
        ) : summary && rated > 0 ? (
          <p className="tw:m-0 tw:text-small tw:text-muted-foreground">You remembered every card.</p>
        ) : null}
        {summaryState.error ? (
          <InlineNotice kind="info" className="tw:w-full tw:text-left">
            Your answers are saved, but this summary could not load: {summaryState.error}{" "}
            <Button variant="text" disabled={summaryState.loading} onClick={() => setAttempt((value) => value + 1)}>Try again</Button>
          </InlineNotice>
        ) : null}
        {errorMessage ? <InlineNotice kind="error" className="tw:w-full tw:text-left">{errorMessage}</InlineNotice> : null}
        <div className="tw:mt-2 tw:grid tw:w-full tw:gap-1">
          <Button variant="primary" className="tw:w-full" disabled={isStarting} onClick={onDone}>Done</Button>
          <Button variant="text" disabled={isStarting} onClick={onViewProgress}>See your progress</Button>
          {keepGoing ? (
            <Button variant="text" disabled={isStarting} onClick={onRestart}>
              {isStarting ? "Starting…" : `Keep going: ${plural(summary?.dueTodayCount ?? 0, "card")} due`}
            </Button>
          ) : null}
        </div>
      </div>
    </PageScaffold>
  );
}

function SessionFact({ value, label }: { value: string | null; label: string }) {
  // The label comes first for screen readers; the value shows on top.
  return (
    <div className="tw:flex tw:min-w-0 tw:flex-col-reverse tw:justify-center tw:gap-0.5 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:px-2 tw:py-3">
      <dt className="tw:text-caption tw:text-muted-foreground">{label}</dt>
      <dd className="tw:m-0 tw:text-section tw:font-semibold tw:tabular-nums">
        {value ?? <Skeleton height="1.5rem" className="tw:mx-auto tw:w-12" />}
      </dd>
    </div>
  );
}
