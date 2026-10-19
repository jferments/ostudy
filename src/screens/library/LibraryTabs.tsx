import type { ReactNode } from "react";

import { LIBRARY_SECTIONS, type LibrarySection } from "../../navigation/appDestinations";
import { cn } from "../../ui/internal/cn";
import { ItemTrigger } from "../../ui/primitives/Item";

const VIEWS: Readonly<Record<LibrarySection, { label: string }>> = {
  decks: { label: "Flashcards" },
  documents: { label: "Documents" },
  "study-guides": { label: "Study guides" },
};

interface LibraryTabsProps {
  view: LibrarySection;
  /** Choosing the open page again returns it to its first view. */
  onSelectView: (view: LibrarySection) => void;
  /** Page-wide actions at the end of the row, such as Import deck. */
  actions?: ReactNode;
  className?: string;
}

/**
 * Library's pages as underline tabs: Flashcards (the Workbench), Documents and
 * Study guides. Underlines keep them apart from the side rail's tiles and from
 * the filters inside each page.
 */
export function LibraryTabs({ view, onSelectView, actions, className }: LibraryTabsProps) {
  return (
    <div
      className={cn(
        "tw:flex tw:min-w-0 tw:items-end tw:justify-between tw:gap-3 tw:border-0 tw:border-b tw:border-solid tw:border-border",
        className,
      )}
    >
      <nav aria-label="Library pages" data-tour-anchor="library.views" className="tw:min-w-0 tw:overflow-x-auto">
        <ul className="tw:m-0 tw:flex tw:list-none tw:gap-5 tw:p-0 tw:compact:gap-4">
          {LIBRARY_SECTIONS.map((section) => {
            const active = section === view;
            return (
              <li key={section} className="tw:shrink-0">
                <ItemTrigger
                  data-testid={`library-view-${section}`}
                  aria-current={active ? "page" : undefined}
                  onClick={() => onSelectView(section)}
                  className={cn(
                    "tw:relative tw:min-h-11 tw:justify-center tw:rounded-none tw:px-0.5 tw:text-body tw:whitespace-nowrap tw:outline-none tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:reduce-motion:transition-none",
                    "tw:after:absolute tw:after:inset-x-0 tw:after:-bottom-px tw:after:h-0.75 tw:after:rounded-t-sm",
                    active
                      ? "tw:font-semibold tw:text-foreground tw:after:bg-foreground"
                      : "tw:font-medium tw:text-muted-foreground tw:hover:text-foreground",
                  )}
                >
                  {VIEWS[section].label}
                </ItemTrigger>
              </li>
            );
          })}
        </ul>
      </nav>
      {actions ? <div className="tw:flex tw:shrink-0 tw:items-center tw:gap-2 tw:pb-1.5">{actions}</div> : null}
    </div>
  );
}
