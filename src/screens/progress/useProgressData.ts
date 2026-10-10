import { useEffect, useRef, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import type { AnalyticsDimensionOptions, QuestionTypeAnalytics, SessionAnalytics, TagAnalytics } from "../../ipc/analytics";
import { getErrorMessage } from "../../ipc/common";
import {
  getProgressActivity,
  getProgressCardStrength,
  getProgressFilterOptions,
  getProgressForecast,
  getProgressOverview,
  getProgressQuestionTypes,
  getProgressSessions,
  getProgressSubjects,
  getProgressTopics,
  getReviewLog,
  type CardStrength,
  type ProgressActivity,
  type ProgressForecast,
  type ProgressOverview,
  type ProgressQuery,
  type ReviewLogPage,
  type SubjectProgress,
} from "../../ipc/progress";
import { getSubjectLibrary, type SubjectLibrary } from "../../ipc/subjects";

export interface SectionState<T> {
  /** The last value read; kept while a newer read is loading. */
  data: T | null;
  loading: boolean;
  error: string | null;
}

/**
 * One independent latest-request lane: a newer key supersedes an older read,
 * whose value or error is then dropped. Sections never wait on each other.
 */
export function useLatestRead<T>(key: string, read: () => Promise<T>): SectionState<T> {
  const lane = useRequestLane();
  const readRef = useRef(read);
  readRef.current = read;
  const [state, setState] = useState<SectionState<T>>({ data: null, loading: true, error: null });
  useEffect(() => {
    let active = true;
    setState((current) => (current.loading && current.error === null ? current : { ...current, loading: true, error: null }));
    latestResult(lane, () => readRef.current()).then(
      (value) => {
        if (active && value !== undefined) setState({ data: value, loading: false, error: null });
      },
      (error: unknown) => {
        if (active) setState((current) => ({ ...current, loading: false, error: getErrorMessage(error) }));
      },
    );
    return () => {
      active = false;
    };
  }, [key, lane]);
  return state;
}

export interface ProgressData {
  overview: SectionState<ProgressOverview>;
  forecast: SectionState<ProgressForecast>;
  strength: SectionState<CardStrength>;
  topics: SectionState<TagAnalytics[]>;
  subjects: SectionState<SubjectProgress[]>;
  questionTypes: SectionState<QuestionTypeAnalytics[]>;
  sessions: SectionState<SessionAnalytics[]>;
  answers: SectionState<ReviewLogPage>;
  activity: SectionState<ProgressActivity>;
  options: SectionState<AnalyticsDimensionOptions>;
  library: SectionState<SubjectLibrary>;
}

/** Every Progress section on its own lane. The calendar, filter options and subject list ignore the scope. */
export function useProgressData(query: ProgressQuery): ProgressData {
  const key = JSON.stringify(query);
  return {
    overview: useLatestRead(key, () => getProgressOverview(query)),
    forecast: useLatestRead(key, () => getProgressForecast(query)),
    strength: useLatestRead(key, () => getProgressCardStrength(query)),
    topics: useLatestRead(key, () => getProgressTopics(query)),
    subjects: useLatestRead(key, () => getProgressSubjects(query)),
    questionTypes: useLatestRead(key, () => getProgressQuestionTypes(query)),
    sessions: useLatestRead(key, () => getProgressSessions(query)),
    answers: useLatestRead(key, () => getReviewLog(query)),
    activity: useLatestRead("activity", getProgressActivity),
    options: useLatestRead("options", getProgressFilterOptions),
    library: useLatestRead("library", getSubjectLibrary),
  };
}
