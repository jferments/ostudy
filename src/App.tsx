import { lazy, Suspense, useEffect, useState } from "react";

import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { AppHeaderBar } from "./components/AppHeaderBar";
import { IncomingShareCoordinator } from "./components/IncomingShareCoordinator";
import { MainNavigation } from "./components/MainNavigation";
import { Toast } from "./components/Toast";
import { UsbSyncCoordinator } from "./components/UsbSyncCoordinator";
import { TutorProvider } from "./screens/tutor/TutorProvider";
import { DocumentJobsProvider } from "./controllers/DocumentJobsProvider";
import {
  cardsDueToday,
  HomeSummaryProvider,
  useHomeSummary,
} from "./controllers/HomeSummaryProvider";
import {
  useAppController,
  type AppSection,
  type AppWorkflow,
} from "./controllers/useAppController";
import { isLibrarySection, type AppDestination } from "./navigation/appDestinations";
import { shouldResetStudyGuidesRoute } from "./navigation/studyGuidesNavigation";
import { HomeScreen } from "./screens/home/HomeScreen";
import { LibraryLessons } from "./screens/library/LibraryLessons";
import { LibraryOverview } from "./screens/library/LibraryOverview";
import { LibraryFrame } from "./screens/library/LibraryFrame";
import { workbenchLaunchFor } from "./screens/library/workbench/workbenchLaunch";
import { PerformanceProfiler } from "./testing/PerformanceProfiler";
import { AppearanceProvider } from "./ui/appearance/AppearanceProvider";
import { LayoutModeProvider } from "./ui/layout/LayoutModeProvider";
import { RouteTransition } from "./ui/motion/Motion";
import { Button } from "./ui/primitives/Button";
import { LoadingState } from "./ui/primitives/EmptyState";
import { TourProvider } from "./ui/tutorials/TourProvider";
import shellStyles from "./AppShell.module.css";
import { AppFrame, AppContent } from "./ui/layout/AppFrame";
import { BrandMark } from "./components/Brand";
import { InlineNotice } from "./ui/primitives/InlineNotice";

const loadLibraryWorkbench = () => import("./screens/library/workbench/LibraryWorkbench");
const loadDocumentsSection = () => import("./screens/DocumentsSection");
const loadStudyGuidesRoute = () => import("./screens/StudyGuidesRoute");
const loadPracticeScreen = () => import("./screens/PracticeScreen");
const loadProgressScreen = () => import("./screens/progress/ProgressScreen");
const loadTutorScreen = () => import("./screens/tutor/TutorWorkspace");
const loadSettingsScreen = () => import("./screens/SettingsScreen");

const LibraryWorkbench = lazy(() =>
  loadLibraryWorkbench().then((module) => ({ default: module.LibraryWorkbench })),
);
const DocumentsSection = lazy(() =>
  loadDocumentsSection().then((module) => ({ default: module.DocumentsSection })),
);
const StudyGuidesRoute = lazy(() =>
  loadStudyGuidesRoute().then((module) => ({ default: module.StudyGuidesRoute })),
);
const PracticeScreen = lazy(() =>
  loadPracticeScreen().then((module) => ({ default: module.PracticeScreen })),
);
const ProgressScreen = lazy(() =>
  loadProgressScreen().then((module) => ({ default: module.ProgressScreen })),
);
const TutorScreen = lazy(() =>
  loadTutorScreen().then((module) => ({ default: module.TutorScreen })),
);
const SettingsScreen = lazy(() =>
  loadSettingsScreen().then((module) => ({ default: module.SettingsScreen })),
);

const FEATURE_PRELOADERS = [
  loadLibraryWorkbench,
  loadDocumentsSection,
  loadStudyGuidesRoute,
  loadPracticeScreen,
  loadProgressScreen,
  loadTutorScreen,
  loadSettingsScreen,
] as const;

export type {
  AnalyticsLaunchRequest,
  AppSection,
  AppWorkflow,
  ContextualPracticeLaunchRequest,
  DeckOpenRequest,
  PracticeLaunchRequest,
  PracticeReturnTarget,
} from "./controllers/useAppController";

function App() {
  const controller = useAppController();
  const [studyGuidesNavigationKey, setStudyGuidesNavigationKey] = useState(0);
  // The lesson Library's Lessons page is playing.
  const [openLessonState, setOpenLesson] = useState<{ subjectId: string; lessonId: string } | null>(null);
  const tutorialsEnabled = controller.activeSettings.tutorialsEnabled ?? false;

  useEffect(() => {
    if (controller.isLoading || controller.fatalError) return;
    const timer = window.setTimeout(() => {
      for (const preload of FEATURE_PRELOADERS) {
        void preload().catch(() => undefined);
      }
    }, 750);
    return () => window.clearTimeout(timer);
  }, [controller.fatalError, controller.isLoading]);

  function navigate(section: AppSection): void {
    if (
      shouldResetStudyGuidesRoute(
        section,
        controller.section,
        controller.activeWorkflow,
      )
    ) {
      setStudyGuidesNavigationKey((current) => current + 1);
    }
    void controller.navigate(section);
  }

  function selectDestination(destination: AppDestination): void {
    navigate(DESTINATION_SECTIONS[destination] ?? controller.librarySection);
  }

  function startReview(deckIds: string[]): void {
    void controller.practiceScope(deckIds, []);
  }

  function openLesson(subjectId: string, lessonId: string): void {
    setOpenLesson({ subjectId, lessonId });
    controller.chooseLibrarySubject({ kind: "subject", subjectId });
    navigate("lessons");
  }

  const librarySubject = controller.librarySubject;
  const libraryFrame = {
    subject: librarySubject,
    onChooseSubject: controller.chooseLibrarySubject,
    onSelectView: navigate,
    onStartReview: startReview,
  };
  const subjectFilter =
    librarySubject.kind === "all" ? undefined : librarySubject.kind === "none" ? null : librarySubject.subjectId;

  // Review lives on Home, so Home also stands in for the legacy Review section.
  const homeScreen = (
    <HomeScreen
      onStartReview={startReview}
      onOpenSubject={(subjectId) => void controller.openSubject(subjectId)}
      onOpenLesson={openLesson}
      onOpenLibrary={() => void controller.openLibrary()}
      onPracticeSubject={(subjectId) => void controller.practiceSubject(subjectId)}
      onOpenProgress={() => navigate("analytics")}
    />
  );

  return (
    <LayoutModeProvider>
      <AppearanceProvider settings={controller.activeSettings}>
        <TourProvider enabled={!controller.isLoading && !controller.fatalError && tutorialsEnabled}>
          {controller.isLoading ? (
            <div className="tw:grid tw:min-h-dvh tw:place-content-center tw:gap-4 tw:bg-background tw:p-8 tw:text-center tw:text-foreground">
              <div className="tw:mx-auto"><BrandMark className="tw:size-12" /></div>
              <h1>Opening ostudy</h1>
              <p>Loading your library…</p>
            </div>
          ) : controller.fatalError ? (
            <div className="tw:grid tw:min-h-dvh tw:place-content-center tw:gap-4 tw:bg-background tw:p-8 tw:text-center tw:text-foreground">
              <h1>ostudy could not open the library</h1>
              <InlineNotice kind="error">{controller.fatalError}</InlineNotice>
              <Button variant="primary" onClick={() => window.location.reload()}>
                Try again
              </Button>
            </div>
          ) : (
            <DocumentJobsProvider refreshDecks={controller.refreshDecks} showToast={controller.showToast}>
              <HomeSummaryProvider
                section={controller.section}
                activeWorkflow={controller.activeWorkflow}
                decks={controller.decks}
                finishedSessions={controller.finishedPracticeSessions}
                studyRules={controller.savedSettings}
              >
                <TutorProvider>
                  <AppFrame>
                    <IncomingShareCoordinator
                      refreshDecks={controller.refreshDecks}
                      showToast={controller.showToast}
                    />
                    <UsbSyncCoordinator
                      refreshDecks={controller.refreshDecks}
                      showToast={controller.showToast}
                    />
                    <PerformanceProfiler id="navigation">
                      <ShellHeader
                        section={controller.section}
                        activeWorkflow={controller.activeWorkflow}
                        onSelectDestination={selectDestination}
                        onSearch={() => void controller.openSearch()}
                        onSettings={() => navigate("settings")}
                      />
                    </PerformanceProfiler>

                    <AppContent>
                      <RouteTransition
                        routeKey={
                          controller.activeWorkflow ??
                          (isLibrarySection(controller.section) ? "library" : controller.section)
                        }
                        relation="sibling"
                        className={shellStyles.routeLayer}
                      >
                        <AppErrorBoundary>
                          <Suspense fallback={<LoadingState label="Opening section…" />}>
                            {controller.activeWorkflow === "practice" ? (
                              <PracticeScreen
                                decks={controller.decks}
                                settings={controller.activeSettings}
                                launchRequest={controller.practiceWorkflow?.launchRequest ?? null}
                                onLaunchConsumed={controller.consumePracticeLaunch}
                                restoreCheckpoint={
                                  controller.practiceWorkflow?.restoreCheckpoint ?? null
                                }
                                onRestoreConsumed={controller.consumePracticeRestore}
                                onCheckpointChange={controller.reportPracticeCheckpoint}
                                returnTarget={controller.practiceWorkflow?.returnTarget ?? null}
                                onViewAnalytics={controller.viewSessionAnalytics}
                                onEndEarly={controller.returnFromPractice}
                                onChooseDecks={controller.choosePracticeDecks}
                                showToast={controller.showToast}
                              />
                            ) : controller.section === "home" ? (
                              homeScreen
                            ) : controller.section === "overview" ? (
                              <LibraryFrame view="overview" {...libraryFrame}>
                                {/* The tabs name the page; its title is for headings navigation. */}
                                <h1 className="tw:sr-only">Overview</h1>
                                <LibraryOverview
                                  subject={librarySubject}
                                  decks={controller.decks}
                                  onSelectView={navigate}
                                  onOpenLesson={openLesson}
                                  onOpenDeck={(deckId) => void controller.openLibrary({ deckId })}
                                />
                              </LibraryFrame>
                            ) : controller.section === "lessons" ? (
                              <LibraryFrame view="lessons" {...libraryFrame}>
                                <h1 className="tw:sr-only">Lessons</h1>
                                <LibraryLessons
                                  subject={librarySubject}
                                  openLessonId={
                                    openLessonState !== null &&
                                    librarySubject.kind === "subject" &&
                                    openLessonState.subjectId === librarySubject.subjectId
                                      ? openLessonState.lessonId
                                      : null
                                  }
                                  onOpenLesson={openLesson}
                                  onCloseLesson={() => setOpenLesson(null)}
                                  showToast={controller.showToast}
                                />
                              </LibraryFrame>
                            ) : controller.section === "decks" ? (
                              <LibraryFrame view="decks" fill {...libraryFrame}>
                                <LibraryWorkbench
                                  decks={controller.decks}
                                  refreshDecks={controller.refreshDecks}
                                  launch={workbenchLaunchFor(controller.flashcardsRoute)}
                                  onPracticeDeck={controller.practiceDeck}
                                  onPracticeCards={controller.practiceCards}
                                  onPracticeSelected={(request) => controller.practiceSelected(request, [])}
                                  practiceShuffle={controller.activeSettings.practiceShuffle}
                                  onOpenDocuments={() => navigate("documents")}
                                  onCardDirtyChange={controller.setHasUnsavedCard}
                                  onDeckScopeChange={controller.recordLibraryDeck}
                                  subject={librarySubject}
                                  onSubjectChange={controller.recordLibrarySubject}
                                  showToast={controller.showToast}
                                />
                              </LibraryFrame>
                            ) : controller.section === "study-guides" ? (
                              <LibraryFrame view="study-guides" {...libraryFrame}>
                                <StudyGuidesRoute
                                  key={studyGuidesNavigationKey}
                                  subject={librarySubject}
                                  showToast={controller.showToast}
                                  onPracticeCards={controller.practiceCards}
                                />
                              </LibraryFrame>
                            ) : controller.section === "documents" ? (
                              <LibraryFrame view="documents" {...libraryFrame}>
                                <PerformanceProfiler id="documents">
                                  <DocumentsSection
                                    refreshDecks={controller.refreshDecks}
                                    onPracticeCards={controller.practiceCards}
                                    onStudyCollection={controller.practiceSubject}
                                    subjectId={subjectFilter}
                                    showToast={controller.showToast}
                                  />
                                </PerformanceProfiler>
                              </LibraryFrame>
                            ) : controller.section === "review" ? (
                              homeScreen
                            ) : controller.section === "tutor" ? (
                              <TutorScreen />
                            ) : controller.section === "analytics" ? (
                              <ProgressScreen
                                decks={controller.decks}
                                launchRequest={controller.analyticsLaunch}
                                onLaunchConsumed={controller.consumeAnalyticsLaunch}
                                onPracticeScope={controller.practiceScope}
                                onSearchTag={controller.openAnalyticsTagSearch}
                              />
                            ) : controller.section === "settings" ? (
                              <SettingsScreen
                                settings={controller.savedSettings}
                                onPreviewSettings={controller.setActiveSettings}
                                onSavedSettings={controller.saveSettingsLocally}
                                onDirtyChange={controller.setHasUnsavedSettings}
                                showToast={controller.showToast}
                              />
                            ) : null}
                          </Suspense>
                        </AppErrorBoundary>
                      </RouteTransition>
                    </AppContent>

                    <Toast toast={controller.toast} onDismiss={() => controller.setToast(null)} />
                  </AppFrame>
                </TutorProvider>
              </HomeSummaryProvider>
            </DocumentJobsProvider>
          )}
        </TourProvider>
      </AppearanceProvider>
    </LayoutModeProvider>
  );
}

/** Library opens its last view, so it has no single section here. */
const DESTINATION_SECTIONS: Readonly<Record<AppDestination, AppSection | null>> = {
  home: "home",
  library: null,
  tutor: "tutor",
  progress: "analytics",
  settings: "settings",
};

interface ShellHeaderProps {
  section: AppSection;
  activeWorkflow: AppWorkflow | null;
  onSelectDestination: (destination: AppDestination) => void;
  onSearch: () => void;
  onSettings: () => void;
}

/** The header and main navigation, with today's count on Home. */
function ShellHeader({
  section,
  activeWorkflow,
  onSelectDestination,
  onSearch,
  onSettings,
}: ShellHeaderProps) {
  const { summary } = useHomeSummary();
  return (
    <AppHeaderBar
      section={section}
      activeWorkflow={activeWorkflow}
      onSearch={onSearch}
      onSettings={onSettings}
      navigation={
        <MainNavigation
          section={section}
          activeWorkflow={activeWorkflow}
          dueToday={cardsDueToday(summary)}
          onSelect={onSelectDestination}
        />
      }
    />
  );
}

export default App;
