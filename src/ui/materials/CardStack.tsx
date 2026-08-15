import type { HTMLAttributes, ReactNode } from "react";

import styles from "./Materials.module.css";

export function CardStack({
  remaining,
  className = "",
  children,
  ...props
}: HTMLAttributes<HTMLDivElement> & { remaining: number; children: ReactNode }) {
  const layers = remaining > 3 ? 2 : remaining > 1 ? 1 : 0;
  return (
    <div
      {...props}
      className={`${styles.cardStack} ${className}`.trim()}
      data-stack-layers={layers}
      data-motion-kind="spatial"
    >
      {layers >= 2 ? <span className={`${styles.backingCard} ${styles.backingTwo}`} aria-hidden="true" /> : null}
      {layers >= 1 ? <span className={`${styles.backingCard} ${styles.backingOne}`} aria-hidden="true" /> : null}
      <div className={styles.cardStackContent}>{children}</div>
    </div>
  );
}
