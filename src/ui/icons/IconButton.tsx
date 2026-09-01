import {
  forwardRef,
  type ButtonHTMLAttributes,
} from "react";

import { Tooltip } from "../primitives/Tooltip";
import { Icon, type IconName } from "./Icon";
import styles from "./IconButton.module.css";

export interface IconButtonProps
  extends Omit<
    ButtonHTMLAttributes<HTMLButtonElement>,
    "aria-busy" | "aria-label" | "children" | "title"
  > {
  icon: IconName;
  label: string;
  tooltip?: string;
  tone?: "neutral" | "danger";
  loading?: boolean;
  iconSize?: 16 | 20 | 24 | 32;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(
  function IconButton(
    {
      icon,
      label,
      tooltip,
      tone = "neutral",
      loading = false,
      disabled = false,
      iconSize = 20,
      className = "",
      type = "button",
      ...props
    },
    ref,
  ) {
    const accessibleLabel = label.trim();
    if (accessibleLabel === "") {
      throw new Error("IconButton requires a nonblank accessible label.");
    }
    const tooltipContent = tooltip?.trim() || accessibleLabel;

    return (
      <Tooltip content={tooltipContent}>
        <button
          {...props}
          ref={ref}
          type={type}
          className={`${styles.button} ${styles[tone]} ${className}`.trim()}
          data-tone={tone}
          aria-label={accessibleLabel}
          aria-busy={loading || undefined}
          disabled={disabled || loading}
        >
          <span className={styles.icon} data-loading={loading || undefined}>
            <Icon name={icon} size={iconSize} />
          </span>
          {loading ? (
            <>
              <span className={styles.spinner} aria-hidden="true" />
              <span className={styles.loadingFallback} aria-hidden="true">
                …
              </span>
            </>
          ) : null}
        </button>
      </Tooltip>
    );
  },
);
