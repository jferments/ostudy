import type { ReactNode } from "react";

import { LIBRARY_SECTIONS, type LibrarySection } from "../../navigation/appDestinations";
import { Icon, type IconName } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { RouteTransition } from "../../ui/motion/Motion";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { ItemTrigger } from "../../ui/primitives/Item";

const VIEWS: Readonly<Record<LibrarySection, { label: string; icon: IconName }>> = {
  decks: { label: "Decks", icon: "cards" },
  documents: { label: "Documents", icon: "document" },
  "study-guides": { label: "Study guides", icon: "study-guide" },
};

interface LibraryFrameProps {
  view: LibrarySection;
  /** Choosing the open view again returns it to its first page. */
  onSelectView: (view: LibrarySection) => void;
  children: ReactNode;
}

/**
 * Library until the Workbench replaces it: today's Flashcards, Documents and
 * Study guides screens as three views under one compact switcher. The
 * switcher stays put while the view below it changes.
 */
export function LibraryFrame({ view, onSelectView, children }: LibraryFrameProps) {
  const { layoutMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  return (
    // Documents keeps its own width; the switcher lines up with each view.
    <PageScaffold width={view === "documents" ? "wide" : "full"} data-testid="library">
      <nav
        aria-label="Library"
        data-tour-anchor="library.views"
        className={cn("tw:mb-5 tw:compact:mb-4", compact ? "tw:flex" : "tw:inline-flex")}
      >
        <div
          className={cn(
            "tw:gap-1 tw:rounded-full tw:border tw:border-solid tw:border-border tw:bg-card tw:p-1 tw:shadow-sm tw:compact:shadow-none",
            compact ? "tw:grid tw:w-full tw:grid-cols-3" : "tw:flex",
          )}
        >
          {LIBRARY_SECTIONS.map((section) => {
            const active = section === view;
            const item = VIEWS[section];
            return (
              <ItemTrigger
                key={section}
                data-testid={`library-view-${section}`}
                aria-current={active ? "page" : undefined}
                onClick={() => onSelectView(section)}
                className={cn(
                  "tw:justify-center tw:gap-2 tw:rounded-full tw:whitespace-nowrap tw:text-muted-foreground tw:outline-none tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:hover:text-foreground tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:reduce-motion:transition-none",
                  compact ? "tw:h-9 tw:px-2 tw:text-label" : "tw:h-9 tw:px-4 tw:text-small",
                  active && "tw:bg-accent tw:font-semibold tw:text-accent-foreground tw:hover:text-accent-foreground",
                )}
              >
                {compact ? null : <Icon name={item.icon} size={16} className="tw:shrink-0" />}
                <span className="tw:min-w-0 tw:truncate">{item.label}</span>
              </ItemTrigger>
            );
          })}
        </div>
      </nav>
      <RouteTransition routeKey={view} relation="sibling" className="tw:min-w-0">
        {children}
      </RouteTransition>
    </PageScaffold>
  );
}
