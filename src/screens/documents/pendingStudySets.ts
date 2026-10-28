import { useMemo, useSyncExternalStore } from "react";

import type { MaterialsChoice } from "./SubjectStudyActions";

/** A New study set whose materials start once its documents are read. */
export interface PendingStudySet extends MaterialsChoice {
  documentIds: string[];
}

/**
 * Study sets waiting for their documents, by subject. They live outside any
 * page: StudySetMaterialsStarter, mounted with the app, starts each one's
 * materials once its documents are read, whichever page is open.
 */
const pending = new Map<string, PendingStudySet>();
const listeners = new Set<() => void>();

function changed(): void {
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function waitingSubjectsKey(): string {
  return [...pending.keys()].sort().join("\n");
}

/** The study set waiting for the subject's documents, if one is. */
export function useWaitingStudySet(subjectId: string): PendingStudySet | null {
  return useSyncExternalStore(subscribe, () => pending.get(subjectId) ?? null);
}

/** Every waiting study set, by subject. */
export function waitingStudySets(): ReadonlyMap<string, PendingStudySet> {
  return pending;
}

/** The subjects whose New study set is still waiting for its documents to be read. */
export function useSubjectsAwaitingStudySets(): ReadonlySet<string> {
  const key = useSyncExternalStore(subscribe, waitingSubjectsKey);
  return useMemo(() => new Set(key ? key.split("\n") : []), [key]);
}

export function resetStudySetsForTests(): void {
  pending.clear();
  changed();
}

export function queueStudySet(subjectId: string, set: PendingStudySet): void {
  pending.set(subjectId, set);
  changed();
}

/** The subject's waiting study set, which the caller now owns. */
export function takeStudySet(subjectId: string): PendingStudySet | null {
  const set = pending.get(subjectId) ?? null;
  if (pending.delete(subjectId)) changed();
  return set;
}
