import type { ReactNode } from "react";

import styles from "./shared.module.css";

interface InlineNoticeProps {
  kind?: "info" | "success" | "warning" | "error";
  title?: string;
  children: ReactNode;
  className?: string;
}

export function InlineNotice({
  kind = "info",
  title,
  children,
  className = "",
}: InlineNoticeProps) {
  const urgent = kind === "error";
  return (
    <div
      className={`${styles.notice} ${styles[kind]} ${className}`.trim()}
      role={urgent ? "alert" : "status"}
    >
      {title ? <strong>{title}</strong> : null}
      <div>{children}</div>
    </div>
  );
}

export function InlineProgress({ children }: { children: ReactNode }) {
  return (
    <div className={styles.inlineProgress} role="status" aria-live="polite">
      {children}
    </div>
  );
}
