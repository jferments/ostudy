import { forwardRef, type SelectHTMLAttributes } from "react";

import { NativeSelect as SelectRenderer } from "../vendor/shadcn/native-select";

/** Native options, change events and numeric size; no vendor state in features. */
export const NativeSelect = forwardRef<HTMLSelectElement, SelectHTMLAttributes<HTMLSelectElement>>(
  function NativeSelect(props, ref) {
    return <SelectRenderer {...props} ref={ref} />;
  },
);
