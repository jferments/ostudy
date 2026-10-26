import type { ReactNode } from "react";

import { SUBJECT_SECTIONS, type SubjectSection } from "../../navigation/appDestinations";
import { cn } from "../../ui/internal/cn";
import { ItemTrigger } from "../../ui/primitives/Item";

/** Phones use the short labels so all five tabs fit. */
const VIEWS: Readonly<Record<SubjectSection, { label: string; short: string }>> = {
  decks: { label: "Flashcards", short: "Cards" },
  "study-guides": { label: "Study guides", short: "Guides" },
  lessons: { label: "Lessons", short: "Lessons" },
  podcasts: { label: "Podcasts", short: "Podcasts" },
  documents: { label: "Documents", short: "Docs" },
};

interface LibraryTabsProps {
  view: SubjectSection;
  /** Choosing the open page again returns it to its first view. */
  onSelectView: (view: SubjectSection) => void;
  /** Page-wide actions at the end of the row, such as Import deck. */
  actions?: ReactNode;
  className?: string;
}

/**
 * A subject's pages as folder tabs: Lessons, Flashcards (the Workbench),
 * Study guides, Podcasts and Documents. The open
 * page's tab is in front, joined to the page below it; the others sit back on
 * a shaded face.
 */
export function LibraryTabs({ view, onSelectView, actions, className }: LibraryTabsProps) {
  return (
    <div
      className={cn(
        "tw:flex tw:min-w-0 tw:items-end tw:justify-between tw:gap-3 tw:border-0 tw:border-b tw:border-solid tw:border-border",
        className,
      )}
    >
      {/* The row overlaps the line below it so the front tab can cover it. */}
      <nav aria-label="Subject pages" data-tour-anchor="library.views" className="tw:-mb-px tw:min-w-0 tw:overflow-x-auto">
        <ul className="tw:m-0 tw:flex tw:list-none tw:items-end tw:gap-1 tw:p-0">
          {SUBJECT_SECTIONS.map((section) => {
            const active = section === view;
            return (
              <li key={section} className="tw:shrink-0">
                <ItemTrigger
                  data-testid={`library-view-${section}`}
                  aria-current={active ? "page" : undefined}
                  aria-label={VIEWS[section].label}
                  onClick={() => onSelectView(section)}
                  className={cn(
                    "tw:justify-center tw:rounded-t-md tw:rounded-b-none tw:border tw:border-b-0 tw:border-solid tw:px-4 tw:text-body tw:whitespace-nowrap tw:compact:px-2.5 tw:compact:text-small tw:outline-none tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:focus-visible:ring-inset tw:reduce-motion:transition-none",
                    active
                      ? "tw:min-h-12 tw:border-border tw:bg-background tw:font-semibold tw:text-foreground"
                      : "tw:mb-px tw:min-h-11 tw:border-border/70 tw:bg-foreground/6 tw:font-medium tw:text-muted-foreground tw:hover:bg-foreground/10 tw:hover:text-foreground",
                  )}
                >
                  <span className="tw:compact:hidden">{VIEWS[section].label}</span>
                  <span className="tw:hidden tw:compact:inline">{VIEWS[section].short}</span>
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
