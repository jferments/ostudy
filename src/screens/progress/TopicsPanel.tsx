import { useState } from "react";

import type { TagAnalytics } from "../../ipc/analytics";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { Panel } from "../../ui/primitives/Panel";
import { StatDelta } from "../../ui/primitives/StatTile";
import { formatPercent, plural, pointsDeltaFrom } from "./progressFormat";

const SHOWN = 5;

/** Topics with reviews in range, the ones that need attention first, as native ranks them. */
export function reviewedTopics(topics: TagAnalytics[] | null): TagAnalytics[] {
  return (topics ?? []).filter((topic) => topic.reviewCount > 0);
}

interface TopicsPanelProps {
  topics: TagAnalytics[] | null;
  goal: number | null;
  loading: boolean;
  practicingTagId: string | null;
  onPractice: (topic: TagAnalytics) => void;
}

export function TopicsPanel({ topics, goal, loading, practicingTagId, onPractice }: TopicsPanelProps) {
  const [showAll, setShowAll] = useState(false);
  const reviewed = reviewedTopics(topics);
  const shown = showAll ? reviewed : reviewed.slice(0, SHOWN);
  const goalText = goal === null ? "" : ` against your ${formatPercent(goal)} goal`;
  return (
    <Panel
      headingLevel={2}
      title="Needs attention"
      description={reviewed.length === 0 ? "Topics come from your tags." : `Your tags${goalText}, the ones that need attention first`}
      aria-busy={loading || undefined}
      actions={reviewed.length > SHOWN ? (
        <Button variant="text" aria-expanded={showAll} onClick={() => setShowAll((value) => !value)}>
          {showAll ? "Show fewer" : `All ${reviewed.length} topics`}
        </Button>
      ) : undefined}
    >
      {topics === null ? null : reviewed.length === 0 ? (
        <p className="tw:m-0 tw:text-small tw:text-muted-foreground">Tag your cards and review them to see which topics are slipping.</p>
      ) : (
        <ul className="tw:m-0 tw:grid tw:list-none tw:p-0">
          {shown.map((topic) => {
            const delta = pointsDeltaFrom(topic.trendDelta);
            const rate = topic.correctRate ?? 0;
            const below = goal !== null && topic.correctRate !== null && topic.correctRate < goal;
            return (
              <li key={topic.tagId} className="tw:grid tw:min-w-0 tw:grid-cols-[minmax(0,1fr)_auto] tw:items-center tw:gap-x-4 tw:gap-y-2 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:py-3 tw:first:border-t-0 tw:first:pt-0 tw:wide:grid-cols-[minmax(0,1.2fr)_minmax(6rem,1fr)_4.5rem_auto]">
                <div className="tw:min-w-0">
                  <p className="tw:m-0 tw:break-words tw:font-semibold">{topic.tagName}</p>
                  <p className="tw:m-0 tw:text-caption tw:text-muted-foreground">
                    {plural(topic.reviewCount, "review")}{topic.needsMorePracticeCount > 0 ? ` · ${topic.needsMorePracticeCount.toLocaleString()} missed repeatedly` : ""}
                  </p>
                </div>
                <span aria-hidden="true" className="tw:relative tw:col-span-2 tw:row-start-2 tw:h-1.5 tw:rounded-full tw:bg-progress-track tw:wide:col-span-1 tw:wide:row-start-auto">
                  <span className={cn("tw:absolute tw:inset-y-0 tw:left-0 tw:rounded-full", below ? (rate < (goal ?? 0) - 0.15 ? "tw:bg-destructive" : "tw:bg-warning") : "tw:bg-success")}
                    style={{ width: `${rate * 100}%` }} />
                  {goal !== null ? <span className="tw:absolute tw:-top-1 tw:h-3.5 tw:w-0.5 tw:bg-foreground" style={{ left: `${goal * 100}%` }} /> : null}
                </span>
                <p className="tw:m-0 tw:hidden tw:text-right tw:wide:grid tw:wide:justify-items-end tw:wide:gap-0.5">
                  <strong className="tw:tabular-nums">{formatPercent(topic.correctRate)}</strong>
                  {delta ? <StatDelta {...delta} /> : null}
                </p>
                <div className="tw:flex tw:items-center tw:gap-3">
                  <span className="tw:grid tw:justify-items-end tw:gap-0.5 tw:wide:hidden">
                    <strong className="tw:tabular-nums">{formatPercent(topic.correctRate)}</strong>
                    {delta ? <StatDelta {...delta} /> : null}
                  </span>
                  <Button size="small" variant="secondary" disabled={topic.studyableDueCount === 0 || practicingTagId !== null}
                    aria-label={`Practice ${plural(topic.studyableDueCount, "due card")} in ${topic.tagName}`} onClick={() => onPractice(topic)}>
                    {practicingTagId === topic.tagId ? "Starting…" : topic.studyableDueCount === 0 ? "None due" : `Practice ${topic.studyableDueCount.toLocaleString()}`}
                  </Button>
                </div>
              </li>
            );
          })}
        </ul>
      )}
      {reviewed.length > 0 && goal !== null ? (
        <p className="tw:m-0 tw:flex tw:items-center tw:gap-2 tw:text-caption tw:text-muted-foreground">
          <i aria-hidden="true" className="tw:h-3 tw:w-0.5 tw:bg-foreground" />The tick is your goal. Remembered counts every review of the topic in this range.
        </p>
      ) : null}
    </Panel>
  );
}
