import { lazy, Suspense } from "react";

import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { IncomingShareCoordinator } from "./components/IncomingShareCoordinator";
import { MobileAppBar } from "./components/MobileAppBar";
import { NavigationSidebar } from "./components/NavigationSidebar";
import { Toast } from "./components/Toast";
import { useAppController } from "./controllers/useAppController";
import { AppearanceProvider } from "./ui/appearance/AppearanceProvider";
import { LayoutModeProvider } from "./ui/layout/LayoutModeProvider";
import { AmbientBackdrop } from "./ui/materials/AmbientBackdrop";
import { RouteTransition } from "./ui/motion/Motion";
import { Button } from "./ui/primitives/Button";
import { LoadingState } from "./ui/primitives/EmptyState";
import { TourProvider } from "./ui/tutorials/TourProvider";
import { PerformanceProfiler } from "./testing/PerformanceProfiler";
import shellStyles from "./AppShell.module.css";
import statusStyles from "./AppStatus.module.css";

const FlashcardsWorkspace = lazy(() =>
  import("./screens/flashcards/FlashcardsWorkspace").then((module) => ({
    default: module.FlashcardsWorkspace,
  })),
);
const DocumentsSection = lazy(() =>
  import("./screens/DocumentsSection").then((module) => ({ default: module.DocumentsSection })),
);
const PracticeScreen = lazy(() =>
  import("./screens/PracticeScreen").then((module) => ({ default: module.PracticeScreen })),
);
const AnalyticsScreen = lazy(() =>
  import("./screens/AnalyticsScreen").then((module) => ({ default: module.AnalyticsScreen })),
);
const SettingsScreen = lazy(() =>
  import("./screens/SettingsScreen").then((module) => ({ default: module.SettingsScreen })),
);

export type {
  AnalyticsLaunchRequest,
  AppSection,
  DeckOpenRequest,
  PracticeLaunchRequest,
  PracticeReturnTarget,
} from "./controllers/useAppController";

function App() {
  const controller = useAppController();
  const tutorialsEnabled = controller.activeSettings.tutorialsEnabled ?? false;

  return (
    <LayoutModeProvider>
      <AppearanceProvider settings={controller.activeSettings}>
        <TourProvider enabled={!controller.isLoading && !controller.fatalError && tutorialsEnabled}>
          {controller.isLoading ? (
            <div className={statusStyles.status}>
              <div className={statusStyles.mark}>O</div>
              <h1>Opening ostudy</h1>
              <p>Loading your decks…</p>
            </div>
          ) : controller.fatalError ? (
            <div className={statusStyles.status}>
              <h1>ostudy could not open the library</h1>
              <p>{controller.fatalError}</p>
              <Button variant="primary" onClick={() => window.location.reload()}>
                Try again
              </Button>
            </div>
          ) : (
            <>
              <AmbientBackdrop />
              <div className={shellStyles.shell} data-ui-root="true">
                <IncomingShareCoordinator
                  refreshDecks={controller.refreshDecks}
                  showToast={controller.showToast}
                />
                <MobileAppBar section={controller.section} />
                <PerformanceProfiler id="navigation">
                  <NavigationSidebar section={controller.section} onNavigate={controller.navigate} />
                </PerformanceProfiler>

                <main className={shellStyles.main}>
                  <RouteTransition
                    routeKey={controller.section}
                    relation="sibling"
                    className={shellStyles.routeLayer}
                  >
                    <AppErrorBoundary>
                      <Suspense fallback={<LoadingState label="Opening section…" />}>
                        {controller.section === "decks" ? (
                          <FlashcardsWorkspace
                            workspace={controller.flashcardsWorkspace}
                            navigationKey={controller.flashcardsRoute.requestId}
                            onWorkspaceChange={controller.navigateFlashcardsWorkspace}
                            decks={controller.decks}
                            settings={controller.activeSettings}
                            refreshDecks={controller.refreshDecks}
                            onPracticeDeck={controller.practiceDeck}
                            onPracticeCards={controller.practiceCards}
                            openDeckRequest={controller.deckOpenRequest}
                            onOpenDeck={controller.openFlashcardsDeck}
                            onReturnToLibrary={controller.returnToFlashcardsLibrary}
                            onCardDirtyChange={controller.setHasUnsavedCard}
                            onTemplateDirtyChange={controller.setHasUnsavedTemplate}
                            showToast={controller.showToast}
                          />
                        ) : null}
                        {controller.section === "documents" ? (
                          <PerformanceProfiler id="documents">
                            <DocumentsSection
                              refreshDecks={controller.refreshDecks}
                              onPracticeCards={controller.practiceCards}
                              showToast={controller.showToast}
                            />
                          </PerformanceProfiler>
                        ) : null}
                        {controller.section === "practice" ? (
                          <PracticeScreen
                            decks={controller.decks}
                            settings={controller.activeSettings}
                            launchRequest={controller.practiceLaunch}
                            onLaunchConsumed={controller.consumePracticeLaunch}
                            restoreCheckpoint={controller.practiceRestore}
                            onRestoreConsumed={controller.consumePracticeRestore}
                            onViewAnalytics={controller.viewSessionAnalytics}
                            onEndEarly={controller.returnFromPractice}
                            showToast={controller.showToast}
                          />
                        ) : null}
                        {controller.section === "analytics" ? (
                          <AnalyticsScreen
                            decks={controller.decks}
                            settings={controller.activeSettings}
                            launchRequest={controller.analyticsLaunch}
                            onLaunchConsumed={controller.consumeAnalyticsLaunch}
                            onPracticeScope={controller.practiceScope}
                            showToast={controller.showToast}
                          />
                        ) : null}
                        {controller.section === "settings" ? (
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
                </main>

                <Toast toast={controller.toast} onDismiss={() => controller.setToast(null)} />
              </div>
            </>
          )}
        </TourProvider>
      </AppearanceProvider>
    </LayoutModeProvider>
  );
}

export default App;
