import type { HTMLAttributes } from "react";

import styles from "./Brand.module.css";

export function BrandMark({ className = "", ...props }: HTMLAttributes<HTMLSpanElement>) {
  return (
    <span {...props} className={`${styles.mark} ${className}`.trim()} data-artwork-slot="brand.logo.compact" aria-hidden="true">
      <span className={styles.cardBack} />
      <span className={styles.cardFront}>O</span>
      <span className={styles.ears} />
    </span>
  );
}

export function BrandLockup({ compact = false }: { compact?: boolean }) {
  return (
    <div className={`${styles.lockup} ${compact ? styles.compact : ""}`.trim()} data-artwork-slot="brand.logo.sidebar" aria-label="ostudy">
      <BrandMark />
      <span className={styles.copy}>
        <strong>ostudy</strong>
        {!compact ? <small>Learn with intent</small> : null}
      </span>
    </div>
  );
}
