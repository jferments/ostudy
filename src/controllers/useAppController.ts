import { useCallback, useEffect, useRef, useState } from "react";

import type { ShowToast, ToastMessage } from "../components/Toast";
import type { AnalyticsRange } from "../ipc/analytics";
import { getErrorMessage } from "../ipc/common";
import { listDecks, type DeckSummary } from "../ipc/decks";
import { getSettings, type AppSettings } from "../ipc/settings";
import { useConfirm } from "../ui/overlays/overlayContext";
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
  const confirm = useConfirm();

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

  async function confirmDiscardCard(): Promise<boolean> {
    if (!hasUnsavedCard) return true;
    const accepted = await confirm({
      title: "Discard flashcard changes?",
      description: "The unsaved changes to this flashcard will be lost.",
      confirmLabel: "Discard changes",
      destructive: true,
    });
    if (!accepted) return false;
    setHasUnsavedCard(false);
    return true;
  }

  async function confirmDiscardTemplate(): Promise<boolean> {
    if (section !== "templates" || !hasUnsavedTemplate) return true;
    const accepted = await confirm({
      title: "Discard template changes?",
      description: "The unsaved changes to this card template will be lost.",
      confirmLabel: "Discard changes",
      destructive: true,
    });
    if (!accepted) return false;
    setHasUnsavedTemplate(false);
    return true;
  }

  async function confirmDiscardSettings(): Promise<boolean> {
    if (section !== "settings" || !hasUnsavedSettings) return true;
    const accepted = await confirm({
      title: "Discard settings changes?",
      description:
        "Unsaved preference previews, AI provider metadata, and credential entry will be discarded. Saved credentials are unchanged.",
      confirmLabel: "Discard changes",
      destructive: true,
    });
    if (!accepted) return false;
    setActiveSettings(savedSettings);
    setHasUnsavedSettings(false);
    return true;
  }

  async function canLeaveCurrentSection(): Promise<boolean> {
    if (!(await confirmDiscardCard())) return false;
    if (!(await confirmDiscardTemplate())) return false;
    return confirmDiscardSettings();
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

  async function navigate(next: AppSection): Promise<void> {
    if (next === section && next !== "practice") return;
    if (!(await canLeaveCurrentSection())) return;
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

  async function practiceDeck(deckId: string): Promise<void> {
    await practiceScope([deckId], []);
  }

  async function practiceScope(deckIds: string[], tagIds: string[]): Promise<void> {
    if (!(await canLeaveCurrentSection())) return;
    nextPracticeRequest(deckIds, tagIds, [], true);
    setSection("practice");
  }

  async function practiceCards(cardIds: string[]): Promise<void> {
    if (cardIds.length === 0 || !(await canLeaveCurrentSection())) return;
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
    // Settings owns a combined dirty state. Saving ordinary preferences must not
    // clear an unrelated AI profile or credential draft.
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
