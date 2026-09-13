// Adapted from shadcn/ui's Base UI badge. See README.md for pinned provenance.
import { mergeProps } from "@base-ui/react/merge-props";
import { useRender } from "@base-ui/react/use-render";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "../../internal/cn";

const badgeVariants = cva(
  "tw:inline-flex tw:w-fit tw:min-w-0 tw:items-center tw:gap-1 tw:rounded-full tw:border tw:border-solid tw:px-2 tw:py-1 tw:text-caption tw:font-semibold tw:leading-tight",
  { variants: { variant: {
    secondary: "tw:border-border tw:bg-muted tw:text-muted-foreground",
    default: "tw:border-primary tw:bg-accent tw:text-accent-foreground",
    success: "tw:border-success tw:bg-success-soft tw:text-success",
    warning: "tw:border-warning tw:bg-warning-soft tw:text-warning",
    destructive: "tw:border-destructive tw:bg-destructive-soft tw:text-destructive",
    info: "tw:border-info tw:bg-info-soft tw:text-info",
  } }, defaultVariants: { variant: "secondary" } },
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
