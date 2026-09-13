import { forwardRef, type InputHTMLAttributes } from "react";

import { Input } from "../vendor/shadcn/input";

/** Native field/ref/event semantics; form state and validation stay with callers. */
export const TextField = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  function TextField(props, ref) {
    return <Input {...props} ref={ref} />;
  },
);
