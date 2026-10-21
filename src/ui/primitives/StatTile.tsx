import { useId, type HTMLAttributes, type ReactNode } from "react";

import { Icon } from "../icons/Icon";
import { cn } from "../internal/cn";
import { Card } from "../vendor/shadcn/card";

/** Whether a change is good news. A fall in missed cards is positive, so tone is separate from direction. */
export type StatDeltaTone = "positive" | "negative" | "neutral";

export interface StatDeltaProps {
  /** The change as the learner reads it, such as "3 pts" or "+64 this month". */
  label: string;
  /** Draws an arrow and tells screen readers which way the value moved. */
  direction?: "up" | "down" | "flat";
  tone?: StatDeltaTone;
  className?: string;
}

const deltaTones: Record<StatDeltaTone, string> = {
  positive: "tw:bg-success-soft tw:text-success",
  negative: "tw:bg-destructive-soft tw:text-destructive",
  neutral: "tw:bg-muted tw:text-muted-foreground",
};
const directionIcons = { up: "arrow-up", down: "arrow-down", flat: "arrow-right" } as const;
const directionWords = { up: "up", down: "down", flat: "unchanged" } as const;

/**
 * A change against an earlier period, drawn as a small pill. Color is never the only signal:
 * the arrow shows the direction and a visually hidden word says it.
 */
export function StatDelta({ label, direction, tone = "neutral", className }: StatDeltaProps) {
  return (
    <span
      data-slot="stat-delta"
      data-tone={tone}
      className={cn(
        "tw:inline-flex tw:items-center tw:gap-0.5 tw:whitespace-nowrap tw:rounded-full tw:py-px tw:text-label tw:font-semibold tw:tabular-nums",
        direction ? "tw:pr-1.75 tw:pl-1.25" : "tw:px-2",
        deltaTones[tone],
        className,
      )}
    >
      {direction ? <Icon name={directionIcons[direction]} size={12} className="tw:stroke-3" /> : null}
      {label}
      {direction ? <span className="tw:sr-only"> {directionWords[direction]}</span> : null}
    </span>
  );
}

export type StatTileTone = "neutral" | "accent" | "success" | "warning" | "danger";

export interface StatTileProps extends Omit<HTMLAttributes<HTMLElement>, "children"> {
  label: string;
  /** A small control beside the label, such as a help button. */
  labelAction?: ReactNode;
  value: ReactNode;
  /** A change against an earlier period, shown beside the value. */
  delta?: StatDeltaProps;
  /** One short line under the value that explains it. */
  hint?: ReactNode;
  /** A small chart or meter under the hint, such as a sparkline. */
  children?: ReactNode;
  tone?: StatTileTone;
}

const tileTones: Record<StatTileTone, string> = {
  neutral: "",
  accent: "tw:border-primary/30",
  success: "tw:border-success/30",
  warning: "tw:border-warning/30",
  danger: "tw:border-destructive/30",
};

/**
 * A labeled number on the card surface, as on the mockups' Progress and Home. Phones get the
 * mockups' smaller tile: tighter padding, the medium radius and a lighter label.
 */
export function StatTile({ label, labelAction, value, delta, hint, children, tone = "neutral", className, ...props }: StatTileProps) {
  const labelId = useId();
  return (
    <Card
      {...props}
      as="article"
      aria-labelledby={labelId}
      data-tone={tone}
      className={cn(
        "tw:gap-1 tw:px-5 tw:py-4.5 tw:compact:gap-0.5 tw:compact:rounded-md tw:compact:px-3.5 tw:compact:py-3",
        tileTones[tone],
        className,
      )}
    >
      {labelAction ? (
        <div className="tw:-my-1.5 tw:flex tw:min-w-0 tw:items-center tw:gap-1">
          <p id={labelId} className="tw:m-0 tw:text-small tw:font-medium tw:text-muted-foreground tw:compact:text-label tw:compact:font-normal">
            {label}
          </p>
          {labelAction}
        </div>
      ) : (
        <p id={labelId} className="tw:m-0 tw:text-small tw:font-medium tw:text-muted-foreground tw:compact:text-label tw:compact:font-normal">
          {label}
        </p>
      )}
      <p className="tw:m-0 tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-x-2.5 tw:gap-y-1">
        <span data-slot="stat-value" className="tw:min-w-0 tw:break-words tw:font-heading tw:text-title tw:font-semibold tw:tracking-tighter tw:tabular-nums tw:text-foreground">
          {value}
        </span>
        {delta ? <StatDelta {...delta} /> : null}
      </p>
      {hint ? <p className="tw:m-0 tw:text-small tw:text-(--ostudy-color-text-secondary) tw:compact:text-label">{hint}</p> : null}
      {children ? <div className="tw:mt-2.5 tw:min-w-0">{children}</div> : null}
    </Card>
  );
}

/** Two tiles a row on phones; wider screens fit as many as the row holds. */
export function StatGrid({ children, className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      {...props}
      className={cn(
        "tw:grid tw:min-w-0 tw:grid-cols-2 tw:gap-3 tw:lg:grid-cols-[repeat(auto-fit,minmax(11rem,1fr))]",
        className,
      )}
    >
      {children}
    </div>
  );
}
