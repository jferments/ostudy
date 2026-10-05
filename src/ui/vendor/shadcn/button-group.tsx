// Adapted from shadcn/ui's Nova ButtonGroup composition. See README.md for provenance.
import type { HTMLAttributes } from "react";

import { cn } from "../../internal/cn";

export function ButtonGroup({
  orientation = "horizontal",
  className,
  ...props
}: HTMLAttributes<HTMLDivElement> & { orientation?: "horizontal" | "vertical" }) {
  return (
    <div
      role="group"
      data-slot="button-group"
      data-orientation={orientation}
      className={cn(
        "tw:flex tw:w-fit tw:items-stretch tw:[&>*:focus-visible]:relative tw:[&>*:focus-visible]:z-(--ostudy-layer-focus)",
        orientation === "horizontal"
          ? "tw:[&>[data-slot=button]:not(:first-child)]:rounded-l-none tw:[&>[data-slot=button]:not(:last-child)]:rounded-r-none tw:[&>[data-slot=button]+[data-slot=button]]:-ml-px"
          : "tw:flex-col tw:[&>[data-slot=button]:not(:first-child)]:rounded-t-none tw:[&>[data-slot=button]:not(:last-child)]:rounded-b-none tw:[&>[data-slot=button]+[data-slot=button]]:-mt-px",
        className,
      )}
      {...props}
    />
  );
}

export function ButtonGroupText({ className, ...props }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      data-slot="button-group-text"
      className={cn("tw:flex tw:items-center tw:gap-2 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:bg-muted tw:px-2.5 tw:text-small tw:font-medium tw:[&_svg]:size-4", className)}
      {...props}
    />
  );
}

export function ButtonGroupSeparator({ className, ...props }: HTMLAttributes<HTMLSpanElement>) {
  return <span aria-hidden="true" data-slot="button-group-separator" className={cn("tw:w-px tw:self-stretch tw:bg-border", className)} {...props} />;
}
