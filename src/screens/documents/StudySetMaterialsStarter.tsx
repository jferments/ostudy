import { useEffect } from "react";

import { useDocumentJobs } from "../../controllers/DocumentJobsProvider";
import type { DocumentSummary } from "../../ipc/documents";
import { getStudyTargetBundle } from "../../ipc/studyMaterials";
import { startSubjectCourse } from "../lessons/useSubjectCourse";
import { takeStudySet, useSubjectsAwaitingStudySets, waitingStudySets, type PendingStudySet } from "./pendingStudySets";
import { runStudyTargetBundle } from "./useStudyTargetBundleController";

/** Starts a study set's flashcards and study guide bundle and its lessons. */
export function startStudySetMaterials(subjectId: string, set: PendingStudySet, changed?: () => Promise<unknown>): void {
  if (set.flashcards || set.studyGuide) {
    const target = { kind: "collection" as const, id: subjectId };
    // The latest status says which suppressed outputs a new run recreates.
    void getStudyTargetBundle({ target }).catch(() => null).then((known) =>
      runStudyTargetBundle(target, { flashcards: set.flashcards, studyGuide: set.studyGuide }, known, changed));
  }
  if (set.lessons) startSubjectCourse(subjectId, { podcast: set.podcast ?? false, lectureAudio: set.lectureAudio });
}

/**
 * Starts each waiting study set's materials once its documents are read. It is
 * mounted with the app, so the materials start whichever page is open, not only
 * the subject's Documents page.
 */
export function StudySetMaterialsStarter({
  documents,
  isDocumentWorking,
  changed,
}: {
  documents: readonly DocumentSummary[];
  /** True while a document is being read or is making its first cards. */
  isDocumentWorking: (documentId: string) => boolean;
  changed?: () => Promise<unknown>;
}) {
  const awaiting = useSubjectsAwaitingStudySets();
  useEffect(() => {
    for (const subjectId of awaiting) {
      const set = waitingStudySets().get(subjectId);
      // A document not listed yet has only just been added, so it is still to be read.
      const stillWorking = set?.documentIds.some(
        (documentId) => isDocumentWorking(documentId) || !documents.some((document) => document.id === documentId),
      ) ?? true;
      if (stillWorking) continue;
      const taken = takeStudySet(subjectId);
      if (taken) startStudySetMaterials(subjectId, taken, changed);
    }
  }, [awaiting, documents, isDocumentWorking, changed]);
  return null;
}

/** The starter fed by the app's document jobs. */
export function AppStudySetMaterialsStarter({ refreshDecks }: { refreshDecks: () => Promise<unknown> }) {
  const jobs = useDocumentJobs();
  if (!jobs) return null;
  return (
    <StudySetMaterialsStarter
      documents={jobs.documents}
      isDocumentWorking={(documentId) => jobs.busyDocumentIds.has(documentId)
        || jobs.documents.some((document) => document.id === documentId && document.lifecycleState === "processing")}
      changed={refreshDecks}
    />
  );
}
