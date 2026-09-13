// shadcn/ui Card composition; oStudy permits semantic HTML containers.
import type { HTMLAttributes } from "react";
import { cn } from "../../internal/cn";
export function Card({ as: Element = "div", className, ...props }: HTMLAttributes<HTMLElement> & {
  as?: "section" | "article" | "div" | "aside" | "header" | "nav";
}) {
  return <Element {...props} data-slot="card" className={cn("tw:min-w-0 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-card tw:text-card-foreground tw:shadow-panel", className)} />;
}
export function CardHeader({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div {...props} data-slot="card-header" className={cn("tw:flex tw:items-start tw:justify-between tw:gap-4 tw:px-6 tw:pt-6 tw:pb-4", className)} />;
}
export function CardContent({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div {...props} data-slot="card-content" className={cn("tw:min-w-0 tw:px-6 tw:pb-6", className)} />;
}
