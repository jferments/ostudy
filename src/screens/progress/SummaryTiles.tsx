import type { CardStrength, ProgressActivity, ProgressBucket, ProgressOverview } from "../../ipc/progress";
import { cn } from "../../ui/internal/cn";
import { HelpTrigger } from "../../ui/help/HelpProvider";
import { StatGrid, StatTile } from "../../ui/primitives/StatTile";
import { strengthTotal } from "./StrengthPanel";
import { daysBetween, formatPercent, formatStudyTime, plural, pointsDelta } from "./progressFormat";

/** A line through each period's remembered rate; periods without reviews leave a gap. */
function Sparkline({ timeline }: { timeline: ProgressBucket[] }) {
  const points = timeline.map((bucket, index) => ({ index, value: bucket.correctRate }));
  const known = points.filter((point) => point.value !== null) as { index: number; value: number }[];
  if (known.length < 2) return null;
  const values = known.map((point) => point.value);
  const low = Math.min(...values), high = Math.max(...values);
  const spread = Math.max(0.05, high - low);
  const width = 100, height = 24;
  const x = (index: number) => (timeline.length === 1 ? width : (index / (timeline.length - 1)) * width);
  const y = (value: number) => height - 2 - ((value - low) / spread) * (height - 4);
  const path = known.map((point, order) => `${order === 0 ? "M" : "L"}${x(point.index).toFixed(1)} ${y(point.value).toFixed(1)}`).join(" ");
  const last = known.at(-1)!;
  return (
    <svg aria-hidden="true" viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none" className="tw:h-6 tw:w-full tw:overflow-visible tw:text-success">
      <path d={path} fill="none" stroke="currentColor" strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
      <circle cx={x(last.index)} cy={y(last.value)} r="2" fill="currentColor" />
    </svg>
  );
}

function MiniBars({ values }: { values: number[] }) {
  const max = Math.max(1, ...values);
  return (
    <span aria-hidden="true" className="tw:flex tw:h-6 tw:items-end tw:gap-px">
      {values.map((value, index) => (
        <span key={index} className={cn("tw:min-w-0 tw:flex-1 tw:rounded-t-[1px]", value > 0 ? "tw:bg-chart-1" : "tw:bg-progress-track")}
          style={{ height: value > 0 ? `${Math.max(8, (value / max) * 100)}%` : "2px" }} />
      ))}
    </span>
  );
}

function DayDots({ studied }: { studied: boolean[] }) {
  return (
    <span aria-hidden="true" className="tw:flex tw:flex-wrap tw:gap-1">
      {studied.map((value, index) => (
        <span key={index} className={cn("tw:size-2.5 tw:rounded-full", value ? "tw:bg-chart-1" : "tw:bg-progress-track")} />
      ))}
    </span>
  );
}

/** The range as the Study time tile names it: "this month", "all time". */
function rangeWords(overview: ProgressOverview): string {
  switch (overview.scope.range) {
    case "7d": return "this week";
    case "30d": return "this month";
    case "90d": return "last 90 days";
    case "1y": return "this year";
    case "all": return "all time";
    default: return "these dates";
  }
}

interface SummaryTilesProps {
  overview: ProgressOverview | null;
  strength: CardStrength | null;
  activity: ProgressActivity | null;
  loading: boolean;
}

/** Remembered, study time, days studied and solid cards for the scope and range. */
export function SummaryTiles({ overview, strength, activity, loading }: SummaryTilesProps) {
  const summary = overview?.summary;
  const timeline = overview?.timeline ?? [];
  const daily = timeline.length > 0 && timeline.every((bucket) => bucket.startLocalDate === bucket.endLocalDate);
  const rangeDays = overview?.scope.startLocalDate ? daysBetween(overview.scope.startLocalDate, overview.scope.endLocalDate) + 1 : null;
  const streak = activity?.currentStreak?.length ?? 0;
  const total = strength ? strengthTotal(strength) : 0;
  const pending = "…";
  return (
    <section aria-label="Learning summary" aria-busy={loading || undefined} className="tw:mb-4">
    <StatGrid className="tw:lg:grid-cols-4">
      <StatTile
        label="Remembered"
        value={summary ? formatPercent(summary.correctRate) : pending}
        delta={summary ? pointsDelta(summary.correctRate, overview?.previousSummary?.reviewCount ? overview.previousSummary.correctRate : null) : undefined}
        hint={summary && summary.reviewCount > 0 ? `${summary.rememberedCount.toLocaleString()} of ${plural(summary.reviewCount, "review")}` : undefined}
      >
        {timeline.length > 1 ? <Sparkline timeline={timeline} /> : null}
      </StatTile>
      <StatTile
        label="Study time"
        value={summary ? formatStudyTime(summary.studyTimeMs) : pending}
        hint={overview ? rangeWords(overview) : undefined}
      >
        {timeline.length > 1 ? <MiniBars values={timeline.map((bucket) => bucket.studyTimeMs)} /> : null}
      </StatTile>
      <StatTile
        label="Days studied"
        value={overview ? <>{overview.studyDayCount}{rangeDays !== null ? <span className="tw:text-body tw:font-normal tw:text-muted-foreground"> of {rangeDays}</span> : null}</> : pending}
        hint={streak > 0 ? `${streak}-day streak` : undefined}
      >
        {daily ? <DayDots studied={timeline.slice(-15).map((bucket) => bucket.reviewCount > 0)} /> : null}
      </StatTile>
      <StatTile
        label="Solid cards"
        labelAction={<HelpTrigger topicId="progress.solidCards" label="What solid means" />}
        value={strength ? <>{strength.solidCount.toLocaleString()}<span className="tw:text-body tw:font-normal tw:text-muted-foreground"> of {total.toLocaleString()}</span></> : pending}
      />
    </StatGrid>
    </section>
  );
}
