import type { HTMLAttributes, ReactNode } from "react";

import { cn } from "../internal/cn";
import { Card } from "../vendor/shadcn/card";

export type SurfaceLevel = 0 | 1 | 2 | 3 | 4 | 5;
export type SurfaceMaterial = "solid" | "soft" | "translucent" | "chrome" | "overlay";
export type SurfaceTone = "neutral" | "accent" | "success" | "warning" | "danger";
export interface SurfaceProps extends HTMLAttributes<HTMLElement> {
  as?: "section" | "article" | "div" | "aside" | "header" | "nav";
  level?: SurfaceLevel;
  material?: SurfaceMaterial;
  tone?: SurfaceTone;
  interactive?: boolean;
  selected?: boolean;
  inset?: boolean;
  children: ReactNode;
}

const tones = {
  neutral: "",
  accent: "tw:border-primary/30 tw:bg-accent/50",
  success: "tw:border-success/30 tw:bg-success-soft",
  warning: "tw:border-warning/30 tw:bg-warning-soft",
  danger: "tw:border-destructive/30 tw:bg-destructive-soft",
};
// Surfaces are flat and opaque in every theme; "translucent" stays accepted but renders solid.
const materials = {
  solid: "",
  soft: "tw:bg-muted/40",
  translucent: "",
  chrome: "tw:bg-sidebar",
  overlay: "tw:bg-popover tw:shadow-lg",
};

/**
 * A Card that carries product state: a tone, selection, hover for clickable surfaces, or an inset well.
 * The surface itself (radius, border, background, shadow, padding) belongs to Card; every level from 1
 * up renders that one card, and level 0 drops the frame for content that only needs the layout.
 */
export function Surface({
  as = "section",
  level = 1,
  material = "solid",
  tone = "neutral",
  interactive = false,
  selected = false,
  inset = false,
  className,
  children,
  ...props
}: SurfaceProps) {
  return (
    <Card
      {...props}
      as={as}
      data-surface-level={level}
      data-surface-material={material}
      data-selected={selected || undefined}
      className={cn(
        materials[material],
        tones[tone],
        level === 0 && "tw:border-transparent tw:bg-transparent tw:shadow-none",
        interactive && "tw:transition-colors tw:hover:border-border-strong tw:hover:bg-muted/35 tw:focus-within:ring-2 tw:focus-within:ring-ring/50 tw:reduce-motion:transition-none",
        selected && "tw:border-primary tw:bg-accent/60",
        inset && "tw:bg-muted/35 tw:shadow-none",
        className,
      )}
    >
      {children}
    </Card>
  );
}
