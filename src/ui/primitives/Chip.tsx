import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from "react";
import { Badge, type BadgeTone } from "./Badge";
import { Button } from "./Button";
import { cn } from "../internal/cn";
interface ChipBaseProps { tone?: BadgeTone; selected?: boolean; children: ReactNode; }
const tones: Record<BadgeTone, string> = {
  neutral: "tw:border-border tw:bg-muted tw:text-muted-foreground",
  accent: "tw:border-primary/30 tw:bg-accent tw:text-accent-foreground",
  success: "tw:border-success/30 tw:bg-success-soft tw:text-success",
  warning: "tw:border-warning/30 tw:bg-warning-soft tw:text-warning",
  danger: "tw:border-destructive/30 tw:bg-destructive-soft tw:text-destructive",
  info: "tw:border-info/30 tw:bg-info-soft tw:text-info",
};
export function Chip({ tone = "neutral", selected = false, className, children, ...props }: HTMLAttributes<HTMLSpanElement> & ChipBaseProps) {
  return <Badge {...props} tone={tone} data-selected={selected || undefined} className={cn("tw:px-3 tw:py-1.5 tw:font-medium", selected && "tw:border-primary tw:bg-accent tw:text-accent-foreground", className)}>{children}</Badge>;
}
export function ChipButton({ tone = "neutral", selected = false, className, children, ...props }: ButtonHTMLAttributes<HTMLButtonElement> & ChipBaseProps) {
  return <Button {...props} type={props.type ?? "button"} variant="ghost" size="small" aria-pressed={selected} className={cn("tw:rounded-full tw:px-3 tw:font-medium", tones[tone], selected && "tw:border-primary tw:bg-accent tw:text-accent-foreground", className)}>{children}</Button>;
}
