import { useCallback, useEffect, useRef, useState } from "react";

import type { ShowToast, ToastMessage } from "../components/Toast";
import type { AnalyticsRange } from "../ipc/analytics";
import { getErrorMessage } from "../ipc/common";
import { listDecks, type DeckSummary } from "../ipc/decks";
import { getSettings, type AppSettings } from "../ipc/settings";
import { useRequestLane } from "./requestLane";

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
  tagIds: string[];
  cardIds: string[];
  autoStart: boolean;
}

export interface AnalyticsLaunchRequest {
  requestId: number;
  range: AnalyticsRange;
  deckIds: string[];
  tagIds: string[];
  highlightedSessionId?: string;
}

export const DEFAULT_SETTINGS: AppSettings = {
  theme: "dark",
  uiFont: "system",
  cardFont: "system",
  uiScale: 100,
  cardBaseSize: 18,
  reduceMotion: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

export function useAppController() {
  const [section, setSection] = useState<AppSection>("decks");
  const [decks, setDecks] = useState<DeckSummary[]>([]);
  const [savedSettings, setSavedSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [activeSettings, setActiveSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [isLoading, setIsLoading] = useState(true);
  const [fatalError, setFatalError] = useState<string | null>(null);
  const [toast, setToast] = useState<ToastMessage | null>(null);
  const toastSequence = useRef(1);
  const [practiceLaunch, setPracticeLaunch] = useState<PracticeLaunchRequest | null>(null);
  const practiceRequestSequence = useRef(1);
  const [analyticsLaunch, setAnalyticsLaunch] = useState<AnalyticsLaunchRequest | null>(null);
  const analyticsRequestSequence = useRef(1);
  const [hasUnsavedCard, setHasUnsavedCard] = useState(false);
  const [hasUnsavedTemplate, setHasUnsavedTemplate] = useState(false);
  const [hasUnsavedSettings, setHasUnsavedSettings] = useState(false);
  const deckRefreshLane = useRequestLane();

  const showToast = useCallback<ShowToast>((kind, text, action) => {
    const id = toastSequence.current;
    toastSequence.current += 1;
    setToast(action ? { id, kind, text, action } : { id, kind, text });
  }, []);

  const refreshDecks = useCallback(async (): Promise<DeckSummary[]> => {
    const token = deckRefreshLane.begin();
    const loaded = await listDecks();
    if (deckRefreshLane.isCurrent(token)) setDecks(loaded);
    return loaded;
  }, [deckRefreshLane]);

  const consumePracticeLaunch = useCallback(() => setPracticeLaunch(null), []);
  const consumeAnalyticsLaunch = useCallback(() => setAnalyticsLaunch(null), []);

  useEffect(() => {
    const token = deckRefreshLane.begin();
    void Promise.all([getSettings(), listDecks()])
      .then(([loadedSettings, loadedDecks]) => {
        if (!deckRefreshLane.isCurrent(token)) return;
        setSavedSettings(loadedSettings);
        setActiveSettings(loadedSettings);
        setDecks(loadedDecks);
        setFatalError(null);
      })
      .catch((error: unknown) => {
        if (deckRefreshLane.isCurrent(token)) setFatalError(getErrorMessage(error));
      })
      .finally(() => {
        if (deckRefreshLane.isCurrent(token)) setIsLoading(false);
      });
  }, [deckRefreshLane]);

  useEffect(() => {
    if (!hasUnsavedCard && !hasUnsavedTemplate && !hasUnsavedSettings) return;
    function warnBeforeClose(event: BeforeUnloadEvent): void {
      event.preventDefault();
      event.returnValue = "";
    }
    window.addEventListener("beforeunload", warnBeforeClose);
    return () => window.removeEventListener("beforeunload", warnBeforeClose);
  }, [hasUnsavedCard, hasUnsavedSettings, hasUnsavedTemplate]);

  function confirmDiscardCard(): boolean {
    if (!hasUnsavedCard) return true;
    if (!window.confirm("Discard the unsaved changes to this flashcard?")) return false;
    setHasUnsavedCard(false);
    return true;
  }

  function confirmDiscardTemplate(): boolean {
    if (section !== "templates" || !hasUnsavedTemplate) return true;
    if (!window.confirm("Discard the unsaved changes to this card template?")) return false;
    setHasUnsavedTemplate(false);
    return true;
  }

  function confirmDiscardSettings(): boolean {
    if (section !== "settings" || !hasUnsavedSettings) return true;
    if (!window.confirm("Discard the unsaved settings preview?")) return false;
    setActiveSettings(savedSettings);
    setHasUnsavedSettings(false);
    return true;
  }

  function canLeaveCurrentSection(): boolean {
    return confirmDiscardCard() && confirmDiscardTemplate() && confirmDiscardSettings();
  }

  function nextPracticeRequest(
    deckIds: string[],
    tagIds: string[],
    cardIds: string[],
    autoStart: boolean,
  ): void {
    const requestId = practiceRequestSequence.current;
    practiceRequestSequence.current += 1;
    setPracticeLaunch({ requestId, deckIds, tagIds, cardIds, autoStart });
  }

  function navigate(next: AppSection): void {
    if (next === section && next !== "practice") return;
    if (!canLeaveCurrentSection()) return;
    if (next === "practice") nextPracticeRequest([], [], [], false);
    if (next === "analytics") {
      const requestId = analyticsRequestSequence.current;
      analyticsRequestSequence.current += 1;
      setAnalyticsLaunch({ requestId, range: "7d", deckIds: [], tagIds: [] });
    }
    if (next === "decks" || next === "cards") {
      void refreshDecks().catch((error: unknown) => showToast("error", getErrorMessage(error)));
    }
    setSection(next);
  }

  function practiceDeck(deckId: string): void {
    practiceScope([deckId], []);
  }

  function practiceScope(deckIds: string[], tagIds: string[]): void {
    if (!canLeaveCurrentSection()) return;
    nextPracticeRequest(deckIds, tagIds, [], true);
    setSection("practice");
  }

  function practiceCards(cardIds: string[]): void {
    if (cardIds.length === 0 || !canLeaveCurrentSection()) return;
    nextPracticeRequest([], [], cardIds, true);
    setSection("practice");
  }

  function viewSessionAnalytics(sessionId: string, deckIds: string[]): void {
    const requestId = analyticsRequestSequence.current;
    analyticsRequestSequence.current += 1;
    setAnalyticsLaunch({
      requestId,
      range: "7d",
      deckIds,
      tagIds: [],
      highlightedSessionId: sessionId,
    });
    setSection("analytics");
  }

  function saveSettingsLocally(settings: AppSettings): void {
    setSavedSettings(settings);
    setActiveSettings(settings);
    setHasUnsavedSettings(false);
  }

  return {
    section,
    decks,
    savedSettings,
    activeSettings,
    isLoading,
    fatalError,
    toast,
    setToast,
    practiceLaunch,
    analyticsLaunch,
    showToast,
    refreshDecks,
    consumePracticeLaunch,
    consumeAnalyticsLaunch,
    navigate,
    practiceDeck,
    practiceScope,
    practiceCards,
    viewSessionAnalytics,
    setHasUnsavedCard,
    setHasUnsavedTemplate,
    setHasUnsavedSettings,
    setActiveSettings,
    saveSettingsLocally,
  };
}
