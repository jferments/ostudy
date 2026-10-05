import type { HTMLAttributes, ReactNode } from "react";

import { cn } from "../internal/cn";
import { Icon, type IconName } from "../icons/Icon";
import { ItemTrigger } from "../vendor/shadcn/item";

export type ListRowDensity = "comfortable" | "dense";

export interface ListRowProps extends Omit<HTMLAttributes<HTMLElement>, "title"> {
  as?: "li" | "article" | "div";
  /** "comfortable" for a few rich items, as on the mockups' source lists; "dense" for long lists, as on the Workbench. */
  density?: ListRowDensity;
  /** A ListRowMedia tile, a thumbnail or a status mark before the text. */
  leading?: ReactNode;
  title: ReactNode;
  /** Lines under the title, such as the source, counts or dates. Each child starts its own line. */
  meta?: ReactNode;
  /** Status, figures and actions after the text. They wrap below it when the row is narrow. */
  trailing?: ReactNode;
  /** Makes the leading tile and text one button that opens what the row names. */
  onActivate?: () => void;
  activateDisabled?: boolean;
  /** Ends the button with a chevron, for a row that has no trailing actions of its own. */
  chevron?: boolean;
  selected?: boolean;
}

const densities = {
  comfortable: {
    row: "tw:gap-x-4 tw:gap-y-2 tw:py-4 tw:compact:py-3",
    main: "tw:flex-[1_1_16rem] tw:items-center tw:gap-4 tw:compact:gap-3",
    title: "tw:text-body tw:font-semibold",
    meta: "tw:mt-0.5 tw:text-label tw:text-muted-foreground",
  },
  dense: {
    row: "tw:gap-x-3 tw:gap-y-1 tw:py-2",
    main: "tw:flex-[1_1_12rem] tw:items-start tw:gap-3",
    title: "tw:text-small tw:font-medium",
    meta: "tw:text-caption tw:text-muted-foreground",
  },
} as const;

/**
 * One row of a list: an optional leading tile, a title with meta lines, and trailing status or
 * actions. Rows indent by the surrounding card's gutter, so their text lines up with the card's
 * other content; ListRows draws the dividers and runs the rows to the card's edges.
 */
export function ListRow({
  as: Element = "li",
  density = "comfortable",
  leading,
  title,
  meta,
  trailing,
  onActivate,
  activateDisabled,
  chevron = false,
  selected = false,
  className,
  ...props
}: ListRowProps) {
  const style = densities[density];
  const content = (
    <>
      {leading ? <span data-slot="list-row-leading" className="tw:flex tw:shrink-0">{leading}</span> : null}
      <span data-slot="list-row-text" className="tw:grid tw:min-w-0 tw:flex-1">
        <strong data-slot="list-row-title" className={cn("tw:min-w-0 tw:break-words tw:text-foreground", style.title)}>{title}</strong>
        {meta ? <span data-slot="list-row-meta" className={cn("tw:grid tw:min-w-0 tw:gap-0.5 tw:break-words", style.meta)}>{meta}</span> : null}
      </span>
    </>
  );
  const main = "tw:flex tw:min-w-0";
  return (
    <Element
      {...props}
      data-slot="list-row"
      data-density={density}
      data-selected={selected || undefined}
      className={cn(
        "tw:group/list-row tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:px-[var(--card-gutter,var(--ostudy-space-4))]",
        style.row,
        onActivate && "tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:hover:bg-muted/40 tw:reduce-motion:transition-none",
        selected && "tw:bg-accent/60",
        className,
      )}
    >
      {onActivate ? (
        // The padding, offset by an equal negative margin, keeps the focus ring off the tile and text.
        <ItemTrigger className={cn(main, style.main, "tw:-m-1.5 tw:p-1.5")} disabled={activateDisabled} onClick={onActivate}>
          {content}
          {chevron ? <Icon name="chevron-right" size={18} className="tw:shrink-0 tw:self-center tw:text-muted-foreground" /> : null}
        </ItemTrigger>
      ) : (
        <div data-slot="list-row-main" className={cn(main, style.main)}>{content}</div>
      )}
      {trailing ? <div data-slot="list-row-trailing" className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2">{trailing}</div> : null}
    </Element>
  );
}

/** The mockups' square tile before a row's text: an icon, or a short format label such as PDF. */
export function ListRowMedia({ icon, label, className }: { icon?: IconName; label?: string; className?: string }) {
  return (
    <span
      aria-hidden="true"
      data-slot="list-row-media"
      className={cn(
        "tw:grid tw:h-10.5 tw:min-w-10.5 tw:place-items-center tw:rounded-md tw:bg-muted tw:px-1 tw:text-caption tw:font-semibold tw:uppercase tw:text-muted-foreground",
        "tw:group-data-[density=dense]/list-row:h-8 tw:group-data-[density=dense]/list-row:min-w-8",
        className,
      )}
    >
      {icon ? <Icon name={icon} size={20} /> : label}
    </span>
  );
}

/**
 * A list of rows with a hairline between them. Inside a card the list runs to the card's edges
 * and each row indents by the card's gutter. Use "div" when the rows are articles.
 */
export function ListRows({ as: Element = "ul", className, ...props }: HTMLAttributes<HTMLElement> & { as?: "ul" | "div" }) {
  return (
    <Element
      role={Element === "ul" ? "list" : undefined}
      {...props}
      data-slot="list-rows"
      className={cn(
        "tw:m-0 tw:grid tw:min-w-0 tw:list-none tw:p-0 tw:-mx-[var(--card-gutter,0px)]",
        "tw:[&>*+*]:border-0 tw:[&>*+*]:border-t tw:[&>*+*]:border-solid tw:[&>*+*]:border-border",
        className,
      )}
    />
  );
}
