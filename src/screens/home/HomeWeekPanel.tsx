import { useEffect, useState } from "react";

import {
  getProgressActivity,
  getProgressOverview,
  type ProgressActivity,
  type ProgressOverview,
  type ProgressQuery,
} from "../../ipc/progress";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { Panel } from "../../ui/primitives/Panel";

function weekQuery(): ProgressQuery {
  return { range: "7d", subjectIds: [], deckIds: [], tagIds: [] };
}

interface WeekState {
  overview: ProgressOverview;
  activity: ProgressActivity;
}

/**
 * The last seven days at a glance: streak, how much was remembered, reviews and
 * study time, with a bar per day. Progress has the detail. Hidden until the
 * first review, and when it cannot load.
 */
export function HomeWeekPanel({ refreshKey, onOpenProgress }: { refreshKey: number; onOpenProgress: () => void }) {
  const [week, setWeek] = useState<WeekState | null>(null);

  useEffect(() => {
    let current = true;
    void Promise.all([getProgressOverview(weekQuery()), getProgressActivity()])
      .then(([overview, activity]) => {
        if (current) setWeek({ overview, activity });
      })
      // The week is a glance; Progress reports its own errors.
      .catch(() => undefined);
    return () => {
      current = false;
    };
  }, [refreshKey]);

  if (!week || week.activity.days.length === 0) return null;
  const { summary, timeline } = week.overview;
  const remembered = summary.trueRetention ?? summary.correctRate;
  const streak = week.activity.currentStreak?.length ?? 0;
  const peak = Math.max(1, ...timeline.map((bucket) => bucket.reviewCount));

  return (
    <Panel
      title="This week"
      data-testid="home-week"
      actions={
        <Button variant="text" onClick={onOpenProgress}>
          Progress <Icon name="chevron-right" size={16} />
        </Button>
      }
    >
      <dl className="tw:m-0 tw:grid tw:grid-cols-2 tw:gap-x-3 tw:gap-y-4">
        <Stat label="day streak" value={streak.toLocaleString()} />
        <Stat label="remembered" value={remembered === null ? "–" : `${Math.round(remembered * 100)}%`} />
        <Stat label={summary.reviewCount === 1 ? "review" : "reviews"} value={summary.reviewCount.toLocaleString()} />
        <Stat label="studied" value={formatStudyTime(summary.studyTimeMs)} />
      </dl>
      <div aria-hidden="true" className="tw:flex tw:h-14 tw:items-end tw:gap-1.5">
        {timeline.map((bucket, index) => (
          <span
            key={bucket.startLocalDate}
            title={`${bucket.startLocalDate}: ${bucket.reviewCount.toLocaleString()}`}
            className={cn(
              "tw:flex-1 tw:rounded-xs",
              index === timeline.length - 1 ? "tw:bg-primary" : "tw:bg-accent",
            )}
            style={{ height: `${Math.max(6, (bucket.reviewCount / peak) * 100)}%` }}
          />
        ))}
      </div>
    </Panel>
  );
}

function Stat({ label, value }: { label: string; value: string }) {
  return (
    <div className="tw:flex tw:min-w-0 tw:flex-col-reverse">
      <dt className="tw:text-small tw:text-muted-foreground">{label}</dt>
      <dd className="tw:m-0 tw:text-title tw:leading-tight tw:font-semibold tw:tabular-nums">{value}</dd>
    </div>
  );
}

export function formatStudyTime(ms: number): string {
  const minutes = Math.round(ms / 60_000);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  const rest = minutes % 60;
  return rest === 0 ? `${hours} h` : `${hours} h ${rest}`;
}
