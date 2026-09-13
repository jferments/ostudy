import type { HTMLAttributes, ReactNode } from "react";

import { cn } from "../internal/cn";

interface VisuallyHiddenProps extends HTMLAttributes<HTMLElement> {
  as?: "span" | "div";
  children: ReactNode;
}

export function VisuallyHidden({
  as = "span",
  children,
  className = "",
  ...props
}: VisuallyHiddenProps) {
  const combinedClassName = cn("tw:sr-only", className);
  return as === "div" ? (
    <div className={combinedClassName} {...props}>
      {children}
    </div>
  ) : (
    <span className={combinedClassName} {...props}>
      {children}
    </span>
  );
}
