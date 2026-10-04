// Adapted from shadcn/ui's Base UI Nova button. See README.md for pinned provenance.
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "../../internal/cn";

export const buttonVariants = cva(
  "tw:relative tw:inline-flex tw:shrink-0 tw:cursor-pointer tw:select-none tw:items-center tw:justify-center tw:whitespace-nowrap tw:rounded-button tw:border tw:border-solid tw:border-transparent tw:bg-clip-padding tw:font-sans tw:text-sm tw:font-medium tw:transition-all tw:duration-(--ostudy-motion-duration-fast) tw:ease-standard tw:outline-none tw:reduce-motion:transition-none tw:disabled:pointer-events-none tw:disabled:opacity-50 tw:focus-visible:border-ring tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:[&_svg]:pointer-events-none tw:[&_svg]:shrink-0 tw:[&_svg:not([class*='size-'])]:size-4",
  {
    variants: {
      variant: {
        default: "tw:bg-primary tw:text-primary-foreground tw:shadow-glow tw:enabled:hover:bg-primary/80",
        outline: "tw:border-border tw:bg-background tw:text-foreground tw:enabled:hover:bg-muted tw:aria-expanded:bg-muted",
        secondary: "tw:border-input tw:bg-secondary tw:text-secondary-foreground tw:enabled:hover:bg-muted tw:aria-expanded:bg-secondary",
        ghost: "tw:bg-transparent tw:text-foreground tw:enabled:hover:bg-muted tw:aria-expanded:bg-muted",
        destructive: "tw:bg-destructive-soft tw:text-destructive tw:enabled:hover:bg-destructive/20 tw:focus-visible:border-destructive/40 tw:focus-visible:ring-destructive/20",
        link: "tw:bg-transparent tw:text-primary tw:underline-offset-4 tw:enabled:hover:underline",
      },
      size: {
        xs: "tw:h-6 tw:gap-1 tw:px-2 tw:text-xs tw:[&_svg:not([class*='size-'])]:size-3",
        sm: "tw:h-7 tw:gap-1 tw:px-2.5 tw:text-[0.8rem] tw:[&_svg:not([class*='size-'])]:size-3.5",
        default: "tw:h-8 tw:gap-1.5 tw:px-2.5",
        lg: "tw:h-9 tw:gap-1.5 tw:px-3",
        icon: "tw:size-8 tw:p-0",
        "icon-sm": "tw:size-7 tw:p-0 tw:[&_svg:not([class*='size-'])]:size-3.5",
        "icon-lg": "tw:size-9 tw:p-0",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

export function Button({
  className,
  variant = "default",
  size = "default",
  ...props
}: Omit<ButtonPrimitive.Props, "className"> & VariantProps<typeof buttonVariants> & { className?: string }) {
  return (
    <ButtonPrimitive
      data-slot="button"
      {...props}
      className={cn(
        buttonVariants({ variant, size }),
        "tw:coarse:min-h-touch tw:coarse:min-w-touch",
        className,
      )}
    />
  );
}
