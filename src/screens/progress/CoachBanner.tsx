import type { TagAnalytics } from "../../ipc/analytics";
import type { ProgressActivity, ProgressOverview } from "../../ipc/progress";
import { useArtwork } from "../../ui/artwork/Artwork";
import { Icon } from "../../ui/icons/Icon";
import { Button } from "../../ui/primitives/Button";
import { Skeleton } from "../../ui/primitives/Skeleton";
import { daysBetween, formatPercent, plural, shiftLocalDate } from "./progressFormat";

/** The topic the banner points at: the first one native ranks as needing attention that has cards due. */
export function slippingTopic(topics: TagAnalytics[] | null): TagAnalytics | undefined {
  return (topics ?? []).find((topic) => topic.status === "needs_attention" && topic.studyableDueCount > 0 && topic.reviewCount > 0);
}

export function rangeEyebrow(overview: ProgressOverview): string {
  const { range } = overview.scope;
  if (range === "all") return "All time";
  if (range === "custom") return "These dates";
  if (range === "1y") return "Your last year";
  return `Your last ${range.slice(0, -1)} days`;
}

function periodWord(overview: ProgressOverview): string {
  switch (overview.scope.range) {
    case "7d": return "week";
    case "30d": return "month";
    case "1y": return "year";
    default: return "period";
  }
}

/** The plain-language summary at the top of the page. */
export function coachSentence(overview: ProgressOverview, topic: TagAnalytics | undefined): string {
  const { summary, previousSummary, scope } = overview;
  if (summary.reviewCount === 0) {
    return scope.range === "all"
      ? "Nothing studied yet. Review a few cards and your progress shows up here."
      : "No reviews in this range. Review a few cards and your progress shows up here.";
  }
  const remembered = formatPercent(summary.correctRate);
  let sentence: string;
  if (scope.range === "all" || scope.startLocalDate === null) {
    sentence = `You've studied on ${plural(overview.studyDayCount, "day")} and remembered ${remembered} of what you reviewed`;
  } else {
    const dayCount = daysBetween(scope.startLocalDate, scope.endLocalDate) + 1;
    const days = scope.range === "custom" ? `${overview.studyDayCount} of these ${dayCount} days` : `${overview.studyDayCount} of the last ${dayCount} days`;
    sentence = `You studied ${days} and remembered ${remembered} of what you reviewed`;
  }
  const previous = previousSummary?.correctRate ?? null;
  if (summary.correctRate !== null && previous !== null && (previousSummary?.reviewCount ?? 0) > 0) {
    const points = Math.round((summary.correctRate - previous) * 100);
    const before = `the ${periodWord(overview)} before`;
    sentence += points === 0 ? `, the same as ${before}.` : `, ${plural(Math.abs(points), "point")} ${points > 0 ? "better" : "lower"} than ${before}.`;
  } else {
    sentence += ".";
  }
  if (topic) sentence += ` ${topic.tagName} is the one slipping, at ${formatPercent(topic.correctRate)} remembered.`;
  return sentence;
}

/** "12-day streak. Today's 47 cards keep it going." */
export function streakSentence(activity: ProgressActivity | null, dueToday: number): string | null {
  const streak = activity?.currentStreak;
  if (!activity || !streak) return dueToday > 0 ? `${plural(dueToday, "card")} due today.` : null;
  const studiedToday = streak.endLocalDate === activity.todayLocalDate;
  const keepsGoing = streak.endLocalDate === shiftLocalDate(activity.todayLocalDate, -1);
  const length = `${streak.length}-day streak.`;
  if (studiedToday) return dueToday > 0 ? `${length} ${plural(dueToday, "card")} still due today.` : `${length} You're done for today.`;
  if (keepsGoing) return dueToday > 0 ? `${length} Today's ${plural(dueToday, "card")} keep it going.` : `${length} Study today to keep it going.`;
  return dueToday > 0 ? `${plural(dueToday, "card")} due today.` : null;
}

interface CoachBannerProps {
  overview: ProgressOverview | null;
  topic: TagAnalytics | undefined;
  activity: ProgressActivity | null;
  dueToday: number;
  busy: boolean;
  onPracticeTopic: (topic: TagAnalytics) => void;
  onStartReview: (() => void) | null;
}

export function CoachBanner({ overview, topic, activity, dueToday, busy, onPracticeTopic, onStartReview }: CoachBannerProps) {
  const mascot = useArtwork("progress.coach");
  const image = mascot.definition.image;
  const streak = streakSentence(activity, dueToday);
  return (
    <section aria-label="Summary" className="tw:mb-4 tw:grid tw:min-w-0 tw:grid-cols-[auto_minmax(0,1fr)] tw:items-center tw:gap-x-5 tw:gap-y-4 tw:rounded-lg tw:border tw:border-solid tw:border-primary/25 tw:bg-accent tw:p-5 tw:text-accent-foreground tw:compact:gap-x-3 tw:compact:p-4 tw:wide:grid-cols-[auto_minmax(0,1fr)_auto]">
      {mascot.enabled && image ? (
        <img src={image.src} srcSet={image.srcSet} alt="" width={64} height={64} className="tw:size-16 tw:rounded-full tw:bg-card tw:compact:size-12" />
      ) : <span />}
      <div className="tw:min-w-0">
        {overview ? <>
          <p className="tw:m-0 tw:text-caption tw:font-bold tw:uppercase tw:tracking-wide tw:text-primary">{rangeEyebrow(overview)}</p>
          <p className="tw:mt-1 tw:mb-0 tw:max-w-[60ch] tw:text-body-large tw:text-foreground tw:compact:text-body">{coachSentence(overview, topic)}</p>
          {streak ? (
            <p className="tw:mt-2 tw:mb-0 tw:text-small tw:text-foreground">
              <Icon name="practice" size={14} className="tw:mr-1.5 tw:inline-block tw:align-[-2px] tw:text-primary" />
              {streak}
              {onStartReview && topic ? <> <Button variant="text" className="tw:inline tw:h-auto tw:min-h-0 tw:p-0 tw:underline tw:underline-offset-4" disabled={busy} onClick={onStartReview}>Start review</Button></> : null}
            </p>
          ) : null}
        </> : (
          <div className="tw:grid tw:gap-2" aria-hidden="true"><Skeleton width="40%" height="0.75rem" /><Skeleton width="90%" height="1.25rem" /><Skeleton width="70%" height="1.25rem" /></div>
        )}
      </div>
      {overview && (topic || onStartReview) ? (
        <div className="tw:col-span-2 tw:wide:col-span-1">
          {topic ? (
            <Button variant="primary" size="large" className="tw:w-full" disabled={busy} onClick={() => onPracticeTopic(topic)}>
              <Icon name="practice" size={16} />Practice those {plural(topic.studyableDueCount, "card")}
            </Button>
          ) : (
            <Button variant="primary" size="large" className="tw:w-full" disabled={busy} onClick={onStartReview ?? undefined}>
              <Icon name="practice" size={16} />Review {plural(dueToday, "due card")}
            </Button>
          )}
        </div>
      ) : null}
    </section>
  );
}
