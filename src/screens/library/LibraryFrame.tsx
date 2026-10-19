import { createContext, useContext, useLayoutEffect, useState, type ReactNode } from "react";

import type { LibrarySection } from "../../navigation/appDestinations";
import { RouteTransition } from "../../ui/motion/Motion";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { LibraryTabs } from "./LibraryTabs";

/** Lets a hosted view hand the whole page to one of its own pages. */
const LibraryPageContext = createContext<((fullPage: boolean) => void) | null>(null);

/**
 * Hands the Library page to a view's own page while `active`, such as an open
 * study guide: the tabs step aside and the frame drops its inset, so the
 * page's sticky top bar starts at the edges of the scroll area, right under
 * the app header. The page's own back button returns to the view. Outside a
 * LibraryFrame it does nothing.
 */
export function useLibraryFullPage(active: boolean): void {
  const setFullPage = useContext(LibraryPageContext);
  useLayoutEffect(() => {
    if (!active || !setFullPage) return;
    setFullPage(true);
    return () => setFullPage(false);
  }, [active, setFullPage]);
}

interface LibraryFrameProps {
  view: LibrarySection;
  /** Choosing the open view again returns it to its first page. */
  onSelectView: (view: LibrarySection) => void;
  children: ReactNode;
}

/**
 * Library's Documents and Study guides pages under Library's tabs, whose first
 * tab, Flashcards, returns to the Workbench. The tabs stay put while the page
 * below them changes, and step aside while a page shows a view of its own (see
 * useLibraryFullPage).
 */
export function LibraryFrame({ view, onSelectView, children }: LibraryFrameProps) {
  const [fullPage, setFullPage] = useState(false);
  return (
    <LibraryPageContext.Provider value={setFullPage}>
      {/* Documents keeps its own width; the tabs line up with each view. */}
      <PageScaffold
        width={view === "documents" ? "wide" : "full"}
        inset={fullPage ? false : undefined}
        data-testid="library"
      >
        {fullPage ? null : (
          <LibraryTabs view={view} onSelectView={onSelectView} className="tw:mb-5 tw:compact:mb-4" />
        )}
        <RouteTransition routeKey={view} relation="sibling" className="tw:min-w-0">
          {children}
        </RouteTransition>
      </PageScaffold>
    </LibraryPageContext.Provider>
  );
}
