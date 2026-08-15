import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from "react";

import type { BadgeTone } from "./Badge";
import styles from "./Status.module.css";

interface ChipBaseProps {
  tone?: BadgeTone;
  selected?: boolean;
  children: ReactNode;
}

export function Chip({
  tone = "neutral",
  selected = false,
  className = "",
  children,
  ...props
}: HTMLAttributes<HTMLSpanElement> & ChipBaseProps) {
  return (
    <span
      {...props}
      className={`${styles.chip} ${styles[tone]} ${selected ? styles.selected : ""} ${className}`.trim()}
      data-selected={selected || undefined}
    >
      {children}
    </span>
  );
}

export function ChipButton({
  tone = "neutral",
  selected = false,
  className = "",
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & ChipBaseProps) {
  return (
    <button
      {...props}
      type={props.type ?? "button"}
      className={`${styles.chip} ${styles.chipButton} ${styles[tone]} ${selected ? styles.selected : ""} ${className}`.trim()}
      aria-pressed={selected}
    >
      {children}
    </button>
  );
}
