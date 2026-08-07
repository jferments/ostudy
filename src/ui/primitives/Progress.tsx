import styles from "./Progress.module.css";

interface ProgressProps {
  value: number;
  max: number;
  label: string;
  className?: string;
}

export function Progress({ value, max, label, className = "" }: ProgressProps) {
  const boundedMax = Math.max(1, max);
  const boundedValue = Math.min(Math.max(0, value), boundedMax);
  return (
    <div
      className={`${styles.track} ${className}`.trim()}
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuenow={Math.min(Math.max(0, value), max)}
    >
      <span style={{ width: `${(boundedValue / boundedMax) * 100}%` }} />
    </div>
  );
}
