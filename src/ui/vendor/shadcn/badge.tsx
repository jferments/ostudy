// Adapted from shadcn/ui's Base UI Nova badge. See README.md for pinned provenance.
import { mergeProps } from "@base-ui/react/merge-props";
import { useRender } from "@base-ui/react/use-render";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "../../internal/cn";

const badgeVariants = cva(
  "tw:inline-flex tw:h-5 tw:w-fit tw:min-w-0 tw:items-center tw:gap-1 tw:rounded-full tw:border tw:border-solid tw:border-transparent tw:px-2 tw:py-0.5 tw:text-xs tw:font-medium tw:leading-none tw:transition-colors tw:[&_svg]:size-3",
  {
    variants: {
      variant: {
        secondary: "tw:bg-secondary tw:text-secondary-foreground",
        default: "tw:bg-primary tw:text-primary-foreground",
        outline: "tw:border-border tw:bg-transparent tw:text-foreground",
        success: "tw:bg-success-soft tw:text-success",
        warning: "tw:bg-warning-soft tw:text-warning",
        destructive: "tw:bg-destructive-soft tw:text-destructive",
        info: "tw:bg-info-soft tw:text-info",
      },
    },
    defaultVariants: { variant: "secondary" },
  },
);

export function Badge({ className, variant, render, ...props }:
  useRender.ComponentProps<"span"> & VariantProps<typeof badgeVariants>) {
  return useRender({
    defaultTagName: "span",
    props: mergeProps<"span">({ className: cn(badgeVariants({ variant }), className) }, props),
    render,
    state: { slot: "badge" },
  });
}
