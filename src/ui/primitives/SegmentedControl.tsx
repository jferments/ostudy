import type { KeyboardEvent } from "react";
import { SegmentedRadioGroup, SegmentedRadioItem } from "../vendor/shadcn/form-controls";

export interface SegmentedOption<T extends string> { value: T; label: string; }
interface SegmentedControlProps<T extends string> {
  label: string;
  value: T;
  options: ReadonlyArray<SegmentedOption<T>>;
  onChange: (value: T) => void;
  disabled?: boolean;
}

export function SegmentedControl<T extends string>({ label, value, options, onChange, disabled = false }: SegmentedControlProps<T>) {
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
    <SegmentedRadioGroup aria-label={label} value={value} disabled={disabled}
      onKeyDown={handleBoundaryKey}
      onValueChange={(next) => {
        const option = options.find((item) => item.value === next);
        if (option) onChange(option.value);
      }}>
      {options.map((option) => (
        <SegmentedRadioItem key={option.value} value={option.value}>{option.label}</SegmentedRadioItem>
      ))}
    </SegmentedRadioGroup>
  );
}
