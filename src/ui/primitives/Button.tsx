import {
  forwardRef,
  type ButtonHTMLAttributes,
  type ReactNode,
} from "react";

import styles from "./Button.module.css";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?:
    | "primary"
    | "secondary"
    | "ghost"
    | "danger"
    | "dangerSecondary"
    | "text"
    | "back"
    | "dangerGhost";
  size?: "small" | "medium" | "large" | "icon";
  loading?: boolean;
  children: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  {
    variant = "secondary",
    size = "medium",
    loading = false,
    disabled,
    className = "",
    children,
    type = "button",
    ...props
  },
  ref,
) {
  return (
    <button
      {...props}
      ref={ref}
      type={type}
      className={`${styles.button} ${styles[variant]} ${styles[size]} ${className}`.trim()}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
    >
      <span className={loading ? styles.hiddenLabel : undefined}>{children}</span>
      {loading ? <span className={styles.loadingLabel}>Working…</span> : null}
    </button>
  );
});
