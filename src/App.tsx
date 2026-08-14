import { lazy, Suspense } from "react";

import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { IncomingShareCoordinator } from "./components/IncomingShareCoordinator";
import { NavigationSidebar } from "./components/NavigationSidebar";
import { Toast } from "./components/Toast";
import { useAppController } from "./controllers/useAppController";
import { AppearanceProvider } from "./ui/appearance/AppearanceProvider";
import { LoadingState } from "./ui/primitives/EmptyState";

const DecksWorkspace = lazy(() =>
  import("./screens/DecksWorkspace").then((module) => ({ default: module.DecksWorkspace })),
);
const DocumentsSection = lazy(() =>
  import("./screens/DocumentsSection").then((module) => ({ default: module.DocumentsSection })),
);
const CardsScreen = lazy(() =>
  import("./screens/CardsScreen").then((module) => ({ default: module.CardsScreen })),
);
const TemplatesScreen = lazy(() =>
  import("./screens/TemplatesScreen").then((module) => ({ default: module.TemplatesScreen })),
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

  return (
    <AppearanceProvider settings={controller.activeSettings}>
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
          <button type="button" className="primary-button" onClick={() => window.location.reload()}>
            Try again
          </button>
        </div>
      ) : (
        <div className="app-shell" data-ui-root="true">
          <IncomingShareCoordinator
            refreshDecks={controller.refreshDecks}
            showToast={controller.showToast}
          />
          <NavigationSidebar section={controller.section} onNavigate={controller.navigate} />

          <main className="main-area">
            <AppErrorBoundary>
              <Suspense fallback={<LoadingState label="Opening section…" />}>
                {controller.section === "decks" ? (
                  <DecksWorkspace
                    decks={controller.decks}
                    settings={controller.activeSettings}
                    refreshDecks={controller.refreshDecks}
                    onPracticeDeck={controller.practiceDeck}
                    onPracticeCards={controller.practiceCards}
                    openDeckRequest={controller.deckOpenRequest}
                    onOpenDeckRequestConsumed={controller.consumeDeckOpenRequest}
                    onManageTemplates={() => controller.navigate("templates")}
                    onDirtyChange={controller.setHasUnsavedCard}
                    showToast={controller.showToast}
                  />
                ) : null}
                {controller.section === "documents" ? (
                  <DocumentsSection
                    refreshDecks={controller.refreshDecks}
                    showToast={controller.showToast}
                  />
                ) : null}
                {controller.section === "cards" ? (
                  <CardsScreen
                    decks={controller.decks}
                    refreshDecks={controller.refreshDecks}
                    onPracticeCards={controller.practiceCards}
                    onDirtyChange={controller.setHasUnsavedCard}
                    showToast={controller.showToast}
                  />
                ) : null}
                {controller.section === "templates" ? (
                  <TemplatesScreen
                    onBack={() => controller.navigate("decks")}
                    onDirtyChange={controller.setHasUnsavedTemplate}
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
          </main>

          <Toast toast={controller.toast} onDismiss={() => controller.setToast(null)} />
        </div>
      )}
    </AppearanceProvider>
  );
}

export default App;
