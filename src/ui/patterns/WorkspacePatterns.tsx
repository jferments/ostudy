import type { HTMLAttributes, ReactNode } from "react";

import { Icon, type IconName } from "../icons/Icon";
import { Sheet } from "../overlays/Dialog";
import { Chip, ChipButton } from "../primitives/Chip";
import { Surface } from "../primitives/Surface";
import styles from "./WorkspacePatterns.module.css";

export function PageScaffold({
  width = "wide",
  className = "",
  children,
  ...props
}: HTMLAttributes<HTMLDivElement> & {
  width?: "reading" | "workspace" | "wide" | "full";
  children: ReactNode;
}) {
  return (
    <div {...props} className={`${styles.page} ${styles[width]} ${className}`.trim()}>
      {children}
    </div>
  );
}

export function PageHeader({
  eyebrow,
  title,
  description,
  icon,
  actions,
  compact = false,
  children,
}: {
  eyebrow?: string;
  title: ReactNode;
  description?: ReactNode;
  icon?: IconName;
  actions?: ReactNode;
  compact?: boolean;
  children?: ReactNode;
}) {
  return (
    <header className={`${styles.pageHeader} ${compact ? styles.compact : ""}`.trim()}>
      <div className={styles.pageTitle}>
        {icon ? (
          <span className={styles.pageIcon}>
            <Icon name={icon} size={compact ? 20 : 24} />
          </span>
        ) : null}
        <div>
          {eyebrow ? <span className={styles.eyebrow}>{eyebrow}</span> : null}
          <h1>{title}</h1>
          {description ? <p>{description}</p> : null}
          {children}
        </div>
      </div>
      {actions ? <div className={styles.headerActions}>{actions}</div> : null}
    </header>
  );
}

export function CommandBar({
  label,
  sticky = false,
  className = "",
  children,
}: {
  label: string;
  sticky?: boolean;
  className?: string;
  children: ReactNode;
}) {
  return (
    <Surface
      as="section"
      level={sticky ? 3 : 1}
      material={sticky ? "chrome" : "soft"}
      className={`${styles.commandBar} ${sticky ? styles.sticky : ""} ${className}`.trim()}
      aria-label={label}
    >
      {children}
    </Surface>
  );
}

export interface ActiveFilter {
  id: string;
  label: string;
}

export function FilterSummary({
  filters,
  onRemove,
  onClear,
}: {
  filters: ReadonlyArray<ActiveFilter>;
  onRemove: (id: string) => void;
  onClear: () => void;
}) {
  if (filters.length === 0) return null;
  return (
    <div className={styles.filterSummary} aria-label="Active filters">
      {filters.map((filter) => (
        <ChipButton
          key={filter.id}
          tone="accent"
          onClick={() => onRemove(filter.id)}
          aria-label={`Remove ${filter.label} filter`}
        >
          {filter.label} <span aria-hidden="true">×</span>
        </ChipButton>
      ))}
      <button type="button" className={styles.clearFilters} onClick={onClear}>
        Clear all
      </button>
    </div>
  );
}

export function AdaptiveSheet({
  open,
  title,
  description,
  onClose,
  children,
}: {
  open: boolean;
  title: string;
  description?: string;
  onClose: () => void;
  children: ReactNode;
}) {
  return open ? (
    <Sheet title={title} description={description} width="medium" onClose={onClose}>
      {children}
    </Sheet>
  ) : null;
}

export function StatPill({
  icon,
  label,
  value,
}: {
  icon?: IconName;
  label: string;
  value: ReactNode;
}) {
  return (
    <Chip className={styles.statPill}>
      {icon ? <Icon name={icon} size={16} /> : null}
      <span>{label}</span>
      <strong>{value}</strong>
    </Chip>
  );
}
