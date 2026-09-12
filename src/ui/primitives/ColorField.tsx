import { forwardRef, type InputHTMLAttributes } from "react";
import { Input } from "../vendor/shadcn/input";
import { cn } from "../internal/cn";

/** Native color picker for authored content, not an application-theme override. */
export const ColorField = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  function ColorField({ className, ...props }, ref) {
    return <Input {...props} ref={ref} type="color" className={cn("tw:h-10 tw:min-h-10 tw:w-12 tw:cursor-pointer tw:p-1 tw:coarse:min-h-touch", className)} />;
  },
);
