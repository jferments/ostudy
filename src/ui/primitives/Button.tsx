import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

import { cn } from "../internal/cn";
import { Button as ShadcnButton } from "../vendor/shadcn/button";

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost" | "danger" | "dangerSecondary" | "text" | "back" | "dangerGhost";
  size?: "small" | "medium" | "large" | "icon";
  loading?: boolean;
  children: ReactNode;
}

const variants = {
  primary: "default",
  secondary: "secondary",
  ghost: "ghost",
  danger: "destructive",
  dangerSecondary: "destructive",
  text: "link",
  back: "ghost",
  dangerGhost: "destructive",
} as const;
const sizes = { small: "sm", medium: "default", large: "lg", icon: "icon" } as const;
const semanticVariantClasses: Partial<Record<NonNullable<ButtonProps["variant"]>, string>> = {
  back: "tw:w-fit tw:px-2 tw:text-muted-foreground tw:hover:text-foreground",
  dangerSecondary: "tw:border-destructive/30 tw:bg-background",
  dangerGhost: "tw:bg-transparent tw:hover:bg-destructive-soft",
};

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
      className={cn(semanticVariantClasses[variant], className)}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
    >
      {/* The label slot adds no box while idle, so an icon and its text are
          direct flex items of the button: centered, and spaced by its gap.
          While busy it keeps their layout and the accessible name but hides them. */}
      <span
        data-slot="button-label"
        className={loading
          ? "tw:inline-flex tw:items-center tw:justify-center tw:gap-[inherit] tw:opacity-0"
          : "tw:contents"}
      >
        {children}
      </span>
      {loading ? (
        <span aria-hidden="true" className="tw:absolute tw:inset-0 tw:grid tw:place-items-center tw:text-caption tw:font-medium">
          Working…
        </span>
      ) : null}
    </ShadcnButton>
  );
});
