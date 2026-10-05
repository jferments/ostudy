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
  accent: "tw:bg-accent/50 tw:ring-primary/30",
  success: "tw:bg-success-soft tw:ring-success/30",
  warning: "tw:bg-warning-soft tw:ring-warning/30",
  danger: "tw:bg-destructive-soft tw:ring-destructive/30",
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
 * Compatibility surface for product-semantic states.
 * Ordinary containers should prefer Card or Item; elevation levels are intentionally
 * visually restrained so migration-era nesting does not recreate the old heavy UI.
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
        level === 0 && "tw:bg-transparent tw:ring-0",
        level >= 3 && material === "solid" && "tw:shadow-sm",
        interactive && "tw:transition-colors tw:hover:bg-muted/35 tw:focus-within:ring-2 tw:focus-within:ring-ring/50 tw:reduce-motion:transition-none",
        selected && "tw:bg-accent/60 tw:ring-2 tw:ring-primary/50",
        inset && "tw:bg-muted/35 tw:shadow-none",
        className,
      )}
    >
      {children}
    </Card>
  );
}
