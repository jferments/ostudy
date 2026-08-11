import { lazy, Suspense, useLayoutEffect } from "react";

import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { NavigationSidebar } from "./components/NavigationSidebar";
import { Toast } from "./components/Toast";
import { useAppController } from "./controllers/useAppController";
import type { AppSettings } from "./ipc/settings";
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
  PracticeLaunchRequest,
} from "./controllers/useAppController";

function App() {
  const controller = useAppController();

  useLayoutEffect(() => {
    applyThemeSettings(controller.activeSettings);
  }, [controller.activeSettings]);

  if (controller.isLoading) {
    return (
      <div className="app-loading">
        <div className="loading-mark">O</div>
        <h1>Opening ostudy</h1>
        <p>Loading your decks…</p>
      </div>
    );
  }

  if (controller.fatalError) {
    return (
      <div className="fatal-screen">
        <h1>ostudy could not open the library</h1>
        <p>{controller.fatalError}</p>
        <button type="button" className="primary-button" onClick={() => window.location.reload()}>
          Try again
        </button>
      </div>
    );
  }

  return (
    <div className="app-shell" data-ui-root="true">
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
                onViewAnalytics={controller.viewSessionAnalytics}
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
  );
}

function applyThemeSettings(settings: AppSettings): void {
  const root = document.documentElement;
  root.dataset.theme = settings.theme;
  root.dataset.animationsEnabled = settings.animationsEnabled ? "true" : "false";
  root.dataset.reduceMotion = settings.reduceMotion ? "true" : "false";
  root.style.setProperty("--ui-scale", String(settings.uiScale / 100));
  root.style.setProperty("--card-base-size", `${settings.cardBaseSize}px`);
  const uiFont =
    settings.theme === "console" && settings.uiFont === "system" ? "mono" : settings.uiFont;
  const cardFont =
    settings.theme === "console" && settings.cardFont === "system" ? "mono" : settings.cardFont;
  root.style.setProperty("--ui-font-family", resolveFont(uiFont));
  root.style.setProperty("--card-font-family", resolveFont(cardFont));
  root.style.colorScheme = settings.theme === "light" ? "light" : "dark";
}

function resolveFont(value: string): string {
  const presets: Record<string, string> = {
    system:
      "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
    humanist: "'Trebuchet MS', 'Segoe UI', ui-sans-serif, sans-serif",
    serif: "Georgia, 'Times New Roman', serif",
    mono: "'JetBrains Mono', 'Cascadia Code', 'Liberation Mono', monospace",
  };
  if (value.startsWith("custom:")) {
    const custom = value.slice("custom:".length).trim();
    return custom ? `'${custom.replaceAll("'", "")}', ${presets.system}` : presets.system;
  }
  return presets[value] ?? presets.system;
}

export default App;
