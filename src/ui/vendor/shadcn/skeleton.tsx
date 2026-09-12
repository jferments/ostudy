// Adapted from shadcn/ui Skeleton; a span preserves oStudy's existing DOM contract.
import type { ComponentProps } from "react";
import { cn } from "../../internal/cn";

export function Skeleton({ className, ...props }: ComponentProps<"span">) {
  return <span {...props} data-slot="skeleton" className={cn("tw:block tw:rounded-md tw:bg-muted tw:animate-skeleton tw:reduce-motion:animate-none", className)} />;
}
