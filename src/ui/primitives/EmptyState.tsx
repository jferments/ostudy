import type { ReactNode } from "react";

import styles from "./shared.module.css";

interface EmptyStateProps {
  title?: string;
  description?: ReactNode;
  icon?: ReactNode;
  actions?: ReactNode;
  compact?: boolean;
  large?: boolean;
  className?: string;
}

export function EmptyState({
  title,
  description,
  icon,
  actions,
  compact = false,
  large = false,
  className = "",
}: EmptyStateProps) {
  return (
    <section
      className={`${styles.emptyState} ${compact ? styles.compact : ""} ${large ? styles.large : ""} ${className}`.trim()}
    >
      {icon ? <div className={styles.emptyIcon} aria-hidden="true">{icon}</div> : null}
      {title ? <h2>{title}</h2> : null}
      {description ? <p>{description}</p> : null}
      {actions ? <div className={styles.emptyActions}>{actions}</div> : null}
    </section>
  );
}

export function LoadingState({ label = "Loading…" }: { label?: string }) {
  return (
    <div className={styles.loadingState} role="status" aria-live="polite">
      <span className={styles.spinner} aria-hidden="true" />
      <span>{label}</span>
    </div>
  );
}
