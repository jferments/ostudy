import type { HTMLAttributes } from "react";

import mascotUrl from "../../assets/images/racoon mascot (logo).png";
import styles from "./Brand.module.css";

export function BrandMark({ className = "", ...props }: HTMLAttributes<HTMLSpanElement>) {
  return (
    <span
      {...props}
      className={`${styles.mark} ${className}`.trim()}
      data-artwork-slot="brand.logo.compact"
      aria-hidden="true"
    >
      <img className={styles.mascot} src={mascotUrl} alt="" draggable={false} />
    </span>
  );
}

export function BrandLockup({ compact = false }: { compact?: boolean }) {
  return (
    <div
      className={`${styles.lockup} ${compact ? styles.compact : ""}`.trim()}
      data-artwork-slot="brand.logo.sidebar"
      aria-label="ostudy"
    >
      <BrandMark />
      <span className={styles.copy}>
        <strong>ostudy</strong>
      </span>
    </div>
  );
}
