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
  variant?: "standard" | "inline-content";
  inlineContent?: {
    start: ReactNode;
    end: ReactNode;
  };
  valueText?: string;
}

export function Progress({
  value,
  max,
  label,
  className = "",
  size = "medium",
  tone = "accent",
  text,
  variant = "standard",
  inlineContent,
  valueText,
}: ProgressProps) {
  const finiteMax = Number.isFinite(max) ? max : 0;
  const finiteValue = Number.isFinite(value) ? value : 0;
  const accessibleMax = Math.max(0, finiteMax);
  const accessibleValue = Math.min(Math.max(0, finiteValue), accessibleMax);
  const fraction = accessibleMax > 0 ? accessibleValue / accessibleMax : 0;
  const inline = variant === "inline-content" && inlineContent !== undefined;
  const progressStyle = {
    "--progress-value": fraction,
    "--progress-percent": `${fraction * 100}%`,
  } as ProgressStyle;

  return (
    <div
      className={`${styles.root} ${className}`.trim()}
      data-progress-variant={inline ? "inline-content" : "standard"}
    >
      {text ? <div className={styles.text}>{text}</div> : null}
      <div
        className={`${styles.track} ${styles[size]} ${styles[tone]} ${inline ? styles.inlineTrack : ""}`.trim()}
        role="progressbar"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={accessibleMax}
        aria-valuenow={accessibleValue}
        aria-valuetext={valueText}
        style={progressStyle}
      >
        <span className={styles.fill} aria-hidden="true" />
        {inline ? (
          <>
            <div
              className={`${styles.inlineContentLayer} ${styles.inlineContentUnfilled}`}
              data-progress-content-layer="unfilled"
              aria-hidden="true"
            >
              <span>{inlineContent.start}</span>
              <span>{inlineContent.end}</span>
            </div>
            <div
              className={`${styles.inlineContentLayer} ${styles.inlineContentFilled}`}
              data-progress-content-layer="filled"
              aria-hidden="true"
            >
              <span>{inlineContent.start}</span>
              <span>{inlineContent.end}</span>
            </div>
          </>
        ) : null}
      </div>
    </div>
  );
}
