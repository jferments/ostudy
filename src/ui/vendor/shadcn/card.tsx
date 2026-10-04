// shadcn/ui Nova-style Card composition with oStudy semantic tokens.
import type { HTMLAttributes } from "react";

import { cn } from "../../internal/cn";

export type CardElement = "section" | "article" | "div" | "aside" | "header" | "nav";
export type CardTitleElement = "h2" | "h3" | "h4" | "div";

/**
 * The one card surface: the mockups' 20 px radius, a hairline border and the level-1 shadow.
 * `--card-gutter` is the inner padding (20 px, 16 px on phones, 12 px for small cards); the
 * slots read it, so a caller's own padding classes replace it cleanly.
 */
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
        "tw:group/card tw:flex tw:min-w-0 tw:flex-col tw:gap-4 tw:overflow-hidden tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-card tw:py-(--card-gutter) tw:text-small tw:text-card-foreground tw:shadow-sm",
        "tw:[--card-gutter:var(--ostudy-space-5)] tw:compact:[--card-gutter:var(--ostudy-space-4)]",
        // Phones, cards inside cards, and cards in dialogs or popovers sit flat, as in the mockups.
        "tw:compact:shadow-none tw:[[data-slot=card]_&]:shadow-none tw:[[role=dialog]_&]:shadow-none tw:[[role=alertdialog]_&]:shadow-none",
        size === "sm" && "tw:gap-3 tw:rounded-md tw:[--card-gutter:var(--ostudy-space-3)] tw:compact:[--card-gutter:var(--ostudy-space-3)]",
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
        "tw:grid tw:grid-cols-[minmax(0,1fr)_auto] tw:items-start tw:gap-x-4 tw:gap-y-1 tw:px-(--card-gutter)",
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
        "tw:m-0 tw:min-w-0 tw:text-title-small tw:font-semibold tw:tracking-tight tw:group-data-[size=sm]/card:text-body",
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
      className={cn("tw:col-start-1 tw:m-0 tw:text-small tw:leading-relaxed tw:text-muted-foreground", className)}
    />
  );
}

export function CardAction({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div {...props} data-slot="card-action" className={cn("tw:col-start-2 tw:row-span-2 tw:row-start-1", className)} />;
}

export function CardContent({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div {...props} data-slot="card-content" className={cn("tw:min-w-0 tw:px-(--card-gutter)", className)} />;
}

export function CardFooter({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      {...props}
      data-slot="card-footer"
      className={cn(
        "tw:-mb-(--card-gutter) tw:mt-auto tw:flex tw:items-center tw:gap-2 tw:border-0 tw:border-t tw:border-solid tw:border-border tw:bg-muted/40 tw:px-(--card-gutter) tw:py-3",
        className,
      )}
    />
  );
}
