import type { ReactNode } from "react";

import styles from "./shared.module.css";

interface EmptyStateProps {
  title: string;
  description: string;
  icon?: ReactNode;
  actions?: ReactNode;
  compact?: boolean;
}

export function EmptyState({
  title,
  description,
  icon,
  actions,
  compact = false,
}: EmptyStateProps) {
  return (
    <section className={`${styles.emptyState} ${compact ? styles.compact : ""}`.trim()}>
      {icon ? <div className={styles.emptyIcon} aria-hidden="true">{icon}</div> : null}
      <h2>{title}</h2>
      <p>{description}</p>
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
