import type { HTMLAttributes, ReactNode } from "react";

import { Card, CardAction, CardContent, CardDescription, CardHeader, CardTitle } from "./Card";

interface PanelProps extends HTMLAttributes<HTMLElement> {
  as?: "section" | "article" | "div" | "aside";
  title?: string;
  description?: string;
  actions?: ReactNode;
  children: ReactNode;
}

/** Convenience composition for titled sections; ordinary styling comes from Card. */
export function Panel({ as = "section", title, description, actions, children, className, ...props }: PanelProps) {
  const hasHeader = Boolean(title || description || actions);
  return (
    <Card {...props} as={as} className={className}>
      {hasHeader ? (
        <CardHeader>
          {title ? <CardTitle>{title}</CardTitle> : <span />}
          {description ? <CardDescription>{description}</CardDescription> : null}
          {actions ? <CardAction>{actions}</CardAction> : null}
        </CardHeader>
      ) : null}
      <CardContent className="tw:grid tw:gap-4">{children}</CardContent>
    </Card>
  );
}
