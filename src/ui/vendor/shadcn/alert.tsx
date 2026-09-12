// Adapted from shadcn/ui Alert; provenance and license in README.md.
import type { ComponentProps } from "react";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "../../internal/cn";
const alertVariants = cva("tw:relative tw:w-full tw:rounded-lg tw:border tw:border-solid tw:p-4 tw:text-sm tw:leading-relaxed", { variants: { variant: {
  info: "tw:border-border tw:bg-card tw:text-foreground",
  success: "tw:border-success/30 tw:bg-success-soft tw:text-foreground",
  warning: "tw:border-warning/30 tw:bg-warning-soft tw:text-foreground",
  error: "tw:border-destructive/40 tw:bg-destructive-soft tw:text-destructive",
} }, defaultVariants: { variant: "info" } });
export function Alert({ className, variant, ...props }: ComponentProps<"div"> & VariantProps<typeof alertVariants>) {
  return <div data-slot="alert" role="alert" className={cn(alertVariants({ variant }), className)} {...props} />;
}
export function AlertTitle({ className, ...props }: ComponentProps<"h3">) {
  return <h3 data-slot="alert-title" className={cn("tw:mt-0 tw:mb-1 tw:text-base tw:font-semibold tw:tracking-tight", className)} {...props} />;
}
export function AlertDescription({ className, ...props }: ComponentProps<"div">) {
  return <div data-slot="alert-description" className={cn("tw:min-w-0 tw:break-words tw:[&_p]:m-0 tw:[&_a]:underline tw:[&_a]:underline-offset-4", className)} {...props} />;
}
