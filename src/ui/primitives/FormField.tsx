import {
  cloneElement,
  isValidElement,
  type FormHTMLAttributes,
  type LabelHTMLAttributes,
  type ReactElement,
  type ReactNode,
  useId,
} from "react";

import { HelpTrigger } from "../help/HelpProvider";
import type { HelpTopicId } from "../help/helpRegistry";
import { cn } from "../internal/cn";

interface FormFieldProps {
  label: string;
  description?: string;
  error?: string | null;
  required?: boolean;
  helpTopic?: HelpTopicId;
  children: ReactElement<{
    id?: string;
    "aria-describedby"?: string;
    "aria-invalid"?: boolean;
    required?: boolean;
  }>;
  className?: string;
}

/** Associates labels, contextual help, descriptions, errors, and validation state. */
export function FormField({
  label,
  description,
  error,
  required = false,
  helpTopic,
  children,
  className = "",
}: FormFieldProps) {
  const generatedId = useId().replaceAll(":", "");
  const inputId = children.props.id ?? `field-${generatedId}`;
  const descriptionId = description ? `${inputId}-description` : undefined;
  const errorId = error ? `${inputId}-error` : undefined;
  const describedBy = [children.props["aria-describedby"], descriptionId, errorId]
    .filter(Boolean)
    .join(" ") || undefined;

  return (
    <div role="group" data-slot="field" className={cn("tw:flex tw:min-w-0 tw:w-full tw:flex-col tw:gap-1.5", className)}>
      <div className="tw:flex tw:min-w-0 tw:items-center tw:justify-between tw:gap-3">
        <label htmlFor={inputId} className="tw:min-w-0 tw:text-small tw:font-medium tw:leading-none tw:text-foreground">
          {label}
          {required ? <span aria-hidden="true"> *</span> : null}
        </label>
        {helpTopic ? <HelpTrigger topicId={helpTopic} label={`Help for ${label}`} /> : null}
      </div>
      {description ? (
        <p id={descriptionId} data-slot="field-description" className="tw:m-0 tw:text-small tw:leading-relaxed tw:text-muted-foreground">
          {description}
        </p>
      ) : null}
      {isValidElement(children)
        ? cloneElement(children, {
            id: inputId,
            required: required || children.props.required,
            "aria-describedby": describedBy,
            "aria-invalid": error ? true : children.props["aria-invalid"],
          })
        : children}
      {error ? (
        <p id={errorId} data-slot="field-error" className="tw:m-0 tw:text-small tw:text-destructive" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}

export function FieldGroup({ label, description, children }: { label: string; description?: string; children: ReactNode }) {
  return (
    <fieldset data-slot="field-set" className="tw:m-0 tw:flex tw:min-w-0 tw:flex-col tw:gap-3 tw:border-0 tw:p-0">
      <legend className="tw:mb-1 tw:px-0 tw:text-small tw:font-medium tw:text-foreground">{label}</legend>
      {description ? <p className="tw:-mt-2 tw:mb-1 tw:text-small tw:leading-relaxed tw:text-muted-foreground">{description}</p> : null}
      {children}
    </fieldset>
  );
}

export function FormStack({
  className = "",
  children,
  ...props
}: FormHTMLAttributes<HTMLFormElement>) {
  return (
    <form {...props} className={cn("tw:flex tw:min-w-0 tw:flex-col tw:gap-5", className)}>
      {children}
    </form>
  );
}

export function FieldError({ children }: { children: ReactNode }) {
  return (
    <p data-slot="field-error" className="tw:mt-1 tw:text-small tw:text-destructive" role="alert">
      {children}
    </p>
  );
}

export function SwitchRow({
  className = "",
  children,
  ...props
}: LabelHTMLAttributes<HTMLLabelElement>) {
  return (
    <label
      {...props}
      className={cn(
        "tw:flex tw:min-h-10 tw:items-center tw:justify-between tw:gap-5 tw:py-2 tw:text-small tw:[&_small]:block tw:[&_small]:mt-0.5 tw:[&_small]:text-small tw:[&_small]:font-normal tw:[&_small]:text-muted-foreground tw:coarse:min-h-touch",
        className,
      )}
    >
      {children}
    </label>
  );
}
