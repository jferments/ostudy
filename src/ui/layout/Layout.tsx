import type { CSSProperties, HTMLAttributes, ReactNode } from "react";

import { cn } from "../internal/cn";

type LayoutElement = "div" | "section" | "article" | "aside" | "header" | "nav" | "main";
type CssVariables = CSSProperties & Record<`--${string}`, string | number>;

interface BaseLayoutProps extends HTMLAttributes<HTMLElement> {
  as?: LayoutElement;
  gap?: string;
  children: ReactNode;
}

export function Stack({ as: Element = "div", gap = "var(--ostudy-space-4)", className = "", style, children, ...props }: BaseLayoutProps) {
  return <Element {...props} className={cn("tw:flex tw:min-w-0 tw:flex-col tw:gap-(--layout-gap)", className)} style={{ ...style, "--layout-gap": gap } as CssVariables}>{children}</Element>;
}

export function Inline({ as: Element = "div", gap = "var(--ostudy-space-3)", className = "", style, children, ...props }: BaseLayoutProps) {
  return <Element {...props} className={cn("tw:flex tw:min-w-0 tw:items-center tw:gap-(--layout-gap)", className)} style={{ ...style, "--layout-gap": gap } as CssVariables}>{children}</Element>;
}

export function Cluster({ as: Element = "div", gap = "var(--ostudy-space-3)", className = "", style, children, ...props }: BaseLayoutProps) {
  return <Element {...props} className={cn("tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-(--layout-gap)", className)} style={{ ...style, "--layout-gap": gap } as CssVariables}>{children}</Element>;
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
      className={cn("tw:grid tw:min-w-0 tw:grid-cols-[repeat(auto-fit,minmax(min(100%,var(--layout-min-item)),1fr))] tw:gap-(--layout-gap)", className)}
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
  return <Element {...props} className={cn(
    "tw:mx-auto tw:w-full tw:min-w-0 tw:px-page",
    { reading: "tw:max-w-(--ostudy-layout-reading-width)", workspace: "tw:max-w-(--ostudy-layout-workspace-width)", wide: "tw:max-w-(--ostudy-layout-wide-width)", full: "tw:max-w-none" }[width],
    className,
  )}>{children}</Element>;
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
      className={cn("tw:grid tw:min-w-0 tw:grid-cols-[var(--layout-primary)_var(--layout-secondary)] tw:items-start tw:gap-(--layout-gap) tw:[&>*]:min-w-0 tw:max-[900px]:grid-cols-1", className)}
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
