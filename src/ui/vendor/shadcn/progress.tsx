// Adapted from shadcn/ui Base UI Progress; see README.md.
import type { CSSProperties, ReactNode } from "react";
import { Progress } from "@base-ui/react/progress";
import { cn } from "../../internal/cn";
const heights = { small: "tw:h-1", medium: "tw:h-2", large: "tw:h-3" };
export function ProgressBar({ value, max, label, valueText, style, size, className, inline, children }: {
  value: number | null; max: number; label: string; valueText?: string; style: CSSProperties; size: keyof typeof heights; className?: string; inline: boolean; children?: ReactNode;
}) {
  return <Progress.Root value={value} max={Math.max(1, max)} aria-label={label} aria-valuemin={0} aria-valuemax={max} aria-valuenow={value ?? undefined} aria-valuetext={valueText} style={style} data-slot="progress" className={cn("tw:w-full tw:min-w-0", className)}>
    <Progress.Track data-slot="progress-track" className={cn("tw:relative tw:w-full tw:overflow-hidden tw:rounded-full tw:bg-progress-track tw:forced-colors:border tw:forced-colors:border-[CanvasText] tw:forced-colors:bg-[Canvas]", heights[size], inline && "tw:h-9 tw:border tw:border-solid tw:border-border tw:text-xs tw:font-semibold tw:tabular-nums")}>
      <Progress.Indicator data-indeterminate={value === null ? "" : undefined} aria-hidden="true" data-slot="progress-indicator" className="tw:block tw:h-full tw:data-indeterminate:w-1/3 tw:data-indeterminate:animate-pulse tw:reduce-motion:animate-none tw:rounded-[inherit] tw:bg-(--progress-color) tw:transition-[width] tw:duration-(--ostudy-motion-duration-standard) tw:reduce-motion:transition-none tw:forced-colors:bg-[Highlight]" />
      {children}
    </Progress.Track>
  </Progress.Root>;
}
