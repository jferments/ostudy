import type { HTMLAttributes, ReactNode } from "react";

import styles from "./VisuallyHidden.module.css";

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
  const combinedClassName = `${styles.root} ${className}`.trim();
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
