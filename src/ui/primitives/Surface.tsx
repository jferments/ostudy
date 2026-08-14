import type { HTMLAttributes, ReactNode } from "react";

import styles from "./Surface.module.css";

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

export function Surface({
  as: Element = "section",
  level = 1,
  material = "solid",
  tone = "neutral",
  interactive = false,
  selected = false,
  inset = false,
  className = "",
  children,
  ...props
}: SurfaceProps) {
  return (
    <Element
      {...props}
      className={`${styles.surface} ${styles[`level${level}`]} ${styles[material]} ${styles[tone]} ${interactive ? styles.interactive : ""} ${selected ? styles.selected : ""} ${inset ? styles.inset : ""} ${className}`.trim()}
      data-surface-level={level}
      data-surface-material={material}
      data-selected={selected || undefined}
    >
      {children}
    </Element>
  );
}
