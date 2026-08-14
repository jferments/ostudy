import { useCallback, useEffect, useRef, useState } from "react";

import type { ShowToast, ToastMessage } from "../components/Toast";
import type { AnalyticsRange } from "../ipc/analytics";
import { getErrorMessage } from "../ipc/common";
import { listDecks, type DeckSummary } from "../ipc/decks";
import { listenToLifecycle } from "../ipc/platform";
import { getSettings, type AppSettings } from "../ipc/settings";
import {
  getUiCheckpoint,
  saveUiCheckpoint,
  type PracticeCheckpoint,
} from "../ipc/uiCheckpoint";
import type { FlashcardsWorkspaceName } from "../navigation/flashcardsRoutes";
import { registerPlatformBackHandler } from "../platform/backNavigation";
import { useConfirm } from "../ui/overlays/overlayContext";
import {
  hasActivePracticeSession,
  terminateActivePracticeSession,
} from "./practiceExitGuard";
import { useRequestLane } from "./requestLane";

export type AppSection =
  | "decks"
  | "documents"
  | "cards"
  | "templates"
  | "practice"
  | "analytics"
  | "settings";

export interface PracticeReturnTarget {
  section: Exclude<AppSection, "practice">;
  deckId?: string;
  flashcardsWorkspace?: FlashcardsWorkspaceName;
}

export interface DeckOpenRequest {
  requestId: number;
  deckId: string;
}

export interface PracticeLaunchRequest {
  requestId: number;
  deckIds: string[];
  tagIds: string[];
  cardIds: string[];
  autoStart: boolean;
  mode: "scheduled" | "extra";
  returnTarget: PracticeReturnTarget | null;
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
  motionPreference: "system",
  materialPreference: "automatic",
  decorativeArtwork: true,
  celebrationsEnabled: true,
  tutorialsEnabled: true,
  deckViewDensity: "comfortable",
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

interface NormalizedDestination {
  section: AppSection;
  flashcardsWorkspace?: FlashcardsWorkspaceName;
}

export function useAppController() {
  const [section, setSection] = useState<AppSection>("decks");
  const [flashcardsWorkspace, setFlashcardsWorkspace] =
    useState<FlashcardsWorkspaceName>("decks");
  const [decks, setDecks] = useState<DeckSummary[]>([]);
  const [savedSettings, setSavedSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [activeSettings, setActiveSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [isLoading, setIsLoading] = useState(true);
  const [fatalError, setFatalError] = useState<string | null>(null);
  const [toast, setToast] = useState<ToastMessage | null>(null);
  const toastSequence = useRef(1);
  const [practiceLaunch, setPracticeLaunch] = useState<PracticeLaunchRequest | null>(null);
  const [practiceRestore, setPracticeRestore] = useState<PracticeCheckpoint | null>(null);
  const practiceRequestSequence = useRef(1);
  const [deckOpenRequest, setDeckOpenRequest] = useState<DeckOpenRequest | null>(null);
  const deckOpenRequestSequence = useRef(1);
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
  const consumePracticeRestore = useCallback(() => setPracticeRestore(null), []);
  const consumeDeckOpenRequest = useCallback(() => setDeckOpenRequest(null), []);
  const consumeAnalyticsLaunch = useCallback(() => setAnalyticsLaunch(null), []);

  useEffect(() => {
    const token = deckRefreshLane.begin();
    const checkpoint = getUiCheckpoint().catch((error: unknown) => {
      console.error(
        "Unable to restore the device navigation checkpoint:",
        getErrorMessage(error),
      );
      return null;
    });
    void Promise.all([getSettings(), listDecks(), checkpoint])
      .then(([loadedSettings, loadedDecks, restoredCheckpoint]) => {
        if (!deckRefreshLane.isCurrent(token)) return;
        const normalizedSettings = normalizeSettings(loadedSettings);
        setSavedSettings(normalizedSettings);
        setActiveSettings(normalizedSettings);
        setDecks(loadedDecks);
        if (restoredCheckpoint) {
          const restored = normalizeDestination(restoredCheckpoint.section);
          const restoredSection =
            restored.section === "practice" && !restoredCheckpoint.practice
              ? "decks"
              : restored.section;
          const restoredWorkspace =
            restoredSection === "decks"
              ? restoredCheckpoint.flashcardsWorkspace ??
                restored.flashcardsWorkspace ??
                "decks"
              : "decks";
          setSection(restoredSection);
          setFlashcardsWorkspace(restoredWorkspace);
          if (
            restoredCheckpoint.deckId &&
            loadedDecks.some((deck) => deck.id === restoredCheckpoint.deckId)
          ) {
            setFlashcardsWorkspace("decks");
            setDeckOpenRequest({
              requestId: deckOpenRequestSequence.current,
              deckId: restoredCheckpoint.deckId,
            });
            deckOpenRequestSequence.current += 1;
          }
          if (restoredCheckpoint.practice) setPracticeRestore(restoredCheckpoint.practice);
          if (restoredCheckpoint.analytics) {
            setAnalyticsLaunch({
              requestId: analyticsRequestSequence.current,
              range: restoredCheckpoint.analytics.range,
              deckIds: restoredCheckpoint.analytics.deckIds,
              tagIds: restoredCheckpoint.analytics.tagIds,
            });
            analyticsRequestSequence.current += 1;
          }
        }
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
    if (isLoading || section === "practice") return;
    const analytics =
      section === "analytics" && analyticsLaunch
        ? {
            range: analyticsLaunch.range,
            deckIds: analyticsLaunch.deckIds,
            tagIds: analyticsLaunch.tagIds,
          }
        : undefined;
    void saveUiCheckpoint({
      section,
      flashcardsWorkspace: section === "decks" ? flashcardsWorkspace : undefined,
      deckId: deckOpenRequest?.deckId,
      analytics,
    }).catch((error: unknown) => {
      console.error("Unable to save the device navigation checkpoint:", getErrorMessage(error));
    });
  }, [
    analyticsLaunch,
    deckOpenRequest?.deckId,
    flashcardsWorkspace,
    isLoading,
    section,
  ]);

  useEffect(() => {
    if (isLoading) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listenToLifecycle((snapshot) => {
      if (
        section !== "practice" &&
        (snapshot.phase === "background" ||
          snapshot.phase === "suspending" ||
          snapshot.phase === "termination_requested")
      ) {
        void saveUiCheckpoint({
          section,
          flashcardsWorkspace: section === "decks" ? flashcardsWorkspace : undefined,
          deckId: deckOpenRequest?.deckId,
        }).catch((error: unknown) => {
          console.error(
            "Unable to save the lifecycle navigation checkpoint:",
            getErrorMessage(error),
          );
        });
      }
    })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch((error: unknown) => {
        if (!disposed) {
          console.error("Unable to observe application lifecycle:", getErrorMessage(error));
        }
      });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [deckOpenRequest?.deckId, flashcardsWorkspace, isLoading, section]);

  useEffect(() => {
    if (section === "decks") return;
    return registerPlatformBackHandler(() => {
      void navigate("decks");
      return true;
    });
  }, [
    hasUnsavedCard,
    hasUnsavedSettings,
    hasUnsavedTemplate,
    savedSettings,
    section,
  ]);

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
    const templateWorkspaceOpen =
      section === "templates" ||
      (section === "decks" && flashcardsWorkspace === "templates");
    if (!templateWorkspaceOpen || !hasUnsavedTemplate) return true;
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

  async function confirmPracticeTermination(next: AppSection): Promise<boolean> {
    if (section !== "practice" || next === "practice" || !hasActivePracticeSession()) {
      return true;
    }
    const accepted = await confirm({
      title: "Terminate this practice session?",
      description:
        "Cards you already reviewed are saved and will keep their updated schedules and review history. Leaving now ends this session without opening post-review analytics.",
      confirmLabel: "Yes, terminate",
      destructive: true,
    });
    if (!accepted) return false;
    const terminated = await terminateActivePracticeSession();
    if (!terminated) {
      showToast(
        "error",
        "The practice session could not be ended safely. Resolve the practice error before leaving.",
      );
    }
    return terminated;
  }

  function nextPracticeRequest(
    deckIds: string[],
    tagIds: string[],
    cardIds: string[],
    autoStart: boolean,
    mode: "scheduled" | "extra",
    returnTarget: PracticeReturnTarget | null,
  ): void {
    const requestId = practiceRequestSequence.current;
    practiceRequestSequence.current += 1;
    setPracticeLaunch({ requestId, deckIds, tagIds, cardIds, autoStart, mode, returnTarget });
  }

  async function navigate(next: AppSection): Promise<void> {
    const destination = normalizeDestination(next);
    const alreadyThere =
      destination.section === section &&
      (destination.section !== "decks" ||
        destination.flashcardsWorkspace === undefined ||
        destination.flashcardsWorkspace === flashcardsWorkspace);
    if (alreadyThere && destination.section !== "practice") return;
    if (!(await confirmPracticeTermination(destination.section))) return;
    if (!(await canLeaveCurrentSection())) return;
    if (destination.section === "practice") {
      setPracticeRestore(null);
      nextPracticeRequest([], [], [], false, "scheduled", null);
    }
    if (destination.section === "analytics") {
      const requestId = analyticsRequestSequence.current;
      analyticsRequestSequence.current += 1;
      setAnalyticsLaunch({ requestId, range: "7d", deckIds: [], tagIds: [] });
    }
    if (destination.section === "decks") {
      setFlashcardsWorkspace(destination.flashcardsWorkspace ?? "decks");
      void refreshDecks().catch((error: unknown) =>
        showToast("error", getErrorMessage(error)),
      );
    }
    setSection(destination.section);
  }

  async function navigateFlashcardsWorkspace(
    next: FlashcardsWorkspaceName,
  ): Promise<void> {
    if (section === "decks" && next === flashcardsWorkspace) return;
    if (!(await canLeaveCurrentSection())) return;
    setSection("decks");
    setFlashcardsWorkspace(next);
    if (next === "decks" || next === "search" || next === "create") {
      void refreshDecks().catch((error: unknown) =>
        showToast("error", getErrorMessage(error)),
      );
    }
  }

  async function practiceDeck(
    deckId: string,
    origin: "library" | "detail" = "library",
  ): Promise<void> {
    if (!(await canLeaveCurrentSection())) return;
    setPracticeRestore(null);
    nextPracticeRequest(
      [deckId],
      [],
      [],
      true,
      "extra",
      origin === "detail"
        ? { section: "decks", deckId, flashcardsWorkspace: "decks" }
        : { section: "decks", flashcardsWorkspace: "decks" },
    );
    setSection("practice");
  }

  async function practiceScope(deckIds: string[], tagIds: string[]): Promise<void> {
    if (!(await canLeaveCurrentSection())) return;
    const returnTarget: PracticeReturnTarget | null =
      section === "practice"
        ? null
        : {
            section,
            ...(section === "decks" ? { flashcardsWorkspace } : {}),
          };
    setPracticeRestore(null);
    nextPracticeRequest(deckIds, tagIds, [], true, "scheduled", returnTarget);
    setSection("practice");
  }

  async function practiceCards(cardIds: string[]): Promise<void> {
    if (cardIds.length === 0 || !(await canLeaveCurrentSection())) return;
    const returnTarget: PracticeReturnTarget | null =
      section === "practice"
        ? null
        : {
            section,
            ...(section === "decks" ? { flashcardsWorkspace } : {}),
          };
    setPracticeRestore(null);
    nextPracticeRequest([], [], cardIds, true, "scheduled", returnTarget);
    setSection("practice");
  }

  function returnFromPractice(target: PracticeReturnTarget): void {
    setPracticeRestore(null);
    const destination = normalizeDestination(target.section);
    const workspace =
      target.flashcardsWorkspace ?? destination.flashcardsWorkspace ?? "decks";
    if (destination.section === "decks" && target.deckId) {
      const requestId = deckOpenRequestSequence.current;
      deckOpenRequestSequence.current += 1;
      setDeckOpenRequest({ requestId, deckId: target.deckId });
    }
    if (destination.section === "decks") {
      setFlashcardsWorkspace(target.deckId ? "decks" : workspace);
      void refreshDecks().catch((error: unknown) =>
        showToast("error", getErrorMessage(error)),
      );
    }
    setSection(destination.section);
  }

  function viewSessionAnalytics(sessionId: string, deckIds: string[]): void {
    setPracticeRestore(null);
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
    const normalized = normalizeSettings(settings);
    setSavedSettings(normalized);
    setActiveSettings(normalized);
    // Settings owns a combined dirty state. Saving ordinary preferences must not
    // clear an unrelated AI profile or credential draft.
  }

  return {
    section,
    flashcardsWorkspace,
    decks,
    savedSettings,
    activeSettings,
    isLoading,
    fatalError,
    toast,
    setToast,
    practiceLaunch,
    practiceRestore,
    deckOpenRequest,
    analyticsLaunch,
    showToast,
    refreshDecks,
    consumePracticeLaunch,
    consumePracticeRestore,
    consumeDeckOpenRequest,
    consumeAnalyticsLaunch,
    navigate,
    navigateFlashcardsWorkspace,
    practiceDeck,
    practiceScope,
    practiceCards,
    returnFromPractice,
    viewSessionAnalytics,
    setHasUnsavedCard,
    setHasUnsavedTemplate,
    setHasUnsavedSettings,
    setActiveSettings,
    saveSettingsLocally,
  };
}

function normalizeDestination(section: AppSection): NormalizedDestination {
  if (section === "cards") return { section: "decks", flashcardsWorkspace: "search" };
  if (section === "templates") {
    return { section: "decks", flashcardsWorkspace: "templates" };
  }
  return { section };
}

function normalizeSettings(settings: AppSettings): AppSettings {
  const motionPreference =
    settings.motionPreference ??
    (settings.animationsEnabled === false
      ? "none"
      : settings.reduceMotion
        ? "reduced"
        : "system");
  return {
    theme: settings.theme,
    uiFont: settings.uiFont,
    cardFont: settings.cardFont,
    uiScale: settings.uiScale,
    cardBaseSize: settings.cardBaseSize,
    motionPreference,
    materialPreference: settings.materialPreference ?? "automatic",
    decorativeArtwork: settings.decorativeArtwork ?? true,
    celebrationsEnabled: settings.celebrationsEnabled ?? true,
    tutorialsEnabled: settings.tutorialsEnabled ?? true,
    deckViewDensity: settings.deckViewDensity ?? "comfortable",
    confirmDeletions: settings.confirmDeletions,
    practiceShuffle: settings.practiceShuffle,
    desiredRetention: settings.desiredRetention,
    dailyNewLimit: settings.dailyNewLimit,
    dailyReviewLimit: settings.dailyReviewLimit,
  };
}
