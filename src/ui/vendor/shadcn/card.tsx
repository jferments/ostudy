// shadcn/ui Nova-style Card composition with oStudy semantic tokens.
import type { HTMLAttributes } from "react";

import { cn } from "../../internal/cn";

export type CardElement = "section" | "article" | "div" | "aside" | "header" | "nav";
export type CardTitleElement = "h2" | "h3" | "h4" | "div";

export function Card({
  as: Element = "div",
  size = "default",
  className,
  ...props
}: HTMLAttributes<HTMLElement> & { as?: CardElement; size?: "default" | "sm" }) {
  return (
    <Element
      {...props}
      data-slot="card"
      data-size={size}
      className={cn(
        "tw:group/card tw:flex tw:min-w-0 tw:flex-col tw:gap-4 tw:overflow-hidden tw:rounded-xl tw:bg-card tw:py-4 tw:text-sm tw:text-card-foreground tw:ring-1 tw:ring-border",
        size === "sm" && "tw:gap-3 tw:rounded-lg tw:py-3",
        className,
      )}
    />
  );
}

export function CardHeader({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      {...props}
      data-slot="card-header"
      className={cn(
        "tw:grid tw:grid-cols-[minmax(0,1fr)_auto] tw:items-start tw:gap-x-4 tw:gap-y-1 tw:px-4 tw:group-data-[size=sm]/card:px-3",
        className,
      )}
    />
  );
}

/** oStudy section cards retain heading semantics even though Nova owns the visual recipe. */
export function CardTitle({
  as: Element = "h3",
  className,
  ...props
}: HTMLAttributes<HTMLElement> & { as?: CardTitleElement }) {
  return (
    <Element
      {...props}
      data-slot="card-title"
      className={cn(
        "tw:m-0 tw:min-w-0 tw:text-base tw:font-medium tw:leading-snug tw:group-data-[size=sm]/card:text-sm",
        className,
      )}
    />
  );
}

export function CardDescription({ className, ...props }: HTMLAttributes<HTMLParagraphElement>) {
  return (
    <p
      {...props}
      data-slot="card-description"
      className={cn("tw:col-start-1 tw:m-0 tw:text-sm tw:leading-relaxed tw:text-muted-foreground", className)}
    />
  );
}

export function CardAction({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div {...props} data-slot="card-action" className={cn("tw:col-start-2 tw:row-span-2 tw:row-start-1", className)} />;
}

export function CardContent({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div {...props} data-slot="card-content" className={cn("tw:min-w-0 tw:px-4 tw:group-data-[size=sm]/card:px-3", className)} />;
}

export function CardFooter({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      {...props}
      data-slot="card-footer"
      className={cn(
        "tw:-mb-4 tw:mt-auto tw:flex tw:items-center tw:gap-2 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:bg-muted/40 tw:px-4 tw:py-3 tw:group-data-[size=sm]/card:-mb-3 tw:group-data-[size=sm]/card:px-3",
        className,
      )}
    />
  );
}
