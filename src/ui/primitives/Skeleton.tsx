import type { CSSProperties, HTMLAttributes } from "react";

import styles from "./Status.module.css";

type SkeletonStyle = CSSProperties & Record<`--${string}`, string | number>;

export function Skeleton({
  width = "100%",
  height = "1rem",
  radius = "var(--ostudy-radius-small)",
  className = "",
  style,
  ...props
}: HTMLAttributes<HTMLSpanElement> & { width?: string; height?: string; radius?: string }) {
  return (
    <span
      {...props}
      aria-hidden="true"
      className={`${styles.skeleton} ${className}`.trim()}
      style={{ ...style, "--skeleton-width": width, "--skeleton-height": height, "--skeleton-radius": radius } as SkeletonStyle}
    />
  );
}
