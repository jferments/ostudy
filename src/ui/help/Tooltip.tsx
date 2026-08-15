import {
  cloneElement,
  type ReactElement,
  useId,
} from "react";

import styles from "./Help.module.css";

export function Tooltip({
  content,
  children,
}: {
  content: string;
  children: ReactElement<{ "aria-describedby"?: string }>;
}) {
  const id = `tooltip-${useId().replaceAll(":", "")}`;
  const describedBy = [children.props["aria-describedby"], id].filter(Boolean).join(" ");
  return (
    <span className={styles.tooltipRoot}>
      {cloneElement(children, { "aria-describedby": describedBy })}
      <span id={id} role="tooltip" className={styles.tooltip}>
        {content}
      </span>
    </span>
  );
}
