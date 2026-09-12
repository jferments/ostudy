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
    <div className={cn("tw:grid tw:min-w-0 tw:gap-2", className)}>
      <div className="tw:flex tw:items-center tw:justify-between tw:gap-3 tw:text-label tw:font-medium">
        <label htmlFor={inputId}>
          {label}
          {required ? <span aria-hidden="true"> *</span> : null}
        </label>
        {helpTopic ? <HelpTrigger topicId={helpTopic} label={`Help for ${label}`} /> : null}
      </div>
      {description ? <span id={descriptionId} className="tw:text-caption tw:leading-relaxed tw:text-muted-foreground">{description}</span> : null}
      {isValidElement(children)
        ? cloneElement(children, {
            id: inputId,
            required: required || children.props.required,
            "aria-describedby": describedBy,
            "aria-invalid": error ? true : children.props["aria-invalid"],
          })
        : children}
      {error ? <span id={errorId} className="tw:text-caption tw:text-destructive" role="alert">{error}</span> : null}
    </div>
  );
}

export function FieldGroup({ label, description, children }: { label: string; description?: string; children: ReactNode }) {
  return (
    <fieldset className="tw:m-0 tw:grid tw:gap-4 tw:rounded-lg tw:border tw:border-solid tw:border-border tw:p-4 tw:[&>legend]:px-2 tw:[&>legend]:text-label tw:[&>legend]:font-medium tw:[&>p]:m-0 tw:[&>p]:text-label tw:[&>p]:text-muted-foreground">
      <legend>{label}</legend>
      {description ? <p>{description}</p> : null}
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
    <form {...props} className={cn("tw:grid tw:gap-5 tw:[&_label]:grid tw:[&_label]:gap-2", className)}>
      {children}
    </form>
  );
}

export function FieldError({ children }: { children: ReactNode }) {
  return (
    <p className="tw:mt-2 tw:text-label tw:text-destructive" role="alert">
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
    <label {...props} className={cn("tw:flex tw:min-h-14 tw:items-center tw:justify-between tw:gap-5 tw:py-3 tw:[&_small]:block tw:[&_small]:text-caption tw:[&_small]:text-muted-foreground", className)}>
      {children}
    </label>
  );
}
