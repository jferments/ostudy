import { useState } from "react";

import type { ProgressBucket, ProgressOverview } from "../../ipc/progress";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { Panel } from "../../ui/primitives/Panel";
import { SegmentedControl } from "../../ui/primitives/SegmentedControl";
import { formatDay, formatLongDay, formatMonth, formatPercent, formatStudyTime, plural } from "./progressFormat";

type Measure = "reviews" | "minutes" | "new";
const MEASURES = [
  { value: "reviews", label: "Reviews" },
  { value: "minutes", label: "Minutes" },
  { value: "new", label: "New cards" },
] as const;

interface Bar {
  bucket: ProgressBucket;
  /** The lower, solid part: remembered reviews, or the whole value for the other measures. */
  lower: number;
  /** Forgotten reviews on top; 0 for the other measures. */
  upper: number;
}

function bars(timeline: ProgressBucket[], measure: Measure): Bar[] {
  return timeline.map((bucket) => {
    if (measure === "reviews") {
      return { bucket, lower: bucket.rememberedCount, upper: bucket.reviewCount - bucket.rememberedCount };
    }
    const value = measure === "minutes" ? Math.round(bucket.studyTimeMs / 60_000) : bucket.newCardCount;
    return { bucket, lower: value, upper: 0 };
  });
}

/** The average over periods with any study, which is what "a usual day" means to a learner. */
function usualValue(values: number[]): number | null {
  const studied = values.filter((value) => value > 0);
  if (studied.length < 2) return null;
  return Math.round(studied.reduce((sum, value) => sum + value, 0) / studied.length);
}

function bucketLabel(bucket: ProgressBucket, monthly: boolean): string {
  if (monthly) return formatMonth(bucket.startLocalDate);
  if (bucket.startLocalDate === bucket.endLocalDate) return formatLongDay(bucket.startLocalDate);
  return `${formatDay(bucket.startLocalDate)} to ${formatDay(bucket.endLocalDate)}`;
}

function measureText(bar: Bar, measure: Measure): string {
  const total = bar.lower + bar.upper;
  if (measure === "minutes") return `${total} min`;
  if (measure === "new") return plural(total, "new card");
  return `${plural(total, "review")}: ${bar.lower.toLocaleString()} remembered, ${bar.upper.toLocaleString()} forgot`;
}

/**
 * Remembered and forgotten reviews per period as stacked bars, with the usual
 * study day as a dashed line. The bars are one image; the table beside them
 * carries the same numbers for screen readers and anyone who wants them.
 */
export function ReviewsChart({ overview, loading }: { overview: ProgressOverview | null; loading: boolean }) {
  const [measure, setMeasure] = useState<Measure>("reviews");
  const [showTable, setShowTable] = useState(false);
  const timeline = overview?.timeline ?? [];
  const today = overview?.scope.todayLocalDate ?? "";
  const monthly = overview?.scope.range === "all";
  const daily = timeline.every((bucket) => bucket.startLocalDate === bucket.endLocalDate);
  const rows = bars(timeline, measure);
  const totals = rows.map((bar) => bar.lower + bar.upper);
  const usual = usualValue(totals);
  const scaleMax = Math.max(1, usual ?? 0, ...totals);
  const remembered = overview?.summary.rememberedCount ?? 0;
  const forgot = (overview?.summary.reviewCount ?? 0) - remembered;
  // Long custom ranges draw hundreds of days; their bars touch.
  const gap = rows.length > 60 ? "tw:gap-0" : rows.length > 31 ? "tw:gap-px" : "tw:gap-[3px]";
  const unit = monthly ? "month" : daily ? "day" : "period";
  // Five labels at most, always including the first and last periods.
  const labelEvery = Math.max(1, Math.ceil(rows.length / 5));
  const summary = rows.length === 0 || totals.every((value) => value === 0)
    ? "No reviews in this range."
    : `${MEASURES.find((item) => item.value === measure)?.label} per ${unit}, ${bucketLabel(timeline[0]!, monthly)} to ${bucketLabel(timeline.at(-1)!, monthly)}. Most in one ${unit}: ${Math.max(...totals).toLocaleString()}.${usual === null ? "" : ` Usual ${unit}: ${usual.toLocaleString()}.`}`;

  return (
    <Panel
      headingLevel={2}
      headingId="progress-heading"
      title="Reviews and recall"
      aria-labelledby="progress-heading"
      aria-busy={loading || undefined}
      actions={<Button variant="text" aria-expanded={showTable} disabled={!overview} onClick={() => setShowTable((value) => !value)}>
        {showTable ? "Show as chart" : "Show as table"}
      </Button>}
    >
      <SegmentedControl label="Chart shows" value={measure} options={MEASURES} onChange={setMeasure} className="tw:justify-self-start" />
      {showTable ? (
        <div className="tw:max-h-96 tw:w-full tw:min-w-0 tw:overflow-auto tw:rounded-md tw:ring-1 tw:ring-border">
          <table className="tw:w-full tw:border-collapse tw:text-left tw:text-small tw:tabular-nums tw:[&_th]:bg-muted/50 tw:[&_th]:px-3 tw:[&_th]:py-2 tw:[&_th]:text-caption tw:[&_th]:font-medium tw:[&_th]:text-muted-foreground tw:[&_td]:border-0 tw:[&_td]:border-t tw:[&_td]:border-solid tw:[&_td]:border-border tw:[&_td]:px-3 tw:[&_td]:py-2">
            <caption className="tw:sr-only">Reviews, recall and study time for each {unit}</caption>
            <thead><tr><th scope="col">{monthly ? "Month" : "Date"}</th><th scope="col">Reviews</th><th scope="col">Remembered</th><th scope="col">New cards</th><th scope="col">Study time</th></tr></thead>
            <tbody>{[...timeline].reverse().map((bucket) => (
              <tr key={bucket.startLocalDate}>
                <td>{bucketLabel(bucket, monthly)}</td>
                <td>{bucket.reviewCount.toLocaleString()}</td>
                <td>{formatPercent(bucket.correctRate)}</td>
                <td>{bucket.newCardCount.toLocaleString()}</td>
                <td>{formatStudyTime(bucket.studyTimeMs)}</td>
              </tr>
            ))}</tbody>
          </table>
        </div>
      ) : (
        <figure className="tw:m-0 tw:grid tw:min-w-0 tw:gap-3">
          {measure === "reviews" ? (
            <figcaption className="tw:flex tw:flex-wrap tw:gap-x-4 tw:gap-y-1 tw:text-caption tw:text-muted-foreground">
              <LegendSwatch className="tw:bg-chart-1" label="Remembered" value={remembered} />
              <LegendSwatch className="tw:bg-destructive" label="Forgot" value={forgot} />
              {usual !== null ? <span className="tw:inline-flex tw:items-center tw:gap-1.5"><i aria-hidden="true" className="tw:w-4 tw:border-0 tw:border-t-2 tw:border-dashed tw:border-muted-foreground" />Your usual {unit}</span> : null}
            </figcaption>
          ) : null}
          <div role="img" aria-label={summary} className={cn("tw:relative tw:grid tw:h-56 tw:min-w-0 tw:grid-flow-col tw:auto-cols-fr tw:items-end tw:border-0 tw:border-b tw:border-solid tw:border-border tw:compact:h-40", gap)}>
            {usual !== null ? (
              <span aria-hidden="true" className="tw:pointer-events-none tw:absolute tw:inset-x-0 tw:border-0 tw:border-t-2 tw:border-dashed tw:border-muted-foreground/70"
                style={{ bottom: `${(usual / scaleMax) * 100}%` }} />
            ) : null}
            {rows.map((bar) => (
              <span key={bar.bucket.startLocalDate} title={`${bucketLabel(bar.bucket, monthly)}: ${measureText(bar, measure)}`}
                className="tw:flex tw:h-full tw:min-w-0 tw:flex-col tw:justify-end">
                {bar.upper > 0 ? <span className="tw:rounded-t-[2px] tw:bg-destructive" style={{ height: `${(bar.upper / scaleMax) * 100}%` }} /> : null}
                {bar.lower > 0 ? (
                  <span className={cn("tw:bg-chart-1", bar.upper === 0 && "tw:rounded-t-[2px]")} style={{ height: `${(bar.lower / scaleMax) * 100}%` }} />
                ) : (
                  // An empty day keeps a dot so gaps read as "no study", not missing data.
                  bar.upper === 0 ? <span className="tw:mx-auto tw:mb-0.5 tw:size-1 tw:rounded-full tw:bg-border-strong" /> : null
                )}
              </span>
            ))}
          </div>
          <div aria-hidden="true" className={cn("tw:grid tw:min-w-0 tw:grid-flow-col tw:auto-cols-fr tw:text-caption tw:text-muted-foreground", gap)}>
            {rows.map((bar, index) => {
              const last = index === rows.length - 1;
              const show = last || (index % labelEvery === 0 && rows.length - 1 - index >= labelEvery / 2);
              const isToday = last && bar.bucket.endLocalDate === today && daily;
              return (
                <span key={bar.bucket.startLocalDate} className={cn("tw:flex tw:min-w-0 tw:whitespace-nowrap", last && "tw:justify-end", isToday && "tw:font-semibold tw:text-primary")}>
                  {show ? (isToday ? "Today" : monthly ? formatMonth(bar.bucket.startLocalDate) : formatDay(bar.bucket.startLocalDate)) : ""}
                </span>
              );
            })}
          </div>
        </figure>
      )}
    </Panel>
  );
}

function LegendSwatch({ className, label, value }: { className: string; label: string; value: number }) {
  return (
    <span className="tw:inline-flex tw:items-center tw:gap-1.5">
      <i aria-hidden="true" className={cn("tw:size-2.5 tw:rounded-[2px]", className)} />
      {label} <strong className="tw:font-semibold tw:tabular-nums tw:text-foreground">{value.toLocaleString()}</strong>
    </span>
  );
}
