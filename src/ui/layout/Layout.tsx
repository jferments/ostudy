import type { CSSProperties, HTMLAttributes, ReactNode } from "react";

import styles from "./Layout.module.css";

type LayoutElement = "div" | "section" | "article" | "aside" | "header" | "nav" | "main";
type CssVariables = CSSProperties & Record<`--${string}`, string | number>;

interface BaseLayoutProps extends HTMLAttributes<HTMLElement> {
  as?: LayoutElement;
  gap?: string;
  children: ReactNode;
}

export function Stack({ as: Element = "div", gap = "var(--ostudy-space-4)", className = "", style, children, ...props }: BaseLayoutProps) {
  return <Element {...props} className={`${styles.stack} ${className}`.trim()} style={{ ...style, "--layout-gap": gap } as CssVariables}>{children}</Element>;
}

export function Inline({ as: Element = "div", gap = "var(--ostudy-space-3)", className = "", style, children, ...props }: BaseLayoutProps) {
  return <Element {...props} className={`${styles.inline} ${className}`.trim()} style={{ ...style, "--layout-gap": gap } as CssVariables}>{children}</Element>;
}

export function Cluster({ as: Element = "div", gap = "var(--ostudy-space-3)", className = "", style, children, ...props }: BaseLayoutProps) {
  return <Element {...props} className={`${styles.cluster} ${className}`.trim()} style={{ ...style, "--layout-gap": gap } as CssVariables}>{children}</Element>;
}

export function ResponsiveGrid({
  as: Element = "div",
  gap = "var(--ostudy-space-4)",
  minItemWidth = "260px",
  className = "",
  style,
  children,
  ...props
}: BaseLayoutProps & { minItemWidth?: string }) {
  return (
    <Element
      {...props}
      className={`${styles.grid} ${className}`.trim()}
      style={{ ...style, "--layout-gap": gap, "--layout-min-item": minItemWidth } as CssVariables}
    >
      {children}
    </Element>
  );
}

export function ContentContainer({
  as: Element = "div",
  width = "workspace",
  className = "",
  children,
  ...props
}: Omit<BaseLayoutProps, "gap"> & { width?: "reading" | "workspace" | "wide" | "full" }) {
  return <Element {...props} className={`${styles.container} ${styles[width]} ${className}`.trim()}>{children}</Element>;
}

export function SplitPane({
  as: Element = "div",
  gap = "var(--ostudy-space-5)",
  primaryMin = "minmax(0, 1.2fr)",
  secondaryMin = "minmax(280px, 0.8fr)",
  className = "",
  style,
  children,
  ...props
}: BaseLayoutProps & { primaryMin?: string; secondaryMin?: string }) {
  return (
    <Element
      {...props}
      className={`${styles.split} ${className}`.trim()}
      style={{
        ...style,
        "--layout-gap": gap,
        "--layout-primary": primaryMin,
        "--layout-secondary": secondaryMin,
      } as CssVariables}
    >
      {children}
    </Element>
  );
}
