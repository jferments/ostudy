// Adapted from shadcn/ui's Nova native-select; see README.md for provenance.
import type { ComponentProps } from "react";

import { Icon } from "../../icons/Icon";
import { cn } from "../../internal/cn";

/** Keep the operating-system picker and native form semantics, including listboxes. */
export function NativeSelect({ className, multiple, size, ...props }: ComponentProps<"select">) {
  const listbox = multiple || (size !== undefined && size > 1);
  return (
    <div className="tw:relative tw:w-full tw:min-w-0" data-slot="native-select-wrapper">
      <select
        {...props}
        multiple={multiple}
        size={size}
        data-slot="native-select"
        className={cn(
          "tw:w-full tw:min-w-0 tw:rounded-lg tw:border tw:border-solid tw:border-input tw:bg-background tw:px-2.5 tw:font-sans tw:text-small tw:font-normal tw:text-foreground tw:outline-none tw:transition-colors tw:disabled:cursor-not-allowed tw:disabled:opacity-50 tw:focus-visible:border-ring tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:aria-invalid:border-destructive tw:aria-invalid:ring-3 tw:aria-invalid:ring-destructive/20 tw:coarse:min-h-touch",
          listbox ? "tw:min-h-20 tw:py-2" : "tw:h-8 tw:appearance-none tw:pe-9",
          className,
        )}
      />
      {!listbox ? (
        <Icon
          name="chevron-down"
          size={14}
          className="tw:pointer-events-none tw:absolute tw:end-2.5 tw:top-1/2 tw:-translate-y-1/2 tw:text-muted-foreground"
          data-slot="native-select-icon"
        />
      ) : null}
    </div>
  );
}
