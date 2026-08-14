import { useEffect, type HTMLAttributes } from "react";

import { useAppearance } from "../appearance/AppearanceProvider";
import styles from "./Materials.module.css";

export function AmbientBackdrop({ className = "", ...props }: HTMLAttributes<HTMLDivElement>) {
  const appearance = useAppearance();

  useEffect(() => {
    if (appearance.motion !== "full" || !appearance.hoverAvailable || appearance.coarsePointer) return;
    const root = document.documentElement;
    let frame = 0;
    function update(event: PointerEvent) {
      const x = event.clientX / Math.max(1, window.innerWidth) - 0.5;
      const y = event.clientY / Math.max(1, window.innerHeight) - 0.5;
      window.cancelAnimationFrame(frame);
      frame = window.requestAnimationFrame(() => {
        root.style.setProperty("--ambient-pointer-x", x.toFixed(4));
        root.style.setProperty("--ambient-pointer-y", y.toFixed(4));
      });
    }
    window.addEventListener("pointermove", update, { passive: true });
    return () => {
      window.removeEventListener("pointermove", update);
      window.cancelAnimationFrame(frame);
      root.style.removeProperty("--ambient-pointer-x");
      root.style.removeProperty("--ambient-pointer-y");
    };
  }, [appearance.coarsePointer, appearance.hoverAvailable, appearance.motion]);

  return (
    <div {...props} className={`${styles.ambient} ${className}`.trim()} aria-hidden="true">
      <div className={styles.ambientBack} />
      <div className={styles.ambientMiddle} />
      <div className={styles.noise} />
      <div className={styles.vignette} />
    </div>
  );
}
