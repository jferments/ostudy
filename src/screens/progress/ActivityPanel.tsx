import type { CSSProperties, ReactNode } from "react";

import type { ProgressActivity, Streak } from "../../ipc/progress";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Panel } from "../../ui/primitives/Panel";
import {
  daysBetween,
  formatLongDate,
  formatLongDay,
  formatShortMonth,
  formatStudyTime,
  mondayIndex,
  plural,
  shiftLocalDate,
  weekdayName,
} from "./progressFormat";

const LEVELS = ["tw:bg-progress-track", "tw:bg-chart-1/30", "tw:bg-chart-1/55", "tw:bg-chart-1/80", "tw:bg-chart-1"] as const;
const ROW_LABELS = ["Mon", "", "Wed", "", "Fri", "", ""];

/** Cut points between the four shades: quartiles of the busy days, so a light and a heavy learner both see contrast. */
function thresholds(counts: number[]): number[] {
  const sorted = counts.filter((count) => count > 0).sort((left, right) => left - right);
  if (sorted.length === 0) return [1, 1, 1];
  return [0.25, 0.5, 0.75].map((quantile) => sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * quantile))]!);
}

function level(count: number, cuts: number[]): number {
  if (count === 0) return 0;
  return 1 + cuts.filter((cut) => count > cut).length;
}

export function streakText(streak: Streak | null, today: string): string {
  if (!streak) return "None yet";
  const length = plural(streak.length, "day");
  if (streak.endLocalDate === today || streak.endLocalDate === shiftLocalDate(today, -1)) return `${length}, since ${formatLongDate(streak.startLocalDate)}`;
  return `${length}, ${formatLongDate(streak.startLocalDate)} to ${formatLongDate(streak.endLocalDate)}`;
}

/** The whole library's last 26 weeks as a calendar, Monday first, with streaks beside it. */
export function ActivityPanel({ activity, loading }: { activity: ProgressActivity | null; loading: boolean }) {
  if (!activity) return <Panel headingLevel={2} title="Activity" aria-busy={loading || undefined} />;
  const byDate = new Map(activity.days.map((day) => [day.localDate, day]));
  const first = shiftLocalDate(activity.startLocalDate, -mondayIndex(activity.startLocalDate));
  const weekCount = Math.floor(daysBetween(first, activity.todayLocalDate) / 7) + 1;
  const cuts = thresholds(activity.days.map((day) => day.reviewCount));
  const reviewTotal = activity.days.reduce((sum, day) => sum + day.reviewCount, 0);
  const studyTotal = activity.days.reduce((sum, day) => sum + day.studyTimeMs, 0);
  const weeks = Array.from({ length: weekCount }, (_, week) => shiftLocalDate(first, week * 7));
  const summary = `${plural(reviewTotal, "review")} on ${plural(activity.days.length, "day")} over the last six months, ${formatStudyTime(studyTotal)} in all.`;

  return (
    <Panel headingLevel={2} title="Activity" description={`Last six months · ${plural(reviewTotal, "review")} on ${plural(activity.days.length, "day")}`} aria-busy={loading || undefined}>
      <div className="tw:grid tw:min-w-0 tw:gap-6 tw:wide:grid-cols-[minmax(0,1fr)_16rem]">
        <figure className="tw:m-0 tw:min-w-0 tw:overflow-x-auto">
          <div role="img" aria-label={summary} className="tw:inline-grid tw:grid-cols-[auto_repeat(var(--weeks),minmax(0.75rem,1.25rem))] tw:gap-[3px] tw:text-caption tw:text-muted-foreground"
            style={{ "--weeks": String(weekCount) } as CSSProperties}>
            <span />
            {weeks.map((weekStart, index) => {
              const month = formatShortMonth(weekStart);
              // A month is named over its first week; the first column only when its month goes on.
              const showMonth = index === 0 ? formatShortMonth(weeks[1] ?? weekStart) === month : formatShortMonth(weeks[index - 1]!) !== month;
              return <span key={weekStart} aria-hidden="true" className="tw:overflow-visible tw:whitespace-nowrap">{showMonth ? month : ""}</span>;
            })}
            {ROW_LABELS.map((label, row) => [
              <span key={`label-${row}`} aria-hidden="true" className="tw:pr-1.5 tw:leading-none">{label}</span>,
              ...weeks.map((weekStart) => {
                const date = shiftLocalDate(weekStart, row);
                if (date > activity.todayLocalDate) return <span key={date} />;
                const day = byDate.get(date);
                const count = day?.reviewCount ?? 0;
                const isToday = date === activity.todayLocalDate;
                return (
                  <span key={date} title={`${formatLongDay(date)}: ${count === 0 ? "no study" : `${plural(count, "review")}, ${formatStudyTime(day!.studyTimeMs)}`}`}
                    className={cn("tw:aspect-square tw:rounded-[3px]", LEVELS[level(count, cuts)], isToday && count === 0 && "tw:outline-1 tw:outline-dashed tw:outline-muted-foreground")} />
                );
              }),
            ])}
          </div>
          <figcaption aria-hidden="true" className="tw:mt-2 tw:flex tw:items-center tw:gap-1 tw:text-caption tw:text-muted-foreground">
            Fewer {LEVELS.map((shade) => <i key={shade} className={cn("tw:size-2.5 tw:rounded-[2px]", shade)} />)} More reviews
          </figcaption>
        </figure>
        <dl className="tw:m-0 tw:grid tw:content-start tw:gap-4">
          <Fact icon="practice" label="Current streak">{streakText(activity.currentStreak, activity.todayLocalDate)}</Fact>
          <Fact icon="check" label="Longest streak">{streakText(activity.longestStreak, activity.todayLocalDate)}</Fact>
          <Fact icon="calendar" label="Busiest day of the week">{activity.busiestWeekday === null ? "None yet" : weekdayName(activity.busiestWeekday)}</Fact>
        </dl>
      </div>
    </Panel>
  );
}

function Fact({ icon, label, children }: { icon: IconName; label: string; children: ReactNode }) {
  return (
    <div className="tw:grid tw:grid-cols-[auto_minmax(0,1fr)] tw:items-center tw:gap-3">
      <span aria-hidden="true" className="tw:grid tw:size-8 tw:place-items-center tw:rounded-full tw:bg-accent tw:text-primary"><Icon name={icon} size={16} /></span>
      <div className="tw:min-w-0">
        <dt className="tw:text-caption tw:text-muted-foreground">{label}</dt>
        <dd className="tw:m-0 tw:font-semibold">{children}</dd>
      </div>
    </div>
  );
}
