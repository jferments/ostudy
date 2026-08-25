import type { HTMLAttributes, ReactNode } from "react";

import styles from "./shared.module.css";

interface PanelProps extends HTMLAttributes<HTMLElement> {
  as?: "section" | "article" | "div" | "aside";
  title?: string;
  description?: string;
  actions?: ReactNode;
  children: ReactNode;
}

export function Panel({
  as: Element = "section",
  title,
  description,
  actions,
  children,
  className = "",
  ...props
}: PanelProps) {
  return (
    <Element {...props} className={`${styles.panel} ${className}`.trim()}>
      {title || description || actions ? (
        <header className={styles.panelHeader}>
          <div>
            {title ? <h2>{title}</h2> : null}
            {description ? <p>{description}</p> : null}
          </div>
          {actions ? <div className={styles.panelActions}>{actions}</div> : null}
        </header>
      ) : null}
      {children}
    </Element>
  );
}
