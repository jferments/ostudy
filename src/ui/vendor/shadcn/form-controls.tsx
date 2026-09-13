// Adapted from shadcn/ui Base UI checkbox, switch, radio-group, slider,
// textarea and collapsible. Pinned upstream and local changes: README.md.
import type { ComponentProps } from "react";
import { Checkbox as CheckboxPrimitive } from "@base-ui/react/checkbox";
import { Switch as SwitchPrimitive } from "@base-ui/react/switch";
import { Radio as RadioPrimitive } from "@base-ui/react/radio";
import { RadioGroup as RadioGroupPrimitive } from "@base-ui/react/radio-group";
import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { Collapsible } from "@base-ui/react/collapsible";
import { cn } from "../../internal/cn";
import { Icon } from "../../icons/Icon";

type Styled<P> = Omit<P, "className"> & { className?: string };

const focus = "tw:focus-visible:outline-2 tw:focus-visible:outline-offset-2 tw:focus-visible:outline-ring";

export function Checkbox({ className, ...props }: Styled<CheckboxPrimitive.Root.Props>) {
  return (
    <CheckboxPrimitive.Root {...props} data-slot="checkbox" className={cn(
      "tw:relative tw:inline-flex tw:size-5 tw:shrink-0 tw:items-center tw:justify-center tw:rounded-sm tw:border tw:border-solid tw:border-input tw:bg-background tw:text-primary-foreground tw:transition-colors tw:data-checked:border-primary tw:data-checked:bg-primary tw:data-indeterminate:border-primary tw:data-indeterminate:bg-primary tw:data-disabled:cursor-not-allowed tw:data-disabled:opacity-50 tw:coarse:size-6 tw:aria-invalid:border-destructive", focus, className,
    )}>
      <CheckboxPrimitive.Indicator className="tw:grid tw:place-items-center">
        <Icon name={props.indeterminate ? "stop" : "check"} size={14} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}

export function Switch({ className, ...props }: Styled<SwitchPrimitive.Root.Props>) {
  return (
    <SwitchPrimitive.Root {...props} data-slot="switch" className={cn(
      "tw:relative tw:inline-flex tw:h-6 tw:w-11 tw:shrink-0 tw:items-center tw:rounded-full tw:border tw:border-solid tw:border-input tw:bg-muted tw:transition-colors tw:data-checked:border-primary tw:data-checked:bg-primary tw:data-disabled:cursor-not-allowed tw:data-disabled:opacity-50", focus, className,
    )}>
      <SwitchPrimitive.Thumb className="tw:pointer-events-none tw:block tw:size-4 tw:translate-x-1 tw:rounded-full tw:bg-foreground tw:shadow-panel tw:transition-transform tw:data-checked:translate-x-6 tw:data-checked:bg-primary-foreground tw:reduce-motion:transition-none" />
    </SwitchPrimitive.Root>
  );
}

export function RadioGroup({ className, ...props }: Styled<RadioGroupPrimitive.Props>) {
  return <RadioGroupPrimitive {...props} data-slot="radio-group" className={cn("tw:grid tw:gap-3", className)} />;
}
export function RadioGroupItem({ className, ...props }: Styled<RadioPrimitive.Root.Props>) {
  return (
    <RadioPrimitive.Root {...props} data-slot="radio-group-item" className={cn(
      "tw:relative tw:inline-flex tw:size-5 tw:shrink-0 tw:items-center tw:justify-center tw:rounded-full tw:border tw:border-solid tw:border-input tw:bg-background tw:text-primary tw:data-checked:border-primary tw:data-disabled:opacity-50 tw:coarse:size-6", focus, className,
    )}>
      <RadioPrimitive.Indicator className="tw:size-2.5 tw:rounded-full tw:bg-current" />
    </RadioPrimitive.Root>
  );
}

export function Slider({ className, label, ...props }: Styled<SliderPrimitive.Root.Props> & { label: string }) {
  return (
    <SliderPrimitive.Root {...props} data-slot="slider" className={cn("tw:w-full tw:min-w-0", className)}>
      <SliderPrimitive.Control className="tw:relative tw:flex tw:h-11 tw:w-full tw:touch-none tw:items-center tw:select-none tw:data-disabled:opacity-50">
        <SliderPrimitive.Track className="tw:relative tw:h-1.5 tw:grow tw:overflow-hidden tw:rounded-full tw:bg-muted">
          <SliderPrimitive.Indicator className="tw:absolute tw:h-full tw:rounded-full tw:bg-primary" />
        </SliderPrimitive.Track>
        <SliderPrimitive.Thumb aria-label={label} className={cn("tw:block tw:size-5 tw:rounded-full tw:border tw:border-solid tw:border-primary tw:bg-background tw:shadow-panel", focus)} />
      </SliderPrimitive.Control>
    </SliderPrimitive.Root>
  );
}

export function Textarea({ className, ...props }: ComponentProps<"textarea">) {
  return <textarea {...props} data-slot="textarea" className={cn(
    "tw:flex tw:min-h-24 tw:w-full tw:min-w-0 tw:resize-y tw:rounded-md tw:border tw:border-solid tw:border-input tw:bg-background tw:px-3 tw:py-2.5 tw:font-sans tw:text-body tw:font-normal tw:text-foreground tw:placeholder:text-muted-foreground tw:disabled:cursor-not-allowed tw:disabled:opacity-50 tw:aria-invalid:border-destructive", focus, className,
  )} />;
}

export const DisclosureRoot = Collapsible.Root;
export function DisclosureTrigger({ className, children, ...props }: Styled<Collapsible.Trigger.Props>) {
  return <Collapsible.Trigger {...props} data-slot="collapsible-trigger" className={cn(
    "tw:group tw:flex tw:min-h-11 tw:w-full tw:items-center tw:justify-between tw:gap-3 tw:rounded-md tw:border-0 tw:bg-transparent tw:px-3 tw:py-2 tw:text-left tw:text-label tw:font-medium tw:text-foreground tw:hover:bg-muted", focus, className,
  )}>{children}<Icon name="chevron-down" size={16} className="tw:shrink-0 tw:text-muted-foreground tw:transition-transform tw:group-data-open:rotate-180 tw:reduce-motion:transition-none" /></Collapsible.Trigger>;
}
export function DisclosureContent({ className, ...props }: Styled<Collapsible.Panel.Props>) {
  return <Collapsible.Panel {...props} data-slot="collapsible-content" className={cn("tw:px-3 tw:pb-4", className)} />;
}

export function SegmentedRadioGroup({ className, ...props }: Styled<RadioGroupPrimitive.Props>) {
  return <RadioGroupPrimitive {...props} aria-orientation="horizontal" data-slot="segmented-control" className={cn("tw:inline-flex tw:flex-wrap tw:gap-1 tw:rounded-xl tw:border tw:border-solid tw:border-border tw:bg-muted tw:p-1", className)} />;
}
export function SegmentedRadioItem({ className, children, ...props }: Styled<RadioPrimitive.Root.Props>) {
  return <RadioPrimitive.Root {...props} nativeButton render={<button type="button" />} className={cn("tw:min-h-10 tw:rounded-lg tw:border tw:border-solid tw:border-transparent tw:bg-transparent tw:px-3 tw:py-2 tw:text-label tw:font-medium tw:text-muted-foreground tw:data-checked:border-border tw:data-checked:bg-background tw:data-checked:text-foreground tw:data-disabled:opacity-50 tw:coarse:min-h-touch", focus, className)}>{children}</RadioPrimitive.Root>;
}
