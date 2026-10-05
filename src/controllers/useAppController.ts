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
  isLibrarySection,
  type LibraryScope,
  type LibrarySection,
} from "../navigation/appDestinations";
import {
  createAnalyticsSearchRoute,
  createFlashcardsDeckRoute,
  createFlashcardsRootRoute,
  createSubjectFocusRoute,
  normalizeFlashcardsWorkspace,
  type CurrentFlashcardsWorkspaceName,
  type FlashcardsRoute,
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

/**
 * Home and Review are destinations of their own. decks, study-guides and
 * documents are Library's views and analytics is Progress; they keep their
 * ids so saved checkpoints and launch requests still name them.
 */
export type AppSection =
  | "home"
  | "decks"
  | "study-guides"
  | "documents"
  | "review"
  | "tutor"
  | "analytics"
  | "settings";

export type AppWorkflow = "practice";

export interface PracticeReturnTarget {
  section: AppSection;
  deckId?: string;
  flashcardsWorkspace?: CurrentFlashcardsWorkspaceName;
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
  | { kind: "subject"; subjectId: string }
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
  newDayStartHour: 4,
};

export function useAppController() {
  const [section, setSection] = useState<AppSection>("home");
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
  // Sessions that reached their summary in this run: their saved reviews changed what is due.
  const [finishedPracticeSessions, setFinishedPracticeSessions] = useState(0);
  const practiceRequestSequence = useRef(1);
  const [analyticsLaunch, setAnalyticsLaunch] = useState<AnalyticsLaunchRequest | null>(null);
  const analyticsCheckpointRef = useRef<AnalyticsCheckpoint | null>(null);
  const analyticsRequestSequence = useRef(1);
  const navigateRef = useRef<(next: AppSection) => Promise<void>>(async () => undefined);
  // Library reopens the view the learner used last in this process.
  /** The subject page Home shows, or null for the subject list. Process-local, never checkpointed. */
  const [homeSubjectId, setHomeSubjectId] = useState<string | null>(null);
  const lastLibrarySectionRef = useRef<LibrarySection>("decks");
  if (isLibrarySection(section)) lastLibrarySectionRef.current = section;
  const returnPracticeToOriginRef = useRef<() => Promise<void>>(async () => undefined);
  const [hasUnsavedCard, setHasUnsavedCard] = useState(false);
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
      // A session that had a cursor and reports none has finished: its summary shows.
      if (checkpoint === null && practiceCursorRef.current !== null) {
        setFinishedPracticeSessions((current) => current + 1);
      }
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
          const restoredSection: AppSection = restoredCheckpoint.section;
          const restoredWorkspace =
            restoredSection === "decks"
              ? normalizeFlashcardsWorkspace(restoredCheckpoint.flashcardsWorkspace)
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
    // Home's subject list is the root: Android Back from a subject page or
    // any other destination returns there.
    if (section === "home" && homeSubjectId === null && !practiceWorkflow) return;
    return registerPlatformBackHandler(() => {
      void (practiceWorkflow
        ? returnPracticeToOriginRef.current()
        : navigateRef.current("home"));
      return true;
    });
  }, [homeSubjectId, practiceWorkflow, section]);

  useEffect(() => {
    if (!hasUnsavedCard && !hasUnsavedSettings) return;
    function warnBeforeClose(event: BeforeUnloadEvent): void {
      event.preventDefault();
      event.returnValue = "";
    }
    window.addEventListener("beforeunload", warnBeforeClose);
    return () => window.removeEventListener("beforeunload", warnBeforeClose);
  }, [hasUnsavedCard, hasUnsavedSettings]);

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

  async function confirmDiscardSettings(): Promise<boolean> {
    if (section !== "settings" || !hasUnsavedSettings) return true;
    const accepted = await confirm({
      title: "Discard settings changes?",
      description:
        "Unsaved preference previews, AI provider metadata, credential entry, and card template edits will be discarded. Saved credentials are unchanged.",
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
    return confirmDiscardSettings();
  }

  async function confirmPracticeTermination(): Promise<boolean> {
    if (!practiceWorkflow || !hasActivePracticeSession()) {
      return true;
    }
    const accepted = await confirm({
      title: "Terminate this practice session?",
      description:
        "Cards you already reviewed are saved and will keep their updated schedules and review history. Leaving now ends this session without showing its summary.",
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
          // One-click deck Practice always permutes equal-priority cards.
          // Selected-deck setup remains the explicit place to turn shuffling off.
          shuffle: true,
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
    if (request.kind === "subject") {
      openPracticeWorkflow(
        {
          kind: "selected",
          // A subject is studied live, as the decks filed in it now.
          subjectIds: [request.subjectId],
          deckIds: [],
          tagIds: [],
          // Like one-click deck Practice: an extra session over the whole scope.
          shuffle: true,
          mode: "extra",
        },
        returnTarget,
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
    // An active session runs inside Review, so choosing Review keeps it.
    if (next === "review" && practiceWorkflow !== null) return;
    const leavingPracticeWorkflow = practiceWorkflow !== null;
    if (!(await confirmPracticeTermination())) return;
    if (next === "home" && section === "home" && homeSubjectId !== null && !leavingPracticeWorkflow) {
      // Home again from a subject page returns to the subject list.
      setHomeSubjectId(null);
      return;
    }
    const alreadyAtNonFlashcardsRoot = next === section && next !== "decks";
    if (alreadyAtNonFlashcardsRoot && !leavingPracticeWorkflow) return;
    if (!(await canLeaveCurrentSection())) return;
    if (leavingPracticeWorkflow) {
      practiceCursorRef.current = null;
      setPracticeWorkflow(null);
    }
    setHomeSubjectId(null);
    if (
      next !== "decks" &&
      flashcardsRoute.view === "root" &&
      flashcardsRoute.analyticsSearchLaunch
    ) {
      setFlashcardsRoute(createFlashcardsRootRoute("decks", nextFlashcardsRequestId()));
    }
    if (next === "analytics") {
      const requestId = analyticsRequestSequence.current;
      analyticsRequestSequence.current += 1;
      const analytics = { range: "7d" as const, deckIds: [], tagIds: [] };
      analyticsCheckpointRef.current = analytics;
      setAnalyticsLaunch({ requestId, ...analytics });
    }
    if (next === "decks") {
      // Coming back to Library reopens the deck the Workbench had open; choosing
      // Library again from inside it returns to All cards.
      const requestId = nextFlashcardsRequestId();
      const returning = section !== "decks";
      setFlashcardsRoute((current) =>
        returning && current.view === "deck"
          ? createFlashcardsDeckRoute(current.deckId, requestId)
          : createFlashcardsRootRoute("decks", requestId),
      );
      void refreshDecks().catch((error: unknown) =>
        showToast("error", getErrorMessage(error)),
      );
    }
    setSection(next);
  }

  /** Search opens Flashcards Search/Filter inside Library from anywhere. */
  async function openSearch(): Promise<void> {
    const leavingPracticeWorkflow = practiceWorkflow !== null;
    if (!(await confirmPracticeTermination())) return;
    if (!(await canLeaveCurrentSection())) return;
    if (leavingPracticeWorkflow) {
      practiceCursorRef.current = null;
      setPracticeWorkflow(null);
    }
    setSection("decks");
    setFlashcardsRoute(createFlashcardsRootRoute("search", nextFlashcardsRequestId()));
    void refreshDecks().catch((error: unknown) =>
      showToast("error", getErrorMessage(error)),
    );
  }

  /**
   * Opens Library from anywhere. No scope opens the view the learner used
   * last; `subjectId` puts that subject's decks in view (`null` is No
   * subject's unsorted decks); `deckId` opens the deck. The Workbench reads
   * the scope from the Library route (see workbenchLaunchFor).
   */
  async function openLibrary(scope: LibraryScope = {}): Promise<void> {
    if (scope.deckId === undefined && scope.subjectId === undefined) {
      await navigate(lastLibrarySectionRef.current);
      return;
    }
    const leavingPracticeWorkflow = practiceWorkflow !== null;
    if (!(await confirmPracticeTermination())) return;
    if (!(await canLeaveCurrentSection())) return;
    if (leavingPracticeWorkflow) {
      practiceCursorRef.current = null;
      setPracticeWorkflow(null);
    }
    if (scope.deckId !== undefined) {
      openFlashcardsDeck(scope.deckId);
      return;
    }
    setSection("decks");
    setFlashcardsRoute(
      createSubjectFocusRoute(scope.subjectId ?? null, nextFlashcardsRequestId()),
    );
    void refreshDecks().catch((error: unknown) =>
      showToast("error", getErrorMessage(error)),
    );
  }

  /**
   * Opens a subject's page inside Home. No subject has no page of its own, so
   * it opens the Library Workbench on its unsorted cards, or Documents when it holds only
   * documents.
   */
  async function openSubject(
    subjectId: string | null,
    noSubjectView: "decks" | "documents" = "decks",
  ): Promise<void> {
    if (subjectId === null) {
      if (noSubjectView === "documents") await navigate("documents");
      else await openLibrary({ subjectId: null });
      return;
    }
    const leavingPracticeWorkflow = practiceWorkflow !== null;
    if (!(await confirmPracticeTermination())) return;
    if (!(await canLeaveCurrentSection())) return;
    if (leavingPracticeWorkflow) {
      practiceCursorRef.current = null;
      setPracticeWorkflow(null);
    }
    setHomeSubjectId(subjectId);
    setSection("home");
  }

  async function navigateFlashcardsWorkspace(
    next: CurrentFlashcardsWorkspaceName,
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

  /**
   * Records where the Library Workbench is, without a new launch request, so
   * practice returns to the open deck and a checkpoint reopens it.
   */
  function recordLibraryDeck(deckId: string | null): void {
    setFlashcardsRoute((current) => {
      if (deckId === null) {
        return current.view === "deck" ? createFlashcardsRootRoute("decks", current.requestId) : current;
      }
      return current.view === "deck" && current.deckId === deckId
        ? current
        : createFlashcardsDeckRoute(deckId, current.requestId);
    });
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

  function practiceSubject(subjectId: string): Promise<void> {
    return launchPractice({ kind: "subject", subjectId });
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
    const workspace = restoredTarget.flashcardsWorkspace ?? "decks";
    if (restoredTarget.section === "decks") {
      setFlashcardsRoute(
        restoredTarget.deckId
          ? createFlashcardsDeckRoute(restoredTarget.deckId, nextFlashcardsRequestId())
          : createFlashcardsRootRoute(
              workspace,
              nextFlashcardsRequestId(),
              reconcileReturnedDeckSelection(target.restoreDeckSelection, currentDecks),
            ),
      );
    } else if (restoredTarget.section === "analytics") {
      const analytics = analyticsCheckpointRef.current ?? {
        range: "7d",
        deckIds: [],
        tagIds: [],
      };
      const requestId = analyticsRequestSequence.current;
      analyticsRequestSequence.current += 1;
      setAnalyticsLaunch({ requestId, ...analytics });
    }
    setSection(restoredTarget.section);
  }

  async function returnPracticeToOrigin(): Promise<void> {
    const target = practiceWorkflow?.returnTarget;
    if (!practiceWorkflow || !(await confirmPracticeTermination())) return;
    if (!(await canLeaveCurrentSection())) return;
    await returnFromPractice(target ?? { section: "home" });
  }


  navigateRef.current = navigate;
  returnPracticeToOriginRef.current = returnPracticeToOrigin;
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
    /** Counts the practice sessions that reached their summary in this run. */
    finishedPracticeSessions,
    consumeAnalyticsLaunch,
    navigate,
    /** The Library view the Library destination opens: the last one used. */
    librarySection: lastLibrarySectionRef.current,
    openSearch,
    openLibrary,
    openSubject,
    /** The subject page shown inside Home, or null for the subject list. */
    homeSubjectId,
    navigateFlashcardsWorkspace,
    openFlashcardsDeck,
    recordLibraryDeck,
    openAnalyticsTagSearch,
    returnToFlashcardsLibrary,
    practiceDeck,
    practiceScope,
    practiceSelected,
    practiceCards,
    practiceSubject,
    launchPractice,
    returnFromPractice,
    choosePracticeDecks,
    viewSessionAnalytics,
    setHasUnsavedCard,
    setHasUnsavedSettings,
    setActiveSettings,
    saveSettingsLocally,
  };
}

interface CheckpointSnapshotState {
  section: AppSection;
  flashcardsRoute: FlashcardsRoute;
  practiceWorkflow: PracticeWorkflowState | null;
  practiceCursor: PracticeCursorCheckpoint | null;
  analytics: AnalyticsCheckpoint | null;
}

function createCheckpointSnapshot(state: CheckpointSnapshotState): SaveUiCheckpointInput {
  const visibleTarget: PracticeReturnTarget =
    state.section === "decks"
      ? state.flashcardsRoute.view === "deck"
        ? { section: "decks", flashcardsWorkspace: "decks", deckId: state.flashcardsRoute.deckId }
        : { section: "decks", flashcardsWorkspace: state.flashcardsRoute.workspace }
      : { section: state.section };
  const returnTarget = state.practiceWorkflow?.returnTarget ?? visibleTarget;
  const practice =
    state.practiceWorkflow && state.practiceCursor
      ? {
          ...state.practiceCursor,
          returnTarget: toCheckpointReturnTarget(returnTarget),
        }
      : undefined;
  return {
    section: state.section,
    flashcardsWorkspace:
      state.section === "decks" ? state.flashcardsRoute.workspace : undefined,
    deckId:
      state.section === "decks" && state.flashcardsRoute.view === "deck"
        ? state.flashcardsRoute.deckId
        : undefined,
    practice,
    analytics: state.section === "analytics" ? state.analytics ?? undefined : undefined,
  };
}

function toCheckpointReturnTarget(target: PracticeReturnTarget): CheckpointReturnTarget {
  if (target.section !== "decks") {
    return {
      section: target.section,
      flashcardsWorkspace: null,
      deckId: null,
    };
  }
  return {
    section: "decks",
    flashcardsWorkspace: target.flashcardsWorkspace ?? "decks",
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
    flashcardsWorkspace: normalizeFlashcardsWorkspace(target.flashcardsWorkspace),
  };
}

function normalizeSettings(settings: AppSettings): AppSettings {
  const motionPreference = settings.motionPreference ?? "system";
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
    displayQuestionOnAnswer: settings.displayQuestionOnAnswer ?? false,
    desiredRetention: settings.desiredRetention,
    dailyNewLimit: settings.dailyNewLimit,
    dailyReviewLimit: settings.dailyReviewLimit,
    newDayStartHour: settings.newDayStartHour,
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
