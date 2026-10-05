import type { KeyboardEvent } from "react";

import { SegmentedRadioGroup, SegmentedRadioItem } from "../vendor/shadcn/form-controls";

export interface SegmentedOption<T extends string> { value: T; label: string; disabled?: boolean; }
type Option<T extends string> = SegmentedOption<T>;
/**
 * Two to five short choices. More no longer fit beside a title or across a phone; offer a select or
 * a menu instead.
 */
export type SegmentedOptions<T extends string> =
  | readonly [Option<T>, Option<T>]
  | readonly [Option<T>, Option<T>, Option<T>]
  | readonly [Option<T>, Option<T>, Option<T>, Option<T>]
  | readonly [Option<T>, Option<T>, Option<T>, Option<T>, Option<T>];

interface SegmentedControlProps<T extends string> {
  label: string;
  /** The chosen option, or null while none applies, such as when a custom range is chosen beside it. */
  value: T | null;
  options: SegmentedOptions<T>;
  onChange: (value: T) => void;
  disabled?: boolean;
  className?: string;
}

/**
 * A single choice among a few short options, as a radio group: Tab enters at the chosen option,
 * arrow keys move and choose, Home and End jump to the ends.
 */
export function SegmentedControl<T extends string>({ label, value, options, onChange, disabled = false, className }: SegmentedControlProps<T>) {
  function handleBoundaryKey(event: KeyboardEvent<HTMLDivElement>) {
    // Radio groups intentionally omit Home/End in Base UI. These two shortcuts
    // are part of oStudy's segmented-control API; arrow navigation stays upstream.
    if (disabled || event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey
      || (event.key !== "Home" && event.key !== "End")) return;
    const group = event.currentTarget;
    const buttons = Array.from(group.querySelectorAll<HTMLButtonElement>('button[role="radio"]'))
      .filter((button) => button.closest('[role="radiogroup"]') === group
        && !button.disabled && button.getAttribute("aria-disabled") !== "true");
    const target = event.key === "Home" ? buttons[0] : buttons.at(-1);
    if (!target) return;
    event.preventDefault();
    target.focus();
    target.click();
  }

  return (
    <SegmentedRadioGroup aria-label={label} value={value} disabled={disabled} className={className}
      onKeyDown={handleBoundaryKey}
      onValueChange={(next) => {
        const option = options.find((item) => item.value === next);
        if (option) onChange(option.value);
      }}>
      {options.map((option) => (
        <SegmentedRadioItem key={option.value} value={option.value} disabled={option.disabled}>
          {/* An invisible semibold copy of the label reserves the chosen width, so choosing never nudges the other options. */}
          <span data-label={option.label}
            className="tw:inline-grid tw:min-w-0 tw:grid-cols-[minmax(0,max-content)] tw:after:invisible tw:after:h-0 tw:after:overflow-hidden tw:after:font-semibold tw:after:content-[attr(data-label)]">
            {option.label}
          </span>
        </SegmentedRadioItem>
      ))}
    </SegmentedRadioGroup>
  );
}
