import { useCallback, useEffect, useRef, useState } from "react";

import type { ShowToast, ToastMessage } from "../components/Toast";
import type { AnalyticsRange } from "../ipc/analytics";
import { getErrorMessage } from "../ipc/common";
import { listDecks, type DeckSummary } from "../ipc/decks";
import { listenToLifecycle } from "../ipc/platform";
import type {
  CardPracticeRequest,
  SelectedPracticeRequest,
} from "../ipc/practice";
import { getSettings, type AppSettings } from "../ipc/settings";
import {
  getUiCheckpoint,
  saveUiCheckpoint,
  type AnalyticsCheckpoint,
  type CheckpointReturnTarget,
  type PracticeCheckpoint,
  type PracticeCursorCheckpoint,
  type SaveUiCheckpointInput,
} from "../ipc/uiCheckpoint";
import {
  createAnalyticsSearchRoute,
  createFlashcardsDeckRoute,
  createFlashcardsRootRoute,
  normalizeFlashcardsWorkspace,
  type FlashcardsRoute,
  type FlashcardsWorkspaceName,
  type TransientDeckSelectionEntry,
} from "../navigation/flashcardsRoutes";
import { registerPlatformBackHandler } from "../platform/backNavigation";
import { DEFAULT_FONT_PRESET } from "../ui/appearance/appearanceTypes";
import { useConfirm } from "../ui/overlays/overlayContext";
import {
  hasActivePracticeSession,
  terminateActivePracticeSession,
} from "./practiceExitGuard";
import { useRequestLane } from "./requestLane";

export type AppSection =
  | "decks"
  | "study-guides"
  | "documents"
  | "cards"
  | "templates"
  | "analytics"
  | "settings";

type VisibleAppSection = Exclude<AppSection, "cards" | "templates">;

export type AppWorkflow = "practice";

export interface PracticeReturnTarget {
  section: AppSection;
  deckId?: string;
  flashcardsWorkspace?: FlashcardsWorkspaceName;
  restoreDeckSelection?: TransientDeckSelectionEntry[];
}

export interface DeckOpenRequest {
  requestId: number;
  deckId: string;
}

interface PracticeSelectedLaunchRequest extends SelectedPracticeRequest {
  requestId: number;
  kind: "selected";
}

interface PracticeCardsLaunchRequest extends CardPracticeRequest {
  requestId: number;
  kind: "cards";
}

export type PracticeLaunchRequest =
  | PracticeSelectedLaunchRequest
  | PracticeCardsLaunchRequest;

type PracticeLaunchInput =
  | Omit<PracticeSelectedLaunchRequest, "requestId">
  | Omit<PracticeCardsLaunchRequest, "requestId">;

export type ContextualPracticeLaunchRequest =
  | { kind: "deck"; deckId: string }
  | {
      kind: "selected-decks";
      request: SelectedPracticeRequest;
      originSelection: readonly TransientDeckSelectionEntry[];
    }
  | { kind: "scope"; deckIds: string[]; tagIds: string[] }
  | { kind: "cards"; cardIds: string[] };

export interface PracticeWorkflowState {
  kind: "practice";
  launchRequest: PracticeLaunchRequest | null;
  restoreCheckpoint: PracticeCheckpoint | null;
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
  uiFont: DEFAULT_FONT_PRESET,
  cardFont: DEFAULT_FONT_PRESET,
  uiScale: 100,
  cardBaseSize: 18,
  motionPreference: "system",
  materialPreference: "automatic",
  decorativeArtwork: true,
  celebrationsEnabled: true,
  tutorialsEnabled: false,
  confirmDeletions: true,
  practiceShuffle: true,
  desiredRetention: 0.9,
  dailyNewLimit: 20,
  dailyReviewLimit: 200,
};

interface NormalizedDestination {
  section: VisibleAppSection;
  flashcardsWorkspace?: FlashcardsWorkspaceName;
}

export function useAppController() {
  const [section, setSection] = useState<AppSection>("decks");
  const [flashcardsRoute, setFlashcardsRoute] = useState<FlashcardsRoute>(() =>
    createFlashcardsRootRoute("decks", 0),
  );
  const flashcardsRouteSequence = useRef(1);
  const [decks, setDecks] = useState<DeckSummary[]>([]);
  const [savedSettings, setSavedSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [activeSettings, setActiveSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [isLoading, setIsLoading] = useState(true);
  const [fatalError, setFatalError] = useState<string | null>(null);
  const [toast, setToast] = useState<ToastMessage | null>(null);
  const toastSequence = useRef(1);
  const [practiceWorkflow, setPracticeWorkflow] = useState<PracticeWorkflowState | null>(null);
  const practiceCursorRef = useRef<PracticeCursorCheckpoint | null>(null);
  const [practiceCursorRevision, setPracticeCursorRevision] = useState(0);
  const practiceRequestSequence = useRef(1);
  const [analyticsLaunch, setAnalyticsLaunch] = useState<AnalyticsLaunchRequest | null>(null);
  const analyticsCheckpointRef = useRef<AnalyticsCheckpoint | null>(null);
  const analyticsRequestSequence = useRef(1);
  const [hasUnsavedCard, setHasUnsavedCard] = useState(false);
  const [hasUnsavedTemplate, setHasUnsavedTemplate] = useState(false);
  const [hasUnsavedSettings, setHasUnsavedSettings] = useState(false);
  const deckRefreshLane = useRequestLane();
  const confirm = useConfirm();

  const flashcardsWorkspace = flashcardsRoute.workspace;
  const deckOpenRequest: DeckOpenRequest | null =
    flashcardsRoute.view === "deck"
      ? { requestId: flashcardsRoute.requestId, deckId: flashcardsRoute.deckId }
      : null;
  const checkpointStateRef = useRef<
    Pick<CheckpointSnapshotState, "section" | "flashcardsRoute" | "practiceWorkflow">
  >({ section, flashcardsRoute, practiceWorkflow });
  checkpointStateRef.current = { section, flashcardsRoute, practiceWorkflow };

  function nextFlashcardsRequestId(): number {
    const requestId = flashcardsRouteSequence.current;
    flashcardsRouteSequence.current += 1;
    return requestId;
  }

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

  const consumePracticeLaunch = useCallback(() => {
    setPracticeWorkflow((current) =>
      current ? { ...current, launchRequest: null } : current,
    );
  }, []);
  const consumePracticeRestore = useCallback(() => {
    setPracticeWorkflow((current) =>
      current ? { ...current, restoreCheckpoint: null } : current,
    );
  }, []);
  const consumeAnalyticsLaunch = useCallback(() => setAnalyticsLaunch(null), []);
  const reportPracticeCheckpoint = useCallback(
    (checkpoint: PracticeCursorCheckpoint | null) => {
      practiceCursorRef.current = checkpoint;
      setPracticeCursorRevision((current) => current + 1);
    },
    [],
  );

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
          const restoredSection = restored.section;
          const restoredWorkspace =
            restoredSection === "decks"
              ? normalizeFlashcardsWorkspace(
                  restoredCheckpoint.flashcardsWorkspace ?? restored.flashcardsWorkspace,
                )
              : "decks";
          const requestId = nextFlashcardsRequestId();
          const restoredDeckId = restoredCheckpoint.deckId;
          if (
            restoredSection === "decks" &&
            restoredDeckId &&
            loadedDecks.some((deck) => deck.id === restoredDeckId)
          ) {
            setFlashcardsRoute(createFlashcardsDeckRoute(restoredDeckId, requestId));
          } else {
            setFlashcardsRoute(createFlashcardsRootRoute(restoredWorkspace, requestId));
          }
          setSection(restoredSection);
          if (restoredCheckpoint.practice) {
            practiceCursorRef.current = {
              sessionId: restoredCheckpoint.practice.sessionId,
              position: restoredCheckpoint.practice.position,
              revealed: restoredCheckpoint.practice.revealed,
            };
            setPracticeWorkflow({
              kind: "practice",
              launchRequest: null,
              restoreCheckpoint: restoredCheckpoint.practice,
              returnTarget: normalizeRestoredReturnTarget(
                restoredCheckpoint.practice.returnTarget,
                loadedDecks,
              ),
            });
          }
          if (restoredCheckpoint.analytics) {
            analyticsCheckpointRef.current = restoredCheckpoint.analytics;
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
    if (isLoading) return;
    const timeout = window.setTimeout(() => {
      void saveUiCheckpoint(
        createCheckpointSnapshot({
          section,
          flashcardsRoute,
          practiceWorkflow,
          practiceCursor: practiceCursorRef.current,
          analytics: analyticsCheckpointRef.current,
        }),
      ).catch((error: unknown) => {
        console.error(
          "Unable to save the device navigation checkpoint:",
          getErrorMessage(error),
        );
      });
    }, 300);
    return () => window.clearTimeout(timeout);
  }, [
    flashcardsRoute,
    isLoading,
    practiceCursorRevision,
    practiceWorkflow,
    section,
  ]);

  useEffect(() => {
    if (isLoading) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listenToLifecycle((snapshot) => {
      if (
        snapshot.phase === "background" ||
        snapshot.phase === "suspending" ||
        snapshot.phase === "termination_requested"
      ) {
        const current = checkpointStateRef.current;
        void saveUiCheckpoint(
          createCheckpointSnapshot({
            ...current,
            practiceCursor: practiceCursorRef.current,
            analytics: analyticsCheckpointRef.current,
          }),
        ).catch((error: unknown) => {
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
  }, [isLoading]);

  useEffect(() => {
    if (section === "decks" && !practiceWorkflow) return;
    return registerPlatformBackHandler(() => {
      void (practiceWorkflow ? returnPracticeToOrigin() : navigate("decks"));
      return true;
    });
  }, [
    hasUnsavedCard,
    hasUnsavedSettings,
    hasUnsavedTemplate,
    savedSettings,
    practiceWorkflow,
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

  async function confirmPracticeTermination(): Promise<boolean> {
    if (!practiceWorkflow || !hasActivePracticeSession()) {
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

  function capturePracticeReturnTarget(): PracticeReturnTarget {
    if (section !== "decks") return { section };
    if (flashcardsRoute.view === "deck") {
      return {
        section: "decks",
        deckId: flashcardsRoute.deckId,
        flashcardsWorkspace: "decks",
      };
    }
    return { section: "decks", flashcardsWorkspace: flashcardsRoute.workspace };
  }

  function openPracticeWorkflow(
    launchRequest: PracticeLaunchInput,
    returnTarget: PracticeReturnTarget | null,
  ): void {
    const requestId = practiceRequestSequence.current;
    practiceRequestSequence.current += 1;
    practiceCursorRef.current = null;
    setPracticeWorkflow({
      kind: "practice",
      launchRequest: { ...launchRequest, requestId },
      restoreCheckpoint: null,
      returnTarget,
    });
  }

  async function launchPractice(request: ContextualPracticeLaunchRequest): Promise<void> {
    if (request.kind === "cards" && request.cardIds.length === 0) return;
    if (!(await canLeaveCurrentSection())) return;
    const returnTarget = capturePracticeReturnTarget();
    if (request.kind === "deck") {
      openPracticeWorkflow(
        {
          kind: "selected",
          deckIds: [request.deckId],
          tagIds: [],
          shuffle: activeSettings.practiceShuffle,
          mode: "extra",
        },
        returnTarget,
      );
      return;
    }
    if (request.kind === "selected-decks") {
      openPracticeWorkflow(
        { kind: "selected", ...request.request },
        {
          section: "decks",
          flashcardsWorkspace: "decks",
          restoreDeckSelection: [...request.originSelection],
        },
      );
      return;
    }
    if (request.kind === "scope") {
      openPracticeWorkflow(
        {
          kind: "selected",
          deckIds: request.deckIds,
          tagIds: request.tagIds,
          shuffle: activeSettings.practiceShuffle,
          mode: "scheduled",
        },
        returnTarget,
      );
      return;
    }
    const cardIds = deduplicateCardIds(request.cardIds);
    if (cardIds.length === 0) return;
    openPracticeWorkflow(
      { kind: "cards", cardIds, shuffle: activeSettings.practiceShuffle },
      returnTarget,
    );
  }

  async function navigate(next: AppSection): Promise<void> {
    const destination = normalizeDestination(next);
    const leavingPracticeWorkflow = practiceWorkflow !== null;
    if (!(await confirmPracticeTermination())) return;
    const alreadyAtNonFlashcardsRoot =
      destination.section === section && destination.section !== "decks";
    if (alreadyAtNonFlashcardsRoot && !leavingPracticeWorkflow) return;
    if (!(await canLeaveCurrentSection())) return;
    if (leavingPracticeWorkflow) {
      practiceCursorRef.current = null;
      setPracticeWorkflow(null);
    }
    if (
      destination.section !== "decks" &&
      flashcardsRoute.view === "root" &&
      flashcardsRoute.analyticsSearchLaunch
    ) {
      setFlashcardsRoute(createFlashcardsRootRoute("decks", nextFlashcardsRequestId()));
    }
    if (destination.section === "analytics") {
      const requestId = analyticsRequestSequence.current;
      analyticsRequestSequence.current += 1;
      const analytics = { range: "7d" as const, deckIds: [], tagIds: [] };
      analyticsCheckpointRef.current = analytics;
      setAnalyticsLaunch({ requestId, ...analytics });
    }
    if (destination.section === "decks") {
      setFlashcardsRoute(
        createFlashcardsRootRoute(
          destination.flashcardsWorkspace ?? "decks",
          nextFlashcardsRequestId(),
        ),
      );
      void refreshDecks().catch((error: unknown) =>
        showToast("error", getErrorMessage(error)),
      );
    }
    setSection(destination.section);
  }

  async function navigateFlashcardsWorkspace(
    next: FlashcardsWorkspaceName,
  ): Promise<void> {
    if (!(await canLeaveCurrentSection())) return;
    setSection("decks");
    setFlashcardsRoute(createFlashcardsRootRoute(next, nextFlashcardsRequestId()));
    if (next === "decks" || next === "search") {
      void refreshDecks().catch((error: unknown) =>
        showToast("error", getErrorMessage(error)),
      );
    }
  }

  function openFlashcardsDeck(
    deckId: string,
    returnDeckSelection: readonly TransientDeckSelectionEntry[] = [],
  ): void {
    setSection("decks");
    setFlashcardsRoute(
      createFlashcardsDeckRoute(
        deckId,
        nextFlashcardsRequestId(),
        returnDeckSelection,
      ),
    );
  }

  function openAnalyticsTagSearch(tagId: string): void {
    const requestId = nextFlashcardsRequestId();
    setSection("decks");
    setFlashcardsRoute(createAnalyticsSearchRoute(tagId, requestId));
    void refreshDecks().catch((error: unknown) =>
      showToast("error", getErrorMessage(error)),
    );
  }

  async function returnToFlashcardsLibrary(): Promise<void> {
    const returnDeckSelection =
      flashcardsRoute.view === "deck"
        ? flashcardsRoute.returnDeckSelection
        : undefined;
    let reconciledSelection: TransientDeckSelectionEntry[] | undefined;
    try {
      const currentDecks = await refreshDecks();
      reconciledSelection = reconcileReturnedDeckSelection(
        returnDeckSelection,
        currentDecks,
      );
    } catch (error: unknown) {
      showToast("error", getErrorMessage(error));
    }
    setFlashcardsRoute(
      createFlashcardsRootRoute(
        "decks",
        nextFlashcardsRequestId(),
        reconciledSelection,
      ),
    );
  }

  function practiceDeck(deckId: string): Promise<void> {
    return launchPractice({ kind: "deck", deckId });
  }

  function practiceScope(deckIds: string[], tagIds: string[]): Promise<void> {
    return launchPractice({ kind: "scope", deckIds, tagIds });
  }

  function practiceSelected(
    request: SelectedPracticeRequest,
    originSelection: readonly TransientDeckSelectionEntry[],
  ): Promise<void> {
    return launchPractice({ kind: "selected-decks", request, originSelection });
  }

  function practiceCards(cardIds: string[]): Promise<void> {
    return launchPractice({ kind: "cards", cardIds });
  }

  async function returnFromPractice(target: PracticeReturnTarget): Promise<void> {
    practiceCursorRef.current = null;
    setPracticeWorkflow(null);
    let currentDecks = decks;
    if (target.section === "decks") {
      try {
        currentDecks = await refreshDecks();
      } catch (error: unknown) {
        showToast("error", `Deck selection could not be refreshed: ${getErrorMessage(error)}`);
      }
    }
    const restoredTarget = normalizeRestoredReturnTarget(
      toCheckpointReturnTarget(target),
      currentDecks,
    );
    const destination = normalizeDestination(restoredTarget.section);
    const workspace =
      restoredTarget.flashcardsWorkspace ?? destination.flashcardsWorkspace ?? "decks";
    if (destination.section === "decks") {
      setFlashcardsRoute(
        restoredTarget.deckId
          ? createFlashcardsDeckRoute(restoredTarget.deckId, nextFlashcardsRequestId())
          : createFlashcardsRootRoute(
              workspace,
              nextFlashcardsRequestId(),
              reconcileReturnedDeckSelection(target.restoreDeckSelection, currentDecks),
            ),
      );
    } else if (destination.section === "analytics") {
      const analytics = analyticsCheckpointRef.current ?? {
        range: "7d",
        deckIds: [],
        tagIds: [],
      };
      const requestId = analyticsRequestSequence.current;
      analyticsRequestSequence.current += 1;
      setAnalyticsLaunch({ requestId, ...analytics });
    }
    setSection(destination.section);
  }

  async function returnPracticeToOrigin(): Promise<void> {
    const target = practiceWorkflow?.returnTarget;
    if (!practiceWorkflow || !(await confirmPracticeTermination())) return;
    if (!(await canLeaveCurrentSection())) return;
    await returnFromPractice(
      target ?? { section: "decks", flashcardsWorkspace: "decks" },
    );
  }

  async function choosePracticeDecks(): Promise<void> {
    const restoreDeckSelection = practiceWorkflow?.returnTarget?.restoreDeckSelection;
    await returnFromPractice({
      section: "decks",
      flashcardsWorkspace: "decks",
      restoreDeckSelection,
    });
  }

  function viewSessionAnalytics(sessionId: string, deckIds: string[]): void {
    practiceCursorRef.current = null;
    setPracticeWorkflow(null);
    const requestId = analyticsRequestSequence.current;
    analyticsRequestSequence.current += 1;
    const analytics: AnalyticsCheckpoint = {
      range: "7d",
      deckIds,
      tagIds: [],
    };
    analyticsCheckpointRef.current = analytics;
    setAnalyticsLaunch({
      requestId,
      ...analytics,
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
    activeWorkflow: practiceWorkflow?.kind ?? null,
    practiceWorkflow,
    flashcardsRoute,
    flashcardsWorkspace,
    decks,
    savedSettings,
    activeSettings,
    isLoading,
    fatalError,
    toast,
    setToast,
    deckOpenRequest,
    analyticsLaunch,
    showToast,
    refreshDecks,
    consumePracticeLaunch,
    consumePracticeRestore,
    reportPracticeCheckpoint,
    consumeAnalyticsLaunch,
    navigate,
    navigateFlashcardsWorkspace,
    openFlashcardsDeck,
    openAnalyticsTagSearch,
    returnToFlashcardsLibrary,
    practiceDeck,
    practiceScope,
    practiceSelected,
    practiceCards,
    launchPractice,
    returnFromPractice,
    choosePracticeDecks,
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

interface CheckpointSnapshotState {
  section: AppSection;
  flashcardsRoute: FlashcardsRoute;
  practiceWorkflow: PracticeWorkflowState | null;
  practiceCursor: PracticeCursorCheckpoint | null;
  analytics: AnalyticsCheckpoint | null;
}

function createCheckpointSnapshot(state: CheckpointSnapshotState): SaveUiCheckpointInput {
  const destination = normalizeDestination(state.section);
  const visibleTarget: PracticeReturnTarget =
    destination.section === "decks"
      ? state.flashcardsRoute.view === "deck"
        ? { section: "decks", flashcardsWorkspace: "decks", deckId: state.flashcardsRoute.deckId }
        : { section: "decks", flashcardsWorkspace: state.flashcardsRoute.workspace }
      : { section: destination.section };
  const returnTarget = state.practiceWorkflow?.returnTarget ?? visibleTarget;
  const practice =
    state.practiceWorkflow && state.practiceCursor
      ? {
          ...state.practiceCursor,
          returnTarget: toCheckpointReturnTarget(returnTarget),
        }
      : undefined;
  return {
    section: destination.section,
    flashcardsWorkspace:
      destination.section === "decks" ? state.flashcardsRoute.workspace : undefined,
    deckId:
      destination.section === "decks" && state.flashcardsRoute.view === "deck"
        ? state.flashcardsRoute.deckId
        : undefined,
    practice,
    analytics: destination.section === "analytics" ? state.analytics ?? undefined : undefined,
  };
}

function toCheckpointReturnTarget(target: PracticeReturnTarget): CheckpointReturnTarget {
  const destination = normalizeDestination(target.section);
  if (destination.section !== "decks") {
    return {
      section: destination.section,
      flashcardsWorkspace: null,
      deckId: null,
    };
  }
  return {
    section: "decks",
    flashcardsWorkspace:
      target.flashcardsWorkspace ?? destination.flashcardsWorkspace ?? "decks",
    deckId: target.deckId ?? null,
  };
}

function normalizeRestoredReturnTarget(
  target: CheckpointReturnTarget,
  decks: DeckSummary[],
): PracticeReturnTarget {
  if (target.section !== "decks") return { section: target.section };
  if (target.deckId && decks.some((deck) => deck.id === target.deckId)) {
    return {
      section: "decks",
      flashcardsWorkspace: "decks",
      deckId: target.deckId,
    };
  }
  return {
    section: "decks",
    flashcardsWorkspace: target.flashcardsWorkspace ?? "decks",
  };
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
    tutorialsEnabled: settings.tutorialsEnabled ?? false,
    confirmDeletions: settings.confirmDeletions,
    practiceShuffle: settings.practiceShuffle,
    desiredRetention: settings.desiredRetention,
    dailyNewLimit: settings.dailyNewLimit,
    dailyReviewLimit: settings.dailyReviewLimit,
    aiReasoningEffort: settings.aiReasoningEffort,
  };
}

function deduplicateCardIds(cardIds: readonly string[]): string[] {
  const seen = new Set<string>();
  return cardIds.filter((cardId) => {
    if (seen.has(cardId)) return false;
    seen.add(cardId);
    return true;
  });
}

function reconcileReturnedDeckSelection(
  restored: readonly TransientDeckSelectionEntry[] | undefined,
  decks: readonly DeckSummary[],
): TransientDeckSelectionEntry[] | undefined {
  if (!restored || restored.length === 0) return undefined;
  const currentById = new Map(decks.map((deck) => [deck.id, deck]));
  const valid = restored.filter((entry) => {
    const deck = currentById.get(entry.id);
    return Boolean(
      deck &&
        deck.revision === entry.revision &&
        deck.archivedAt === null,
    );
  });
  return valid.length > 0 ? valid : undefined;
}
