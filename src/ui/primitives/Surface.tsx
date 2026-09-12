import type { HTMLAttributes, ReactNode } from "react";
import { Card } from "../vendor/shadcn/card";
import { cn } from "../internal/cn";
export type SurfaceLevel = 0 | 1 | 2 | 3 | 4 | 5;
export type SurfaceMaterial = "solid" | "soft" | "translucent" | "chrome" | "overlay";
export type SurfaceTone = "neutral" | "accent" | "success" | "warning" | "danger";
export interface SurfaceProps extends HTMLAttributes<HTMLElement> {
  as?: "section" | "article" | "div" | "aside" | "header" | "nav";
  level?: SurfaceLevel; material?: SurfaceMaterial; tone?: SurfaceTone;
  interactive?: boolean; selected?: boolean; inset?: boolean; children: ReactNode;
}
const tones = {
  neutral: "", accent: "tw:border-primary tw:bg-accent", success: "tw:border-success tw:bg-success-soft",
  warning: "tw:border-warning tw:bg-warning-soft", danger: "tw:border-destructive tw:bg-destructive-soft",
};
const materials = { solid: "", soft: "tw:bg-muted", translucent: "tw:bg-card", chrome: "tw:bg-sidebar", overlay: "tw:bg-popover tw:shadow-overlay" };
export function Surface({ as = "section", level = 1, material = "solid", tone = "neutral", interactive = false, selected = false, inset = false, className, children, ...props }: SurfaceProps) {
  return <Card {...props} as={as} data-surface-level={level} data-surface-material={material} data-selected={selected || undefined}
    className={cn(materials[material], tones[tone], level === 0 && "tw:border-transparent tw:shadow-none", level >= 3 && "tw:shadow-overlay", interactive && "tw:transition-colors tw:hover:border-border-strong tw:focus-within:border-ring tw:reduce-motion:transition-none", selected && "tw:border-primary tw:bg-accent", inset && "tw:bg-muted tw:shadow-none", className)}>{children}</Card>;
}
