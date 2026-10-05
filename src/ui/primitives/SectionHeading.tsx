import type { ReactNode } from "react";

import { cn } from "../internal/cn";

export interface SectionHeadingProps {
  title: ReactNode;
  description?: ReactNode;
  /** A short uppercase label above the title, such as "Session analytics". */
  eyebrow?: ReactNode;
  /** Controls for the section. They sit beside the title and wrap below it when the row is narrow. */
  actions?: ReactNode;
  /** The level that fits the outline: 2 under the page title, 3 inside a dialog or another section. */
  level?: 2 | 3 | 4;
  /** "default" heads a card or a small section; "large" heads a section that groups several cards. */
  size?: "default" | "large";
  /** Lets a surrounding section name itself with aria-labelledby. */
  id?: string;
  className?: string;
}

const elements = { 2: "h2", 3: "h3", 4: "h4" } as const;

/** The heading for a card or a section of a page, on the type scale, with optional description and actions. */
export function SectionHeading({
  title,
  description,
  eyebrow,
  actions,
  level = 2,
  size = "default",
  id,
  className,
}: SectionHeadingProps) {
  const Heading = elements[level];
  return (
    <div
      data-slot="section-heading"
      className={cn("tw:flex tw:min-w-0 tw:flex-wrap tw:items-start tw:justify-between tw:gap-x-4 tw:gap-y-2.5", className)}
    >
      {/* A description keeps 16rem to read in before the actions wrap below it; a bare title
          keeps short actions, such as a count, beside it whenever both fit on the row. */}
      <div className={cn("tw:min-w-0", description ? "tw:flex-[1_1_16rem]" : "tw:flex-[1_1_auto]")}>
        {eyebrow ? (
          <p className="tw:mt-0 tw:mb-1 tw:text-caption tw:font-medium tw:uppercase tw:tracking-wide tw:text-muted-foreground">{eyebrow}</p>
        ) : null}
        <Heading
          id={id}
          className={cn(
            "tw:m-0 tw:break-words tw:font-semibold tw:tracking-tight tw:text-foreground",
            size === "large" ? "tw:text-section" : "tw:text-title-small",
          )}
        >
          {title}
        </Heading>
        {description ? (
          <p className="tw:mt-0.5 tw:mb-0 tw:max-w-prose tw:text-small tw:text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {actions ? <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2">{actions}</div> : null}
    </div>
  );
}
