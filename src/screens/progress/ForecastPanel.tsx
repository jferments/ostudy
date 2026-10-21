import type { ProgressForecast } from "../../ipc/progress";
import { cn } from "../../ui/internal/cn";
import { Panel } from "../../ui/primitives/Panel";
import { formatNarrowWeekday, formatWeekday, plural } from "./progressFormat";

/** Reviews due on each of the next 14 study days, with the busiest one called out. */
export function ForecastPanel({ forecast, loading }: { forecast: ProgressForecast | null; loading: boolean }) {
  const days = forecast?.days ?? [];
  const counts = days.map((day) => day.dueCount);
  const max = Math.max(1, ...counts);
  const average = counts.length === 0 ? 0 : counts.reduce((sum, count) => sum + count, 0) / counts.length;
  const peakIndex = counts.indexOf(Math.max(...counts));
  // A day well above the fortnight's average is worth knowing about in advance.
  const peak = peakIndex > 0 && counts[peakIndex]! >= 10 && counts[peakIndex]! > average * 1.5 ? days[peakIndex] : undefined;
  const summary = days.length === 0
    ? "No forecast yet."
    : days.map((day, index) => `${index === 0 ? "Today" : formatWeekday(day.localDate)} ${day.dueCount}`).join(", ");

  return (
    <Panel headingLevel={2} title="Coming up" aria-busy={loading || undefined}>
      <div role="img" aria-label={`Reviews due: ${summary}.`} className="tw:grid tw:h-36 tw:grid-flow-col tw:auto-cols-fr tw:items-end tw:gap-1 tw:border-0 tw:border-b tw:border-solid tw:border-border">
        {days.map((day, index) => (
          <span key={day.localDate} title={`${index === 0 ? "Today" : formatWeekday(day.localDate)}: ${plural(day.dueCount, "review")} due`}
            className="tw:flex tw:h-full tw:flex-col tw:items-center tw:justify-end tw:gap-0.5">
            {index === 0 || day === peak ? <span className={cn("tw:text-caption tw:font-semibold tw:tabular-nums", day === peak ? "tw:text-warning" : "tw:text-primary")}>{day.dueCount}</span> : null}
            <span
              className={cn("tw:w-full tw:rounded-t-[2px]", index === 0 ? "tw:bg-primary" : day === peak ? "tw:bg-warning" : "tw:bg-chart-1/45")}
              style={{ height: `${(day.dueCount / max) * 100}%` }}
            />
          </span>
        ))}
      </div>
      <div aria-hidden="true" className="tw:-mt-2 tw:grid tw:grid-flow-col tw:auto-cols-fr tw:gap-1 tw:text-center tw:text-caption tw:text-muted-foreground">
        {days.map((day, index) => (
          <span key={day.localDate} className={cn(index === 0 && "tw:font-semibold tw:text-primary", day === peak && "tw:font-semibold tw:text-warning")}>
            {formatNarrowWeekday(day.localDate)}
          </span>
        ))}
      </div>
    </Panel>
  );
}
