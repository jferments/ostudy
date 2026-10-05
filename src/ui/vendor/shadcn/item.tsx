// Adapted from shadcn/ui's Nova Item composition. See README.md for provenance.
import type { HTMLAttributes } from "react";

import { cn } from "../../internal/cn";

const variants = {
  default: "tw:bg-transparent",
  outline: "tw:bg-background tw:ring-1 tw:ring-border",
  muted: "tw:bg-muted/50",
} as const;
const sizes = {
  default: "tw:gap-3 tw:px-3 tw:py-3",
  sm: "tw:gap-2.5 tw:px-2.5 tw:py-2.5",
  xs: "tw:gap-2 tw:px-2 tw:py-2",
} as const;

export function Item({
  as: Element = "div",
  variant = "default",
  size = "default",
  selected = false,
  className,
  ...props
}: HTMLAttributes<HTMLElement> & {
  as?: "div" | "li" | "article" | "section";
  variant?: keyof typeof variants;
  size?: keyof typeof sizes;
  selected?: boolean;
}) {
  return (
    <Element
      {...props}
      data-slot="item"
      data-variant={variant}
      data-size={size}
      data-selected={selected || undefined}
      className={cn(
        "tw:group/item tw:flex tw:w-full tw:min-w-0 tw:flex-wrap tw:items-center tw:rounded-lg tw:outline-none tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:reduce-motion:transition-none tw:focus-within:ring-2 tw:focus-within:ring-ring/50",
        variants[variant],
        sizes[size],
        selected && "tw:bg-accent tw:ring-1 tw:ring-primary/40",
        className,
      )}
    />
  );
}

export function ItemGroup({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div role="list" data-slot="item-group" className={cn("tw:flex tw:w-full tw:flex-col tw:gap-1", className)} {...props} />;
}

export function ItemSeparator({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div role="separator" data-slot="item-separator" className={cn("tw:h-px tw:w-full tw:bg-border", className)} {...props} />;
}

export function ItemMedia({
  variant = "default",
  className,
  ...props
}: HTMLAttributes<HTMLDivElement> & { variant?: "default" | "icon" | "image" }) {
  return (
    <div
      data-slot="item-media"
      data-variant={variant}
      className={cn(
        "tw:flex tw:shrink-0 tw:items-center tw:justify-center tw:[&_svg]:pointer-events-none",
        variant === "icon" && "tw:size-8 tw:rounded-lg tw:bg-muted tw:text-muted-foreground tw:[&_svg]:size-4",
        variant === "image" && "tw:overflow-hidden tw:rounded-lg tw:[&_img]:size-10 tw:[&_img]:object-cover",
        className,
      )}
      {...props}
    />
  );
}

export function ItemContent({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div data-slot="item-content" className={cn("tw:flex tw:min-w-0 tw:flex-1 tw:flex-col tw:gap-0.5", className)} {...props} />;
}

export function ItemTitle({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div data-slot="item-title" className={cn("tw:min-w-0 tw:truncate tw:text-small tw:font-medium tw:text-foreground", className)} {...props} />;
}

export function ItemDescription({ className, ...props }: HTMLAttributes<HTMLParagraphElement>) {
  return <p data-slot="item-description" className={cn("tw:m-0 tw:line-clamp-2 tw:text-small tw:font-normal tw:leading-relaxed tw:text-muted-foreground", className)} {...props} />;
}

export function ItemActions({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div data-slot="item-actions" className={cn("tw:ml-auto tw:flex tw:shrink-0 tw:items-center tw:gap-1", className)} {...props} />;
}

export function ItemHeader({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div data-slot="item-header" className={cn("tw:flex tw:basis-full tw:items-center tw:justify-between tw:gap-3", className)} {...props} />;
}

export function ItemFooter({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return <div data-slot="item-footer" className={cn("tw:flex tw:basis-full tw:items-center tw:justify-between tw:gap-3", className)} {...props} />;
}
