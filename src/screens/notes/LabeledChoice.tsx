import type { ReactNode } from "react";

/** A few choices with their name above them, as a form field's label sits. */
export function LabeledChoice({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="tw:grid tw:min-w-0 tw:justify-items-start tw:gap-1.5">
      <span aria-hidden="true" className="tw:text-small tw:font-medium tw:leading-none tw:text-foreground">{label}</span>
      {children}
    </div>
  );
}
