import { SegmentedRadioGroup, SegmentedRadioItem } from "../vendor/shadcn/form-controls";
export interface SegmentedOption<T extends string> { value: T; label: string; }
interface SegmentedControlProps<T extends string> {
  label: string; value: T; options: ReadonlyArray<SegmentedOption<T>>;
  onChange: (value: T) => void; disabled?: boolean;
}
export function SegmentedControl<T extends string>({ label, value, options, onChange, disabled = false }: SegmentedControlProps<T>) {
  return <SegmentedRadioGroup aria-label={label} value={value} disabled={disabled} onValueChange={(next) => {
    const option = options.find((item) => item.value === next);
    if (option) onChange(option.value);
  }}>{options.map((option) => <SegmentedRadioItem key={option.value} value={option.value}>{option.label}</SegmentedRadioItem>)}</SegmentedRadioGroup>;
}
