import type { HTMLAttributes, ReactNode } from "react";

import styles from "./Status.module.css";

export type BadgeTone = "neutral" | "accent" | "success" | "warning" | "danger" | "info";

export function Badge({
  tone = "neutral",
  className = "",
  children,
  ...props
}: HTMLAttributes<HTMLSpanElement> & { tone?: BadgeTone; children: ReactNode }) {
  return (
    <span {...props} className={`${styles.badge} ${styles[tone]} ${className}`.trim()}>
      {children}
    </span>
  );
}
