import { lazy, Suspense, useEffect, useState } from "react";

import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { IncomingShareCoordinator } from "./components/IncomingShareCoordinator";
import { MobileAppBar } from "./components/MobileAppBar";
import { NavigationSidebar } from "./components/NavigationSidebar";
import { Toast } from "./components/Toast";
import { UsbSyncCoordinator } from "./components/UsbSyncCoordinator";
import { TutorProvider } from "./screens/tutor/TutorProvider";
import { DocumentJobsProvider } from "./controllers/DocumentJobsProvider";
import { useAppController } from "./controllers/useAppController";
import { shouldResetStudyGuidesRoute } from "./navigation/studyGuidesNavigation";
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

const loadFlashcardsWorkspace = () => import("./screens/flashcards/FlashcardsWorkspace");
const loadDocumentsSection = () => import("./screens/DocumentsSection");
const loadStudyGuidesRoute = () => import("./screens/StudyGuidesRoute");
const loadPracticeScreen = () => import("./screens/PracticeScreen");
const loadAnalyticsScreen = () => import("./screens/AnalyticsScreen");
const loadTutorScreen = () => import("./screens/tutor/TutorWorkspace");
const loadSettingsScreen = () => import("./screens/SettingsScreen");

const FlashcardsWorkspace = lazy(() =>
  loadFlashcardsWorkspace().then((module) => ({ default: module.FlashcardsWorkspace })),
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
const AnalyticsScreen = lazy(() =>
  loadAnalyticsScreen().then((module) => ({ default: module.AnalyticsScreen })),
);
const TutorScreen = lazy(() =>
  loadTutorScreen().then((module) => ({ default: module.TutorScreen })),
);
const SettingsScreen = lazy(() =>
  loadSettingsScreen().then((module) => ({ default: module.SettingsScreen })),
);

const FEATURE_PRELOADERS = [
  loadFlashcardsWorkspace,
  loadDocumentsSection,
  loadStudyGuidesRoute,
  loadPracticeScreen,
  loadAnalyticsScreen,
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
  const tutorialsEnabled = controller.activeSettings.tutorialsEnabled ?? false;
  const legacyTemplatesSection = controller.section === "templates";

  useEffect(() => {
    if (controller.isLoading || controller.fatalError) return;
    const timer = window.setTimeout(() => {
      for (const preload of FEATURE_PRELOADERS) {
        void preload().catch(() => undefined);
      }
    }, 750);
    return () => window.clearTimeout(timer);
  }, [controller.fatalError, controller.isLoading]);

  function navigate(section: Parameters<typeof controller.navigate>[0]): void {
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

  return (
    <LayoutModeProvider>
      <AppearanceProvider settings={controller.activeSettings}>
        <TourProvider enabled={!controller.isLoading && !controller.fatalError && tutorialsEnabled}>
          {controller.isLoading ? (
            <div className="tw:grid tw:min-h-dvh tw:place-content-center tw:gap-4 tw:bg-background tw:p-8 tw:text-center tw:text-foreground">
              <div className="tw:mx-auto"><BrandMark /></div>
              <h1>Opening ostudy</h1>
              <p>Loading your decks…</p>
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
                  <MobileAppBar
                    section={controller.section}
                    activeWorkflow={controller.activeWorkflow}
                    onNavigate={navigate}
                  />
                  <PerformanceProfiler id="navigation">
                    <NavigationSidebar
                      section={controller.section}
                      activeWorkflow={controller.activeWorkflow}
                      onNavigate={navigate}
                    />
                  </PerformanceProfiler>

                  <AppContent>
                    <RouteTransition
                      routeKey={controller.activeWorkflow ?? controller.section}
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
                          ) : controller.section === "decks" ? (
                            <FlashcardsWorkspace
                              workspace={controller.flashcardsWorkspace}
                              navigationKey={controller.flashcardsRoute.requestId}
                              onWorkspaceChange={controller.navigateFlashcardsWorkspace}
                              decks={controller.decks}
                              settings={controller.activeSettings}
                              refreshDecks={controller.refreshDecks}
                              onPracticeDeck={controller.practiceDeck}
                              onPracticeSelected={controller.practiceSelected}
                              restoredSelection={
                                controller.flashcardsRoute.view === "root"
                                  ? controller.flashcardsRoute.restoreDeckSelection
                                  : undefined
                              }
                              analyticsSearchLaunch={
                                controller.flashcardsRoute.view === "root"
                                  ? controller.flashcardsRoute.analyticsSearchLaunch
                                  : undefined
                              }
                              onPracticeCards={controller.practiceCards}
                              openDeckRequest={controller.deckOpenRequest}
                              onOpenDeck={controller.openFlashcardsDeck}
                              onReturnToLibrary={controller.returnToFlashcardsLibrary}
                              onCardDirtyChange={controller.setHasUnsavedCard}
                              showToast={controller.showToast}
                            />
                          ) : controller.section === "study-guides" ? (
                            <StudyGuidesRoute
                              key={studyGuidesNavigationKey}
                              showToast={controller.showToast}
                              onPracticeCards={controller.practiceCards}
                            />
                          ) : controller.section === "documents" ? (
                            <PerformanceProfiler id="documents">
                              <DocumentsSection
                                refreshDecks={controller.refreshDecks}
                                onPracticeCards={controller.practiceCards}
                                onOpenDeck={controller.openFlashcardsDeck}
                                showToast={controller.showToast}
                              />
                            </PerformanceProfiler>
                          ) : controller.section === "tutor" ? (
                            <TutorScreen />
                          ) : controller.section === "analytics" ? (
                            <AnalyticsScreen
                              decks={controller.decks}
                              settings={controller.activeSettings}
                              launchRequest={controller.analyticsLaunch}
                              onLaunchConsumed={controller.consumeAnalyticsLaunch}
                              onPracticeScope={controller.practiceScope}
                              onSearchTag={controller.openAnalyticsTagSearch}
                              showToast={controller.showToast}
                            />
                          ) : controller.section === "settings" ? (
                            <SettingsScreen
                              settings={controller.savedSettings}
                              onPreviewSettings={controller.setActiveSettings}
                              onSavedSettings={controller.saveSettingsLocally}
                              onDirtyChange={controller.setHasUnsavedSettings}
                              showToast={controller.showToast}
                            />
                          ) : legacyTemplatesSection ? (
                            <SettingsScreen
                              settings={controller.savedSettings}
                              initialSection="templates"
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
            </DocumentJobsProvider>
          )}
        </TourProvider>
      </AppearanceProvider>
    </LayoutModeProvider>
  );
}

export default App;
