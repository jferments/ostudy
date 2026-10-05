import type { CardStrength } from "../../ipc/progress";
import { cn } from "../../ui/internal/cn";
import { Panel } from "../../ui/primitives/Panel";
import { formatPercent } from "./progressFormat";

export function strengthTotal(strength: CardStrength): number {
  return strength.newCount + strength.learningCount + strength.gettingThereCount + strength.solidCount;
}

/** "three weeks" for 21 days; the solid threshold comes from native. */
export function stabilityText(days: number): string {
  if (days % 7 === 0) {
    const weeks = days / 7;
    return weeks === 1 ? "a week" : `${["", "one", "two", "three", "four", "five", "six"][weeks] ?? weeks} weeks`;
  }
  return `${days} days`;
}

/** Card strength's colors, strongest first; shared with the subjects table. */
export const STRENGTH_COLORS = {
  solid: "tw:bg-chart-1",
  gettingThere: "tw:bg-chart-1/40",
  learning: "tw:bg-warning",
  new: "tw:bg-progress-track",
} as const;

export function StrengthBar({ strength, className }: { strength: CardStrength; className?: string }) {
  const segments = [
    [strength.solidCount, STRENGTH_COLORS.solid],
    [strength.gettingThereCount, STRENGTH_COLORS.gettingThere],
    [strength.learningCount, STRENGTH_COLORS.learning],
    [strength.newCount, STRENGTH_COLORS.new],
  ] as const;
  return (
    <span aria-hidden="true" className={cn("tw:flex tw:h-3 tw:w-full tw:gap-0.5 tw:overflow-hidden tw:rounded-sm", className)}>
      {segments.map(([value, color], index) => value > 0 ? <span key={index} className={cn("tw:basis-0", color)} style={{ flexGrow: value }} /> : null)}
    </span>
  );
}

interface StrengthPanelProps {
  strength: CardStrength | null;
  goal: number | null;
  loading: boolean;
}

export function StrengthPanel({ strength, goal, loading }: StrengthPanelProps) {
  const total = strength ? strengthTotal(strength) : 0;
  const rows = strength ? [
    { label: "Solid", detail: `Likely to stick for ${stabilityText(strength.solidStabilityDays)} or more`, value: strength.solidCount, color: STRENGTH_COLORS.solid },
    { label: "Getting there", detail: "Remembered, still being spaced out", value: strength.gettingThereCount, color: STRENGTH_COLORS.gettingThere },
    { label: "Learning", detail: "Seen in the last few days or recently forgotten", value: strength.learningCount, color: STRENGTH_COLORS.learning },
    { label: "New", detail: "Not studied yet", value: strength.newCount, color: STRENGTH_COLORS.new },
  ] : [];
  return (
    <Panel headingLevel={2} title="Card strength" description={strength ? `All ${total.toLocaleString()} cards, by how long they're likely to stick` : undefined} aria-busy={loading || undefined}>
      {strength ? <>
        <StrengthBar strength={strength} className="tw:h-4" />
        <dl className="tw:m-0 tw:grid tw:gap-2.5">
          {rows.map((row) => (
            <div key={row.label} className="tw:grid tw:grid-cols-[auto_minmax(0,1fr)_auto] tw:items-start tw:gap-x-2.5">
              <i aria-hidden="true" className={cn("tw:mt-1 tw:size-2.5 tw:rounded-[2px]", row.color, row.color === STRENGTH_COLORS.new && "tw:ring-1 tw:ring-border-strong")} />
              <dt className="tw:min-w-0 tw:font-semibold">{row.label}<span className="tw:block tw:text-caption tw:font-normal tw:text-muted-foreground">{row.detail}</span></dt>
              <dd className="tw:m-0 tw:font-semibold tw:tabular-nums">{row.value.toLocaleString()}</dd>
            </div>
          ))}
        </dl>
        {strength.estimatedRecall !== null ? (
          <div className="tw:grid tw:grid-cols-[auto_minmax(0,1fr)] tw:items-center tw:gap-4 tw:rounded-md tw:bg-muted/50 tw:p-3">
            <strong className="tw:font-heading tw:text-title tw:tabular-nums">{formatPercent(strength.estimatedRecall)}</strong>
            <p className="tw:m-0 tw:text-small">
              <strong>Estimated recall today.</strong> If you were quizzed on all {strength.estimatedRecallCardCount.toLocaleString()} cards you've studied, you'd likely get about this many right.
              {goal !== null ? ` Your goal is ${formatPercent(goal)}.` : ""}
            </p>
          </div>
        ) : null}
      </> : null}
    </Panel>
  );
}
