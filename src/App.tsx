import { useCallback, useEffect, useRef, useState } from "react";

import { AppErrorBoundary } from "./components/AppErrorBoundary";
import { Toast, type ToastMessage } from "./components/Toast";
import { AnalyticsScreen } from "./screens/AnalyticsScreen";
import { CardsScreen } from "./screens/CardsScreen";
import { DecksScreen } from "./screens/DecksScreen";
import { PracticeScreen } from "./screens/PracticeScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { TemplatesScreen } from "./screens/TemplatesScreen";
import {
  getErrorMessage,
  getSettings,
  listDecks,
  type AppSettings,
  type Deck,
} from "./ipc";

export type AppSection =
  | "decks"
  | "cards"
  | "templates"
  | "practice"
  | "analytics"
  | "settings";

export interface PracticeLaunchRequest {
  requestId: number;
  deckIds: string[];
  cardIds: string[];
  autoStart: boolean;
}

const DEFAULT_SETTINGS: AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  reduceMotion: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
};

function App() {
  const [section, setSection] = useState<AppSection>("decks");
  const [decks, setDecks] = useState<Deck[]>([]);
  const [savedSettings, setSavedSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [activeSettings, setActiveSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [isLoading, setIsLoading] = useState(true);
  const [fatalError, setFatalError] = useState<string | null>(null);
  const [toast, setToast] = useState<ToastMessage | null>(null);
  const toastSequence = useRef(1);
  const [practiceLaunch, setPracticeLaunch] = useState<PracticeLaunchRequest | null>(null);
  const practiceRequestSequence = useRef(1);
  const [hasUnsavedEditor, setHasUnsavedEditor] = useState(false);
  const [hasUnsavedSettings, setHasUnsavedSettings] = useState(false);

  const showToast = useCallback((kind: ToastMessage["kind"], text: string) => {
    const id = toastSequence.current;
    toastSequence.current += 1;
    setToast({ id, kind, text });
  }, []);

  const refreshDecks = useCallback(async () => {
    const loaded = await listDecks();
    setDecks(loaded);
    return loaded;
  }, []);

  const consumePracticeLaunch = useCallback(() => setPracticeLaunch(null), []);

  useEffect(() => {
    let current = true;
    void Promise.all([getSettings(), listDecks()])
      .then(([loadedSettings, loadedDecks]) => {
        if (!current) return;
        setSavedSettings(loadedSettings);
        setActiveSettings(loadedSettings);
        setDecks(loadedDecks);
        setFatalError(null);
      })
      .catch((error: unknown) => {
        if (current) setFatalError(getErrorMessage(error));
      })
      .finally(() => {
        if (current) setIsLoading(false);
      });
    return () => {
      current = false;
    };
  }, []);

  useEffect(() => applyThemeSettings(activeSettings), [activeSettings]);

  useEffect(() => {
    if (!hasUnsavedEditor && !hasUnsavedSettings) return;
    function warnBeforeClose(event: BeforeUnloadEvent): void {
      event.preventDefault();
      event.returnValue = "";
    }
    window.addEventListener("beforeunload", warnBeforeClose);
    return () => window.removeEventListener("beforeunload", warnBeforeClose);
  }, [hasUnsavedEditor, hasUnsavedSettings]);

  function confirmDiscardEditor(): boolean {
    if (!hasUnsavedEditor) return true;
    if (!window.confirm("Discard the unsaved changes?")) return false;
    setHasUnsavedEditor(false);
    return true;
  }

  function confirmDiscardSettings(): boolean {
    if (section !== "settings" || !hasUnsavedSettings) return true;
    if (!window.confirm("Discard the unsaved settings preview?")) return false;
    setActiveSettings(savedSettings);
    setHasUnsavedSettings(false);
    return true;
  }

  function navigate(next: AppSection): void {
    if (next === section && next !== "practice") return;
    if (!confirmDiscardEditor() || !confirmDiscardSettings()) return;
    if (next === "practice") {
      const requestId = practiceRequestSequence.current;
      practiceRequestSequence.current += 1;
      setPracticeLaunch({ requestId, deckIds: [], cardIds: [], autoStart: false });
    }
    if (next === "decks" || next === "cards") {
      void refreshDecks().catch((error: unknown) => {
        showToast("error", getErrorMessage(error));
      });
    }
    setSection(next);
  }

  function practiceDeck(deckId: string): void {
    if (!confirmDiscardEditor() || !confirmDiscardSettings()) return;
    const requestId = practiceRequestSequence.current;
    practiceRequestSequence.current += 1;
    setPracticeLaunch({ requestId, deckIds: [deckId], cardIds: [], autoStart: true });
    setSection("practice");
  }

  function practiceCards(cardIds: string[]): void {
    if (cardIds.length === 0) return;
    if (!confirmDiscardEditor() || !confirmDiscardSettings()) return;
    const requestId = practiceRequestSequence.current;
    practiceRequestSequence.current += 1;
    setPracticeLaunch({ requestId, deckIds: [], cardIds, autoStart: true });
    setSection("practice");
  }

  if (isLoading) {
    return (
      <div className="app-loading">
        <div className="loading-mark">O</div>
        <h1>Opening ostudy</h1>
        <p>Loading your decks…</p>
      </div>
    );
  }

  if (fatalError) {
    return (
      <div className="fatal-screen">
        <h1>ostudy could not open the library</h1>
        <p>{fatalError}</p>
        <button type="button" className="primary-button" onClick={() => window.location.reload()}>
          Try again
        </button>
      </div>
    );
  }

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <div className="brand-block"><strong>ostudy</strong></div>
        <nav className="sidebar-nav" aria-label="Main navigation">
          <NavButton active={section === "decks"} icon="▤" label="Decks" onClick={() => navigate("decks")} />
          <NavButton active={section === "cards"} icon="⌕" label="Search/Filter" onClick={() => navigate("cards")} />
          <NavButton active={section === "templates"} icon="◇" label="Templates" onClick={() => navigate("templates")} />
          <NavButton active={section === "practice"} icon="▶" label="Practice" onClick={() => navigate("practice")} />
          <NavButton active={section === "analytics"} icon="▥" label="Analytics" onClick={() => navigate("analytics")} />
          <NavButton active={section === "settings"} icon="⚙" label="Settings" onClick={() => navigate("settings")} />
        </nav>
      </aside>

      <main className="main-area">
        <AppErrorBoundary>
          {section === "decks" ? (
            <DecksScreen
              decks={decks}
              settings={activeSettings}
              refreshDecks={refreshDecks}
              onPracticeDeck={practiceDeck}
              onDirtyChange={setHasUnsavedEditor}
              showToast={showToast}
            />
          ) : null}
          {section === "cards" ? (
            <CardsScreen
              decks={decks}
              refreshDecks={refreshDecks}
              onPracticeCards={practiceCards}
              onDirtyChange={setHasUnsavedEditor}
              showToast={showToast}
            />
          ) : null}
          {section === "templates" ? (
            <TemplatesScreen onDirtyChange={setHasUnsavedEditor} showToast={showToast} />
          ) : null}
          {section === "practice" ? (
            <PracticeScreen
              decks={decks}
              settings={activeSettings}
              launchRequest={practiceLaunch}
              onLaunchConsumed={consumePracticeLaunch}
              showToast={showToast}
            />
          ) : null}
          {section === "analytics" ? <AnalyticsScreen showToast={showToast} /> : null}
          {section === "settings" ? (
            <SettingsScreen
              settings={savedSettings}
              onPreviewSettings={setActiveSettings}
              onSavedSettings={(settings: AppSettings) => {
                setSavedSettings(settings);
                setActiveSettings(settings);
                setHasUnsavedSettings(false);
              }}
              onDirtyChange={setHasUnsavedSettings}
              showToast={showToast}
            />
          ) : null}
        </AppErrorBoundary>
      </main>
      <Toast toast={toast} onDismiss={() => setToast(null)} />
    </div>
  );
}

interface NavButtonProps {
  active: boolean;
  icon: string;
  label: string;
  onClick: () => void;
}

function NavButton({ active, icon, label, onClick }: NavButtonProps) {
  return (
    <button
      type="button"
      className={active ? "nav-button active" : "nav-button"}
      aria-current={active ? "page" : undefined}
      onClick={onClick}
    >
      <span aria-hidden="true">{icon}</span>
      {label}
    </button>
  );
}

function applyThemeSettings(settings: AppSettings): void {
  const root = document.documentElement;
  root.dataset.theme = settings.theme;
  root.dataset.reduceMotion = settings.reduceMotion ? "true" : "false";
  root.style.setProperty("--ui-scale", String(settings.uiScale / 100));
  root.style.setProperty("--card-base-size", `${settings.cardBaseSize}px`);
  const uiFont = settings.theme === "console" && settings.uiFont === "system" ? "mono" : settings.uiFont;
  const cardFont = settings.theme === "console" && settings.cardFont === "system" ? "mono" : settings.cardFont;
  root.style.setProperty("--ui-font-family", resolveFont(uiFont));
  root.style.setProperty("--card-font-family", resolveFont(cardFont));
  root.style.colorScheme = settings.theme === "light" ? "light" : "dark";
}

function resolveFont(value: string): string {
  const presets: Record<string, string> = {
    system: "Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
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
