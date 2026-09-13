import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

import { Button as ShadcnButton } from "../vendor/shadcn/button";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger" | "dangerSecondary" | "text" | "back" | "dangerGhost";
  size?: "small" | "medium" | "large" | "icon";
  loading?: boolean;
  children: ReactNode;
}

const variants = {
  primary: "default", secondary: "outline", ghost: "ghost", danger: "destructive",
  dangerSecondary: "destructive-outline", text: "link", back: "back", dangerGhost: "destructive-ghost",
} as const;
const sizes = { small: "sm", medium: "default", large: "lg", icon: "icon" } as const;

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", size = "medium", loading = false, disabled, className, children, type = "button", ...props },
  ref,
) {
  return (
    <ShadcnButton
      {...props}
      ref={ref}
      type={type}
      variant={variants[variant]}
      size={sizes[size]}
      className={className}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
    >
      {/* Opacity preserves both layout and the action's accessible name while busy. */}
      <span className={loading ? "tw:opacity-0" : undefined}>{children}</span>
      {loading ? <span aria-hidden="true" className="tw:absolute tw:inset-0 tw:grid tw:place-items-center">Working…</span> : null}
    </ShadcnButton>
  );
});
