import type { HTMLAttributes, ReactNode } from "react";
import { Surface } from "../primitives/Surface";
import { cn } from "../internal/cn";

export interface MetricTileProps extends Omit<HTMLAttributes<HTMLElement>, "children"> {
  label: string; value: ReactNode; detail?: ReactNode;
  tone?: "neutral" | "accent" | "success" | "warning" | "danger"; compact?: boolean;
}
const accents = {
  neutral: "tw:border-border", accent: "tw:border-primary/40", success: "tw:border-success/40",
  warning: "tw:border-warning/40", danger: "tw:border-destructive/40",
};
export function MetricTile({ label, value, detail, tone = "neutral", compact = false, className, ...props }: MetricTileProps) {
  return <Surface {...props} as="article" level={compact ? 1 : 2} material={compact ? "soft" : "solid"}
    className={cn("tw:grid tw:min-w-0 tw:content-start tw:gap-3 tw:overflow-hidden", compact ? "tw:p-4" : "tw:p-5", accents[tone], className)} data-tone={tone}>
    <span className="tw:text-sm tw:font-medium tw:text-muted-foreground">{label}</span>
    <strong className={cn("tw:break-words tw:font-semibold tw:leading-none tw:tracking-tight tw:tabular-nums tw:text-foreground", compact ? "tw:text-2xl" : "tw:text-3xl")}>{value}</strong>
    {detail ? <small className="tw:text-xs tw:leading-relaxed tw:text-muted-foreground">{detail}</small> : null}
  </Surface>;
}
export function MetricGrid({ children, compact = false, className, ...props }: HTMLAttributes<HTMLDivElement> & { compact?: boolean }) {
  return <div {...props} className={cn("tw:grid tw:min-w-0 tw:grid-cols-2 tw:gap-4", compact ? "tw:lg:grid-cols-[repeat(auto-fit,minmax(9rem,1fr))]" : "tw:lg:grid-cols-[repeat(auto-fit,minmax(11rem,1fr))]", className)}>{children}</div>;
}
export interface DistributionSegment { id: string; label: string; value: number; tone: "danger" | "warning" | "accent" | "success"; }
const colors = { danger: "tw:bg-destructive", warning: "tw:bg-warning", accent: "tw:bg-primary", success: "tw:bg-success" };
export function DistributionBar({ label, segments }: { label: string; segments: readonly DistributionSegment[] }) {
  const total = segments.reduce((sum, segment) => sum + segment.value, 0);
  return <figure className="tw:m-0 tw:grid tw:min-w-0 tw:gap-4" aria-label={label}>
    <div className="tw:flex tw:h-3 tw:w-full tw:overflow-hidden tw:rounded-full tw:bg-progress-track" aria-hidden="true">
      {segments.map((segment) => <span key={segment.id} className={cn("tw:min-w-0 tw:basis-0", colors[segment.tone])}
        data-tone={segment.tone} style={{ flexGrow: total === 0 ? 0 : segment.value }} />)}
    </div>
    <figcaption className="tw:grid tw:min-w-0 tw:grid-cols-2 tw:gap-x-5 tw:gap-y-3 tw:text-sm tw:text-muted-foreground tw:sm:flex tw:sm:flex-wrap">
      {segments.map((segment) => <span key={segment.id} className="tw:inline-flex tw:min-w-0 tw:items-center tw:gap-2">
        <i className={cn("tw:size-2.5 tw:shrink-0 tw:rounded-full", colors[segment.tone])} data-tone={segment.tone} aria-hidden="true" />
        <span>{segment.label}</span><strong className="tw:font-semibold tw:tabular-nums tw:text-foreground">{segment.value}</strong>
      </span>)}
    </figcaption>
  </figure>;
}
