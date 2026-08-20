import { lazy, Suspense } from "react";

import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { IncomingShareCoordinator } from "./components/IncomingShareCoordinator";
import { MobileAppBar } from "./components/MobileAppBar";
import { NavigationSidebar } from "./components/NavigationSidebar";
import { Toast } from "./components/Toast";
import { useAppController } from "./controllers/useAppController";
import { AppearanceProvider } from "./ui/appearance/AppearanceProvider";
import { AmbientBackdrop } from "./ui/materials/AmbientBackdrop";
import { RouteTransition } from "./ui/motion/Motion";
import { LoadingState } from "./ui/primitives/EmptyState";
import { TourProvider } from "./ui/tutorials/TourProvider";
import shellStyles from "./AppShell.module.css";

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
  PracticeLaunchRequest,
  PracticeReturnTarget,
} from "./controllers/useAppController";

function App() {
  const controller = useAppController();
  const tutorialsEnabled = controller.activeSettings.tutorialsEnabled ?? true;

  return (
    <AppearanceProvider settings={controller.activeSettings}>
      <TourProvider enabled={!controller.isLoading && !controller.fatalError && tutorialsEnabled}>
        {controller.isLoading ? (
          <div className="app-loading">
            <div className="loading-mark">O</div>
            <h1>Opening ostudy</h1>
            <p>Loading your decks…</p>
          </div>
        ) : controller.fatalError ? (
          <div className="fatal-screen">
            <h1>ostudy could not open the library</h1>
            <p>{controller.fatalError}</p>
            <button
              type="button"
              className="primary-button"
              onClick={() => window.location.reload()}
            >
              Try again
            </button>
          </div>
        ) : (
          <>
            <AmbientBackdrop />
            <div className={`${shellStyles.shell} app-shell`} data-ui-root="true">
              <IncomingShareCoordinator
                refreshDecks={controller.refreshDecks}
                showToast={controller.showToast}
              />
              <MobileAppBar section={controller.section} />
              <NavigationSidebar section={controller.section} onNavigate={controller.navigate} />

              <main className={`${shellStyles.main} main-area`}>
                <RouteTransition
                  routeKey={controller.section}
                  relation="sibling"
                  className={shellStyles.routeLayer}
                >
                  <AppErrorBoundary>
                    <Suspense fallback={<LoadingState label="Opening section…" />}>
                      {controller.section === "decks" ? (
                        <FlashcardsWorkspace
                          route={controller.flashcardsRoute}
                          onWorkspaceChange={controller.navigateFlashcardsWorkspace}
                          onDeckViewChange={controller.syncFlashcardsDeckView}
                          decks={controller.decks}
                          settings={controller.activeSettings}
                          refreshDecks={controller.refreshDecks}
                          onPracticeDeck={controller.practiceDeck}
                          onPracticeCards={controller.practiceCards}
                          onCardDirtyChange={controller.setHasUnsavedCard}
                          onTemplateDirtyChange={controller.setHasUnsavedTemplate}
                          showToast={controller.showToast}
                        />
                      ) : null}
                      {controller.section === "documents" ? (
                        <DocumentsSection
                          refreshDecks={controller.refreshDecks}
                          showToast={controller.showToast}
                        />
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
  );
}

export default App;
