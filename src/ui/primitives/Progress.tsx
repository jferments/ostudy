import type { CSSProperties, ReactNode } from "react";

import styles from "./Progress.module.css";

type ProgressStyle = CSSProperties & Record<`--${string}`, string | number>;

interface ProgressProps {
  value: number;
  max: number;
  label: string;
  className?: string;
  size?: "small" | "medium" | "large";
  tone?: "accent" | "success" | "warning" | "danger";
  text?: ReactNode;
}

export function Progress({
  value,
  max,
  label,
  className = "",
  size = "medium",
  tone = "accent",
  text,
}: ProgressProps) {
  const boundedMax = Math.max(1, max);
  const boundedValue = Math.min(Math.max(0, value), boundedMax);
  const fraction = boundedValue / boundedMax;
  return (
    <div className={`${styles.root} ${className}`.trim()}>
      {text ? <div className={styles.text}>{text}</div> : null}
      <div
        className={`${styles.track} ${styles[size]} ${styles[tone]}`}
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={max}
        aria-valuenow={Math.min(Math.max(0, value), max)}
        style={{ "--progress-value": fraction } as ProgressStyle}
      >
        <span />
      </div>
    </div>
  );
}
