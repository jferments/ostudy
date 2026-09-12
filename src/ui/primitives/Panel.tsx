import type { HTMLAttributes, ReactNode } from "react";
import { Card, CardHeader, CardContent } from "../vendor/shadcn/card";
import { cn } from "../internal/cn";
interface PanelProps extends HTMLAttributes<HTMLElement> {
  as?: "section" | "article" | "div" | "aside";
  title?: string; description?: string; actions?: ReactNode; children: ReactNode;
}
export function Panel({ as = "section", title, description, actions, children, className, ...props }: PanelProps) {
  return <Card {...props} as={as} className={className}>
    {title || description || actions ? <CardHeader><div className="tw:min-w-0">
      {title ? <h2 className="tw:m-0 tw:text-section tw:font-semibold tw:tracking-tight">{title}</h2> : null}
      {description ? <p className="tw:mt-2 tw:mb-0 tw:text-label tw:font-normal tw:leading-relaxed tw:text-muted-foreground">{description}</p> : null}
    </div>{actions}</CardHeader> : null}
    <CardContent className={cn(!title && !description && !actions && "tw:pt-6")}>{children}</CardContent>
  </Card>;
}
