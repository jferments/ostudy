import { cn } from "../internal/cn";

/* Stat tiles and their grid live in primitives/StatTile; this pattern owns the session result bar. */

export interface DistributionSegment {
  id: string;
  label: string;
  value: number;
  tone: "danger" | "warning" | "accent" | "success";
}
const colors = {
  danger: "tw:bg-destructive",
  warning: "tw:bg-warning",
  accent: "tw:bg-primary",
  success: "tw:bg-success",
};

export function DistributionBar({ label, segments }: { label: string; segments: readonly DistributionSegment[] }) {
  const total = segments.reduce((sum, segment) => sum + segment.value, 0);
  return (
    <figure className="tw:m-0 tw:grid tw:min-w-0 tw:gap-3" aria-label={label}>
      <div className="tw:flex tw:h-2 tw:w-full tw:overflow-hidden tw:rounded-full tw:bg-progress-track" aria-hidden="true">
        {segments.map((segment) => (
          <span
            key={segment.id}
            className={cn("tw:min-w-0 tw:basis-0", colors[segment.tone])}
            data-tone={segment.tone}
            style={{ flexGrow: total === 0 ? 0 : segment.value }}
          />
        ))}
      </div>
      <figcaption className="tw:grid tw:min-w-0 tw:grid-cols-2 tw:gap-x-5 tw:gap-y-2 tw:text-small tw:text-muted-foreground tw:sm:flex tw:sm:flex-wrap">
        {segments.map((segment) => (
          <span key={segment.id} className="tw:inline-flex tw:min-w-0 tw:items-center tw:gap-2">
            <i className={cn("tw:size-2 tw:shrink-0 tw:rounded-full", colors[segment.tone])} data-tone={segment.tone} aria-hidden="true" />
            <span>{segment.label}</span>
            <strong className="tw:font-medium tw:tabular-nums tw:text-foreground">{segment.value}</strong>
          </span>
        ))}
      </figcaption>
    </figure>
  );
}
