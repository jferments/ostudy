// Adapted from shadcn/ui's Base UI button. See README.md for pinned provenance.
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "../../internal/cn";

export const buttonVariants = cva(
  "tw:relative tw:inline-flex tw:shrink-0 tw:items-center tw:justify-center tw:gap-2 tw:rounded-button tw:border tw:border-solid tw:font-sans tw:font-semibold tw:cursor-pointer tw:select-none tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:ease-standard tw:reduce-motion:transition-none tw:disabled:cursor-not-allowed tw:disabled:opacity-50 tw:focus-visible:outline-2 tw:focus-visible:outline-offset-2 tw:focus-visible:outline-ring tw:[&_svg]:pointer-events-none tw:[&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default: "tw:border-primary tw:bg-primary tw:text-primary-foreground tw:shadow-panel tw:enabled:hover:bg-primary-hover",
        outline: "tw:border-input tw:bg-secondary tw:text-secondary-foreground tw:shadow-panel tw:enabled:hover:bg-muted",
        ghost: "tw:border-transparent tw:bg-transparent tw:text-accent-foreground tw:enabled:hover:bg-accent",
        destructive: "tw:border-destructive tw:bg-destructive tw:text-destructive-foreground",
        "destructive-outline": "tw:border-destructive tw:bg-secondary tw:text-destructive tw:enabled:hover:bg-destructive-soft",
        link: "tw:border-transparent tw:bg-transparent tw:text-accent-foreground tw:enabled:hover:underline",
        back: "tw:mb-5 tw:border-transparent tw:bg-transparent tw:text-accent-foreground tw:enabled:hover:underline",
        "destructive-ghost": "tw:border-transparent tw:bg-transparent tw:text-destructive tw:enabled:hover:bg-destructive-soft",
      },
      size: {
        default: "tw:min-h-control tw:px-4 tw:py-1 tw:text-label",
        sm: "tw:min-h-8.5 tw:px-3 tw:py-1 tw:text-caption",
        lg: "tw:min-h-13 tw:px-6 tw:py-2 tw:text-body",
        icon: "tw:size-touch tw:min-w-touch tw:p-0",
      },
    },
    defaultVariants: { variant: "outline", size: "default" },
  },
);

export function Button({
  className,
  variant,
  size,
  ...props
}: Omit<ButtonPrimitive.Props, "className"> & VariantProps<typeof buttonVariants> & { className?: string }) {
  return (
    <ButtonPrimitive
      data-slot="button"
      {...props}
      className={cn(buttonVariants({ variant, size }), "tw:coarse:min-h-touch", className)}
    />
  );
}
