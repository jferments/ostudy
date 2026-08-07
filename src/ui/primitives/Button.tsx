import type { ButtonHTMLAttributes, ReactNode } from "react";

import styles from "./Button.module.css";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  size?: "small" | "medium" | "large" | "icon";
  loading?: boolean;
  children: ReactNode;
}

export function Button({
  variant = "secondary",
  size = "medium",
  loading = false,
  disabled,
  className = "",
  children,
  ...props
}: ButtonProps) {
  return (
    <button
      {...props}
      type={props.type ?? "button"}
      className={`${styles.button} ${styles[variant]} ${styles[size]} ${className}`.trim()}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
    >
      <span className={loading ? styles.hiddenLabel : undefined}>{children}</span>
      {loading ? <span className={styles.loadingLabel}>Working…</span> : null}
    </button>
  );
}
