import {
  cloneElement,
  isValidElement,
  type ReactElement,
  type ReactNode,
  useId,
} from "react";

import styles from "./shared.module.css";

interface FormFieldProps {
  label: string;
  description?: string;
  error?: string | null;
  required?: boolean;
  children: ReactElement<{
    id?: string;
    "aria-describedby"?: string;
    "aria-invalid"?: boolean;
    required?: boolean;
  }>;
  className?: string;
}

/** Associates labels, descriptions, errors, and validation state consistently. */
export function FormField({
  label,
  description,
  error,
  required = false,
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
        {label}
        {required ? <span aria-hidden="true"> *</span> : null}
      </span>
      {description ? (
        <span id={descriptionId} className={styles.description}>
          {description}
        </span>
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
        <span id={errorId} className={styles.error} role="alert">
          {error}
        </span>
      ) : null}
    </label>
  );
}

export function FieldGroup({
  label,
  description,
  children,
}: {
  label: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <fieldset className={styles.fieldGroup}>
      <legend>{label}</legend>
      {description ? <p>{description}</p> : null}
      {children}
    </fieldset>
  );
}
