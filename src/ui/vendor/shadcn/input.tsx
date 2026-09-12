// Adapted from shadcn/ui's Base UI input; see README.md for upstream provenance.
import type { ComponentProps } from "react";
import { Input as InputPrimitive } from "@base-ui/react/input";

import { cn } from "../../internal/cn";

export function Input({ className, type, ...props }: ComponentProps<"input">) {
  return (
    <InputPrimitive
      {...props}
      type={type}
      data-slot="input"
      className={cn(
        "tw:w-full tw:min-w-0 tw:min-h-control tw:rounded-md tw:border tw:border-solid tw:border-input tw:bg-background tw:px-3 tw:py-2 tw:font-sans tw:font-normal tw:text-body tw:text-foreground tw:placeholder:text-muted-foreground tw:disabled:cursor-not-allowed tw:disabled:opacity-50 tw:focus-visible:outline-2 tw:focus-visible:outline-offset-2 tw:focus-visible:outline-ring tw:aria-invalid:border-destructive tw:coarse:min-h-touch tw:file:mr-3 tw:file:border-0 tw:file:bg-transparent tw:file:text-foreground",
        className,
      )}
    />
  );
}
