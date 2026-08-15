import type { HTMLAttributes, ReactNode } from "react";

import { Surface } from "../primitives/Surface";
import styles from "./Metrics.module.css";

export interface MetricTileProps extends Omit<HTMLAttributes<HTMLElement>, "children"> {
  label: string;
  value: ReactNode;
  detail?: ReactNode;
  tone?: "neutral" | "accent" | "success" | "warning" | "danger";
  compact?: boolean;
}

export function MetricTile({
  label,
  value,
  detail,
  tone = "neutral",
  compact = false,
  className = "",
  ...props
}: MetricTileProps) {
  return (
    <Surface
      {...props}
      as="article"
      level={compact ? 1 : 2}
      material={compact ? "soft" : "solid"}
      className={`${styles.tile} ${compact ? styles.compact : ""} ${className}`.trim()}
      data-tone={tone}
    >
      <span className={styles.label}>{label}</span>
      <strong className={styles.value}>{value}</strong>
      {detail ? <small className={styles.detail}>{detail}</small> : null}
    </Surface>
  );
}

export function MetricGrid({
  children,
  compact = false,
  className = "",
  ...props
}: HTMLAttributes<HTMLDivElement> & { compact?: boolean }) {
  return (
    <div
      {...props}
      className={`${styles.grid} ${compact ? styles.compactGrid : ""} ${className}`.trim()}
    >
      {children}
    </div>
  );
}

export interface DistributionSegment {
  id: string;
  label: string;
  value: number;
  tone: "danger" | "warning" | "accent" | "success";
}

export function DistributionBar({
  label,
  segments,
}: {
  label: string;
  segments: readonly DistributionSegment[];
}) {
  const total = segments.reduce((sum, segment) => sum + segment.value, 0);
  return (
    <figure className={styles.distribution} aria-label={label}>
      <div className={styles.track} aria-hidden="true">
        {segments.map((segment) => (
          <span
            key={segment.id}
            className={styles.segment}
            data-tone={segment.tone}
            style={{ flexGrow: total === 0 ? 0 : segment.value }}
          />
        ))}
      </div>
      <figcaption className={styles.legend}>
        {segments.map((segment) => (
          <span key={segment.id}>
            <i data-tone={segment.tone} aria-hidden="true" />
            <span>{segment.label}</span>
            <strong>{segment.value}</strong>
          </span>
        ))}
      </figcaption>
    </figure>
  );
}
