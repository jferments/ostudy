import { createContext, useContext, useLayoutEffect, useState, type ReactNode } from "react";

import type { ShowToast } from "../../components/Toast";
import type { LibrarySubjectScope, SubjectSection } from "../../navigation/appDestinations";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { RouteTransition } from "../../ui/motion/Motion";
import { PageScaffold } from "../../ui/patterns/WorkspacePatterns";
import { Button } from "../../ui/primitives/Button";
import { LibraryTabs } from "./LibraryTabs";
import { LibrarySubjectHeader, useLibrarySubjects } from "./LibrarySubjects";

interface LibraryPageControls {
  setFullPage: (fullPage: boolean) => void;
  setTabActions: (actions: ReactNode) => void;
}

/** Lets a hosted view hand the whole page to one of its own pages, or add actions beside the tabs. */
const LibraryPageContext = createContext<LibraryPageControls | null>(null);

/**
 * Hands the Library page to a view's own page while `active`, such as an open
 * study guide: the header and tabs step aside and the frame drops
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
  view: SubjectSection;
  /** Choosing the open view again returns it to its first page. */
  onSelectView: (view: SubjectSection) => void;
  subject: LibrarySubjectScope;
  onChooseSubject: (subject: LibrarySubjectScope) => void;
  /** Back to Library's page of subjects. */
  onOpenLibrary: () => void;
  onStartReview: (deckIds: string[]) => void;
  onPracticeSubject: (subjectId: string) => void;
  showToast: ShowToast;
  /** The page fills the frame's height and scrolls its own panes, like the Workbench. */
  fill?: boolean;
  children: ReactNode;
}

/**
 * A subject in Library: the way back to Library, the subject's header, its
 * tabs and the open page below them. The header and tabs stay put while the
 * page below them changes. Search's All cards has the header without tabs.
 */
export function LibraryFrame({
  view,
  onSelectView,
  subject,
  onChooseSubject,
  onOpenLibrary,
  onStartReview,
  onPracticeSubject,
  showToast,
  fill = false,
  children,
}: LibraryFrameProps) {
  const [fullPage, setFullPage] = useState(false);
  const [tabActions, setTabActions] = useState<ReactNode>(null);
  const [controls] = useState<LibraryPageControls>(() => ({ setFullPage, setTabActions }));
  const subjects = useLibrarySubjects(subject);

  const shown = !fullPage;
  // One tree whether or not a view has the whole page, so the view stays
  // mounted while the header and tabs come and go.
  return (
    <LibraryPageContext.Provider value={controls}>
      <PageScaffold
        width="full"
        inset={false}
        className={cn(shown && fill && "tw:flex tw:h-full tw:min-h-0 tw:flex-col tw:compact:h-auto")}
        data-testid="library"
      >
        <div
          className={cn(
            "tw:min-w-0",
            shown && "tw:px-5 tw:pt-3 tw:compact:px-4 tw:compact:pt-2",
            shown && (fill
              ? "tw:flex tw:h-full tw:min-h-0 tw:flex-col tw:pb-4 tw:compact:h-auto tw:compact:pb-[calc(5rem+var(--safe-bottom))]"
              : "tw:pb-[calc(4rem+var(--safe-bottom))]"),
          )}
        >
          {shown ? (
            <Button
              variant="back"
              data-testid="library-back"
              aria-label="Back to Library"
              className="tw:mb-1 tw:shrink-0 tw:justify-self-start"
              onClick={onOpenLibrary}
            >
              <Icon name="chevron-left" size={16} /> Library
            </Button>
          ) : null}
          {shown ? (
            <LibrarySubjectHeader
              scope={subject}
              subjects={subjects}
              onChooseSubject={onChooseSubject}
              showToast={showToast}
              onStartReview={onStartReview}
              onPracticeSubject={onPracticeSubject}
              onOpenDocuments={() => onSelectView("documents")}
            />
          ) : null}
          {shown && subject.kind !== "all" ? (
            <LibraryTabs view={view} onSelectView={onSelectView} actions={tabActions} className="tw:mt-4 tw:mb-4 tw:shrink-0" />
          ) : shown ? (
            <div className="tw:mt-4 tw:mb-2 tw:flex tw:shrink-0 tw:justify-end">{tabActions}</div>
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
    </LibraryPageContext.Provider>
  );
}
