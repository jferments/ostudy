import type { CSSProperties, ReactNode } from "react";
import { cn } from "../internal/cn";
import { ProgressBar } from "../vendor/shadcn/progress";
import styles from "./Progress.module.css";
type ProgressStyle = CSSProperties & Record<`--${string}`, string | number>;
interface ProgressProps {
  value: number; max: number; label: string; className?: string;
  size?: "small" | "medium" | "large"; tone?: "accent" | "success" | "warning" | "danger";
  text?: ReactNode; variant?: "standard" | "inline-content";
  inlineContent?: { start: ReactNode; end: ReactNode }; valueText?: string;
}
export function Progress({ value, max, label, className, size = "medium", tone = "accent", text, variant = "standard", inlineContent, valueText }: ProgressProps) {
  const accessibleMax = Math.max(0, Number.isFinite(max) ? max : 0);
  const accessibleValue = Math.min(Math.max(0, Number.isFinite(value) ? value : 0), accessibleMax);
  const fraction = accessibleMax > 0 ? accessibleValue / accessibleMax : 0;
  const inline = variant === "inline-content" && inlineContent !== undefined;
  const progressStyle = { "--progress-value": fraction, "--progress-percent": `${fraction * 100}%` } as ProgressStyle;
  const contentClass = "tw:pointer-events-none tw:absolute tw:inset-0 tw:grid tw:min-w-0 tw:grid-cols-[minmax(0,1fr)_auto] tw:items-center tw:gap-3 tw:px-3 tw:whitespace-nowrap tw:[&>span]:truncate";
  return <div className={cn("tw:grid tw:min-w-0 tw:gap-2", className)} data-progress-variant={inline ? "inline-content" : "standard"}>
    {text ? <div className="tw:flex tw:min-w-0 tw:items-center tw:justify-between tw:gap-3 tw:text-sm tw:text-muted-foreground">{text}</div> : null}
    <ProgressBar value={accessibleValue} max={accessibleMax} label={label} valueText={valueText} style={progressStyle} size={size} className={styles[tone]} inline={inline}>
      {inline ? <>
        <div className={cn(contentClass, "tw:text-foreground tw:forced-colors:text-[CanvasText]")} data-progress-content-layer="unfilled" aria-hidden="true"><span>{inlineContent.start}</span><span>{inlineContent.end}</span></div>
        <div className={cn(contentClass, "tw:text-(--progress-filled-content-color) tw:[clip-path:inset(0_calc(100%_-_var(--progress-percent))_0_0)] tw:transition-[clip-path] tw:duration-(--ostudy-motion-duration-standard) tw:reduce-motion:transition-none tw:forced-colors:text-[HighlightText]")} data-progress-content-layer="filled" aria-hidden="true"><span>{inlineContent.start}</span><span>{inlineContent.end}</span></div>
      </> : null}
    </ProgressBar>
  </div>;
}
