import { createContext, useContext, useLayoutEffect, useState, type ReactNode } from "react";

import type { LibrarySection, LibrarySubjectScope } from "../../navigation/appDestinations";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { cn } from "../../ui/internal/cn";
import { RouteTransition } from "../../ui/motion/Motion";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { LibraryTabs } from "./LibraryTabs";
import {
  LibrarySubjectHeader,
  LibrarySubjectList,
  LibrarySubjectSheet,
  useLibrarySubjects,
} from "./LibrarySubjects";

interface LibraryPageControls {
  setFullPage: (fullPage: boolean) => void;
  setTabActions: (actions: ReactNode) => void;
}

/** Lets a hosted view hand the whole page to one of its own pages, or add actions beside the tabs. */
const LibraryPageContext = createContext<LibraryPageControls | null>(null);

/**
 * Hands the Library page to a view's own page while `active`, such as an open
 * study guide: the subjects, header and tabs step aside and the frame drops
 * its inset, so the page's sticky top bar starts at the edges of the scroll
 * area, right under the app header. The page's own back button returns to the
 * view. Outside a LibraryFrame it does nothing.
 */
export function useLibraryFullPage(active: boolean): void {
  const setFullPage = useContext(LibraryPageContext)?.setFullPage;
  useLayoutEffect(() => {
    if (!active || !setFullPage) return;
    setFullPage(true);
    return () => setFullPage(false);
  }, [active, setFullPage]);
}

/** Shows a page's own actions, such as Import deck, at the end of Library's tabs. Pass a stable node. */
export function useLibraryTabActions(actions: ReactNode): void {
  const setTabActions = useContext(LibraryPageContext)?.setTabActions;
  useLayoutEffect(() => {
    if (!setTabActions) return;
    setTabActions(actions);
    return () => setTabActions(null);
  }, [actions, setTabActions]);
}

interface LibraryFrameProps {
  view: LibrarySection;
  /** Choosing the open view again returns it to its first page. */
  onSelectView: (view: LibrarySection) => void;
  subject: LibrarySubjectScope;
  onChooseSubject: (subject: LibrarySubjectScope) => void;
  onStartReview: (deckIds: string[]) => void;
  onPracticeSubject: (subjectId: string) => void;
  /** The page fills the frame's height and scrolls its own panes, like the Workbench. */
  fill?: boolean;
  children: ReactNode;
}

/**
 * Library: the subjects beside the page on wide screens, the chosen subject's
 * header and Library's tabs, and the open page below them. The subjects,
 * header and tabs stay put while the page below them changes. Narrower
 * screens choose the subject from the header.
 */
export function LibraryFrame({
  view,
  onSelectView,
  subject,
  onChooseSubject,
  onStartReview,
  onPracticeSubject,
  fill = false,
  children,
}: LibraryFrameProps) {
  const [fullPage, setFullPage] = useState(false);
  const [tabActions, setTabActions] = useState<ReactNode>(null);
  const [controls] = useState<LibraryPageControls>(() => ({ setFullPage, setTabActions }));
  const [switching, setSwitching] = useState(false);
  const { layoutMode } = useLayoutMode();
  const subjects = useLibrarySubjects(subject);
  const wide = layoutMode === "wide";

  const shown = !fullPage;
  // One tree whether or not a view has the whole page, so the view stays
  // mounted while the subjects, header and tabs come and go.
  return (
    <LibraryPageContext.Provider value={controls}>
      <PageScaffold
        width="full"
        inset={false}
        className={cn(
          wide && shown && "tw:flex tw:h-full tw:min-h-0",
          !wide && shown && fill && "tw:flex tw:h-full tw:min-h-0 tw:flex-col tw:compact:h-auto",
        )}
        data-testid="library"
      >
        {wide && shown ? (
          <aside
            aria-label="Library subjects"
            className="tw:h-full tw:w-60 tw:shrink-0 tw:overflow-y-auto tw:border-0 tw:border-r tw:border-solid tw:border-border tw:bg-foreground/4 tw:px-3 tw:py-5"
          >
            <LibrarySubjectList scope={subject} subjects={subjects} onChoose={onChooseSubject} />
          </aside>
        ) : null}
        <div
          className={cn(
            "tw:min-w-0",
            shown && "tw:flex-1 tw:px-5 tw:pt-5 tw:compact:px-4 tw:compact:pt-4",
            shown && (fill
              ? "tw:flex tw:h-full tw:min-h-0 tw:flex-col tw:pb-4 tw:compact:h-auto tw:compact:pb-[calc(5rem+var(--safe-bottom))]"
              : "tw:pb-[calc(4rem+var(--safe-bottom))]"),
            shown && wide && "tw:h-full tw:overflow-y-auto",
          )}
        >
          {shown ? (
            <LibrarySubjectHeader
              scope={subject}
              subjects={subjects}
              onSwitch={wide ? undefined : () => setSwitching(true)}
              onStartReview={onStartReview}
              onPracticeSubject={onPracticeSubject}
            />
          ) : null}
          {shown ? (
            <LibraryTabs view={view} onSelectView={onSelectView} actions={tabActions} className="tw:mt-4 tw:mb-4 tw:shrink-0" />
          ) : null}
          <RouteTransition
            routeKey={view}
            relation="sibling"
            className={cn("tw:min-w-0", fill && shown && "tw:flex tw:min-h-0 tw:flex-1 tw:flex-col")}
          >
            {children}
          </RouteTransition>
        </div>
      </PageScaffold>
      {switching ? (
        <LibrarySubjectSheet
          scope={subject}
          subjects={subjects}
          onChoose={onChooseSubject}
          onClose={() => setSwitching(false)}
        />
      ) : null}
    </LibraryPageContext.Provider>
  );
}
