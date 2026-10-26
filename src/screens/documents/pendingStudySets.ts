import { useMemo, useSyncExternalStore } from "react";

import type { MaterialsChoice } from "./SubjectStudyActions";

/** A New study set whose materials start once its documents are read. */
export interface PendingStudySet extends MaterialsChoice {
  documentIds: string[];
}

/**
 * Study sets waiting for their documents, by subject. They live outside any
 * page, so a subject page that closes before its documents are read hands
 * its wait back here and the next one to open picks it up.
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

/** The subjects whose New study set is still waiting for its documents to be read. */
export function useSubjectsAwaitingStudySets(): ReadonlySet<string> {
  const key = useSyncExternalStore(subscribe, waitingSubjectsKey);
  return useMemo(() => new Set(key ? key.split("\n") : []), [key]);
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
