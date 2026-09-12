import { forwardRef, type HTMLAttributes, type ReactNode, type Ref, type TextareaHTMLAttributes } from "react";
import {
  Checkbox as CheckboxView, Switch as SwitchView, RadioGroup as RadioGroupView,
  RadioGroupItem, Slider as SliderView, Textarea as TextareaView,
  DisclosureRoot, DisclosureTrigger, DisclosureContent,
} from "../vendor/shadcn/form-controls";
import { cn } from "../internal/cn";

export interface CheckControlProps extends Omit<HTMLAttributes<HTMLSpanElement>, "onChange" | "defaultChecked"> {
  checked?: boolean;
  defaultChecked?: boolean;
  onCheckedChange?: (checked: boolean) => void;
  disabled?: boolean;
  required?: boolean;
  readOnly?: boolean;
  name?: string;
  value?: string;
  form?: string;
  inputRef?: Ref<HTMLInputElement>;
  indeterminate?: boolean;
}
export const Checkbox = forwardRef<HTMLSpanElement, CheckControlProps>(function Checkbox(props, ref) {
  return <CheckboxView {...props} ref={ref} />;
});
export const Switch = forwardRef<HTMLSpanElement, Omit<CheckControlProps, "indeterminate">>(function Switch(props, ref) {
  return <SwitchView {...props} ref={ref} />;
});
export const Textarea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(function Textarea(props, ref) {
  return <TextareaView {...props} ref={ref} />;
});

export interface RadioOption<T extends string> { value: T; label: ReactNode; ariaLabel?: string; disabled?: boolean; }
export function RadioGroup<T extends string>({ label, value, options, onValueChange, disabled, name, className, presentation = "list" }: {
  label: string; value: T; options: ReadonlyArray<RadioOption<T>>; onValueChange: (value: T) => void;
  disabled?: boolean; name?: string; className?: string; presentation?: "list" | "cards";
}) {
  return <RadioGroupView aria-label={label} value={value} name={name} disabled={disabled} className={cn(presentation === "cards" && "tw:grid tw:grid-cols-2 tw:gap-4 tw:lg:grid-cols-4", className)}
    onValueChange={(next) => { const option = options.find((item) => item.value === next); if (option) onValueChange(option.value); }}>
    {options.map((option) => <label key={option.value} className={cn("tw:flex tw:min-h-11 tw:cursor-pointer tw:items-center tw:gap-3 tw:text-body", presentation === "cards" && "tw:flex-col tw:items-start tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-background tw:p-4", presentation === "cards" && value === option.value && "tw:border-primary tw:bg-accent")}>
      <RadioGroupItem aria-label={option.ariaLabel} value={option.value} disabled={option.disabled} />
      <span>{option.label}</span>
    </label>)}
  </RadioGroupView>;
}

export function Slider({ label, value, min, max, step = 1, disabled, onValueChange }: {
  label: string; value: number; min: number; max: number; step?: number; disabled?: boolean;
  onValueChange: (value: number) => void;
}) {
  return <SliderView label={label} value={value} min={min} max={max} step={step} disabled={disabled}
    onValueChange={(next) => { if (typeof next === "number" && Number.isFinite(next)) onValueChange(next); }} />;
}

/** Presentation-only disclosure. The caller retains the lifetime of async work. */
export function Disclosure({ title, children, className, defaultOpen = false }: {
  title: ReactNode; children: ReactNode; className?: string; defaultOpen?: boolean;
}) {
  return <DisclosureRoot data-slot="collapsible" defaultOpen={defaultOpen} className={cn("tw:min-w-0 tw:rounded-lg tw:border tw:border-solid tw:border-border", className)}>
    <DisclosureTrigger>{title}</DisclosureTrigger>
    <DisclosureContent keepMounted>{children}</DisclosureContent>
  </DisclosureRoot>;
}
