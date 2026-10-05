import type { HTMLAttributes, ReactNode } from "react";

import { Card, CardContent } from "./Card";
import { SectionHeading } from "./SectionHeading";

interface PanelProps extends HTMLAttributes<HTMLElement> {
  as?: "section" | "article" | "div" | "aside";
  title?: string;
  description?: string;
  actions?: ReactNode;
  /** 3 by default, for panels under a section or dialog heading; 2 for panels directly under the page title. */
  headingLevel?: 2 | 3 | 4;
  /** Id for the title, so the panel or a control can refer to it. */
  headingId?: string;
  children?: ReactNode;
}

/**
 * A titled section on the card surface; the heading is the shared SectionHeading. The description
 * and actions belong to the title, so they show only when the panel has one.
 */
export function Panel({ as = "section", title, description, actions, headingLevel = 3, headingId, children, className, ...props }: PanelProps) {
  return (
    <Card {...props} as={as} className={className}>
      {title ? (
        <SectionHeading
          className="tw:px-(--card-gutter)"
          title={title}
          description={description}
          actions={actions}
          level={headingLevel}
          id={headingId}
        />
      ) : null}
      {children !== undefined && children !== null && children !== false ? (
        <CardContent className="tw:grid tw:gap-4">{children}</CardContent>
      ) : null}
    </Card>
  );
}
