// Adapted from shadcn/ui Empty; see README.md.
import type { ComponentProps } from "react";
import { cn } from "../../internal/cn";
export function Empty({ className, ...props }: ComponentProps<"section">) {
  return <section data-slot="empty" className={cn("tw:flex tw:w-full tw:min-w-0 tw:flex-col tw:items-center tw:justify-center tw:gap-6 tw:rounded-xl tw:border tw:border-dashed tw:border-border tw:bg-card tw:px-6 tw:py-12 tw:text-center", className)} {...props} />;
}
export function EmptyHeader({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="empty-header" className={cn("tw:flex tw:max-w-lg tw:flex-col tw:items-center tw:gap-3", className)} {...props} />;
}
export function EmptyMedia({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="empty-icon" className={cn("tw:flex tw:size-14 tw:shrink-0 tw:items-center tw:justify-center tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-muted tw:text-accent-foreground tw:[&_svg]:shrink-0", className)} {...props} />;
}
export function EmptyTitle({ className, ...props }: ComponentProps<"h2">) {
  return <h2 data-slot="empty-title" className={cn("tw:m-0 tw:text-lg tw:font-semibold tw:tracking-tight tw:text-foreground", className)} {...props} />;
}
export function EmptyDescription({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="empty-description" className={cn("tw:text-sm tw:leading-relaxed tw:text-muted-foreground tw:[&_a]:underline", className)} {...props} />;
}
export function EmptyContent({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="empty-content" className={cn("tw:flex tw:w-full tw:min-w-0 tw:flex-wrap tw:items-center tw:justify-center tw:gap-3", className)} {...props} />;
}
