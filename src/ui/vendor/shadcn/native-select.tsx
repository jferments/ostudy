// Adapted from shadcn/ui's native-select; see README.md for provenance.
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
          "tw:w-full tw:min-w-0 tw:min-h-control tw:rounded-md tw:border tw:border-solid tw:border-input tw:bg-background tw:px-3 tw:py-2 tw:font-sans tw:font-normal tw:text-body tw:text-foreground tw:disabled:cursor-not-allowed tw:disabled:opacity-50 tw:focus-visible:outline-2 tw:focus-visible:outline-offset-2 tw:focus-visible:outline-ring tw:aria-invalid:border-destructive tw:coarse:min-h-touch",
          !listbox && "tw:appearance-none tw:pe-10",
          className,
        )}
      />
      {!listbox ? (
        <Icon
          name="chevron-down"
          size={16}
          className="tw:pointer-events-none tw:absolute tw:end-3 tw:top-1/2 tw:-translate-y-1/2 tw:text-muted-foreground"
          data-slot="native-select-icon"
        />
      ) : null}
    </div>
  );
}
