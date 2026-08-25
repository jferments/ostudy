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
import styles from "./shared.module.css";

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
    <label className={`${styles.field} ${className}`.trim()} htmlFor={inputId}>
      <span className={styles.label}>
        <span>
          {label}
          {required ? <span aria-hidden="true"> *</span> : null}
        </span>
        {helpTopic ? <HelpTrigger topicId={helpTopic} label={`Help for ${label}`} /> : null}
      </span>
      {description ? <span id={descriptionId} className={styles.description}>{description}</span> : null}
      {isValidElement(children)
        ? cloneElement(children, {
            id: inputId,
            required: required || children.props.required,
            "aria-describedby": describedBy,
            "aria-invalid": error ? true : children.props["aria-invalid"],
          })
        : children}
      {error ? <span id={errorId} className={styles.error} role="alert">{error}</span> : null}
    </label>
  );
}

export function FieldGroup({ label, description, children }: { label: string; description?: string; children: ReactNode }) {
  return (
    <fieldset className={styles.fieldGroup}>
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
    <form {...props} className={`${styles.formStack} ${className}`.trim()}>
      {children}
    </form>
  );
}

export function FieldError({ children }: { children: ReactNode }) {
  return (
    <p className={styles.fieldError} role="alert">
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
    <label {...props} className={`${styles.switchRow} ${className}`.trim()}>
      {children}
    </label>
  );
}
