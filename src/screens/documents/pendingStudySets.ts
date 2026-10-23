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

export function queueStudySet(subjectId: string, set: PendingStudySet): void {
  pending.set(subjectId, set);
}

/** The subject's waiting study set, which the caller now owns. */
export function takeStudySet(subjectId: string): PendingStudySet | null {
  const set = pending.get(subjectId) ?? null;
  pending.delete(subjectId);
  return set;
}
