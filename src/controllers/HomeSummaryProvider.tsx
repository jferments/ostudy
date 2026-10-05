import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import { getErrorMessage } from "../ipc/common";
import type { DeckSummary } from "../ipc/decks";
import { getHomeSummary, type HomeSummary } from "../ipc/home";
import { getSubjectLibrary, type SubjectLibrary } from "../ipc/subjects";
import { useDocumentJobs } from "./DocumentJobsProvider";
import type { AppSection, AppWorkflow } from "./useAppController";
import { useRequestLane } from "./requestLane";

export interface HomeSummaryState {
  /** What is due today; null until the first read answers. */
  summary: HomeSummary | null;
  /** Subjects and filing; null until the first read answers. */
  library: SubjectLibrary | null;
  summaryError: string | null;
  libraryError: string | null;
  /** True until both reads have answered once. */
  isLoading: boolean;
  refresh: () => void;
  /** Adopts a subject mutation's answer, then rereads what is due. */
  acceptLibrary: (library: SubjectLibrary) => void;
}

const HomeSummaryContext = createContext<HomeSummaryState | null>(null);

interface HomeSummaryProviderProps {
  section: AppSection;
  activeWorkflow: AppWorkflow | null;
  /** The deck list: a new list means decks or their cards changed. */
  decks: readonly DeckSummary[];
  children: ReactNode;
}

/** Coalesces the triggers one change sets off, such as a session ending. */
const REFRESH_DELAY_MS = 50;

/**
 * Today's summary and the subject library for Home, Review and the Review
 * badge on every screen. Each is an independent latest-request read. They are
 * read again when Home or Review opens, when a practice session ends, when the
 * deck list changes, when a document arrives, leaves or finishes processing,
 * and when the app becomes visible again (the study day may have turned over).
 */
export function HomeSummaryProvider({
  section,
  activeWorkflow,
  decks,
  children,
}: HomeSummaryProviderProps) {
  const summaryLane = useRequestLane();
  const libraryLane = useRequestLane();
  const [summary, setSummary] = useState<HomeSummary | null>(null);
  const [library, setLibrary] = useState<SubjectLibrary | null>(null);
  const [summaryError, setSummaryError] = useState<string | null>(null);
  const [libraryError, setLibraryError] = useState<string | null>(null);
  const [summaryAnswered, setSummaryAnswered] = useState(false);
  const [libraryAnswered, setLibraryAnswered] = useState(false);
  const pendingRefresh = useRef<number | null>(null);
  const documents = useDocumentJobs()?.documents;
  // Which documents exist and whether each is still processing; progress
  // within a step changes nothing Home counts.
  const documentsKey = documents
    ?.map((document) => `${document.id}:${document.lifecycleState}`)
    .join(",");

  const readSummary = useCallback(() => {
    const token = summaryLane.begin();
    void getHomeSummary().then(
      (value) => {
        if (!summaryLane.isCurrent(token)) return;
        setSummary(value);
        setSummaryError(null);
        setSummaryAnswered(true);
      },
      (error: unknown) => {
        if (!summaryLane.isCurrent(token)) return;
        setSummaryError(getErrorMessage(error));
        setSummaryAnswered(true);
      },
    );
  }, [summaryLane]);

  const readLibrary = useCallback(() => {
    const token = libraryLane.begin();
    void getSubjectLibrary().then(
      (value) => {
        if (!libraryLane.isCurrent(token)) return;
        setLibrary(value);
        setLibraryError(null);
        setLibraryAnswered(true);
      },
      (error: unknown) => {
        if (!libraryLane.isCurrent(token)) return;
        setLibraryError(getErrorMessage(error));
        setLibraryAnswered(true);
      },
    );
  }, [libraryLane]);

  const refresh = useCallback(() => {
    readSummary();
    readLibrary();
  }, [readLibrary, readSummary]);

  const scheduleRefresh = useCallback(() => {
    if (pendingRefresh.current !== null) return;
    pendingRefresh.current = window.setTimeout(() => {
      pendingRefresh.current = null;
      refresh();
    }, REFRESH_DELAY_MS);
  }, [refresh]);

  useEffect(
    () => () => {
      if (pendingRefresh.current !== null) window.clearTimeout(pendingRefresh.current);
    },
    [],
  );

  const showsSummary = activeWorkflow === null && (section === "home" || section === "review");
  useEffect(() => {
    if (showsSummary) scheduleRefresh();
  }, [scheduleRefresh, section, showsSummary]);

  const previousWorkflow = useRef(activeWorkflow);
  useEffect(() => {
    const endedPractice = previousWorkflow.current !== null && activeWorkflow === null;
    previousWorkflow.current = activeWorkflow;
    if (endedPractice) scheduleRefresh();
  }, [activeWorkflow, scheduleRefresh]);

  useEffect(() => {
    scheduleRefresh();
  }, [decks, documentsKey, scheduleRefresh]);

  useEffect(() => {
    function refreshWhenVisible(): void {
      if (document.visibilityState === "visible") scheduleRefresh();
    }
    document.addEventListener("visibilitychange", refreshWhenVisible);
    return () => document.removeEventListener("visibilitychange", refreshWhenVisible);
  }, [scheduleRefresh]);

  const acceptLibrary = useCallback(
    (next: SubjectLibrary) => {
      // The mutation's answer is newer than any read still in flight.
      libraryLane.invalidate();
      setLibrary(next);
      setLibraryError(null);
      setLibraryAnswered(true);
      readSummary();
    },
    [libraryLane, readSummary],
  );

  const value = useMemo<HomeSummaryState>(
    () => ({
      summary,
      library,
      summaryError,
      libraryError,
      isLoading: !summaryAnswered || !libraryAnswered,
      refresh: scheduleRefresh,
      acceptLibrary,
    }),
    [
      acceptLibrary,
      library,
      libraryAnswered,
      libraryError,
      scheduleRefresh,
      summary,
      summaryAnswered,
      summaryError,
    ],
  );
  return <HomeSummaryContext.Provider value={value}>{children}</HomeSummaryContext.Provider>;
}

export function useHomeSummary(): HomeSummaryState {
  const value = useContext(HomeSummaryContext);
  if (!value) throw new Error("useHomeSummary must be used inside HomeSummaryProvider.");
  return value;
}

/** The cards Start review studies today, or null while unknown. */
export function cardsDueToday(summary: HomeSummary | null): number | null {
  return summary ? summary.today.scheduledReviews + summary.today.scheduledNew : null;
}
