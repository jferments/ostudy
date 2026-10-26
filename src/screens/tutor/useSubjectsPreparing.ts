import { useEffect, useMemo, useState } from "react";

import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getStudyTargetBundle } from "../../ipc/studyMaterials";
import { useSubjectsAwaitingStudySets } from "../documents/pendingStudySets";
import type { DocumentPreparations } from "../home/homeModel";
import { useSubjectsWritingCourses } from "../lessons/useSubjectCourse";

/** How often subjects with documents are asked whether their materials are still being made. */
export const MATERIALS_STATUS_INTERVAL_MS = 4000;

const RUNNING_BATCH_STATES = new Set(["queued", "preparing", "composing", "committing"]);

/**
 * The subjects whose study materials are still being made: a document being read or
 * written up, a New study set waiting for its documents, flashcards or a study guide
 * being composed, or lessons being written. The Tutor offers a subject only once its
 * materials are done.
 */
export function useSubjectsPreparing(
  documentSubjects: ReadonlyArray<{ documentId: string; subjectId: string | null }>,
  preparations: DocumentPreparations,
  subjectsWithDocuments: readonly string[],
): ReadonlySet<string> {
  const awaiting = useSubjectsAwaitingStudySets();
  const writing = useSubjectsWritingCourses();
  const composing = useSubjectsComposing(subjectsWithDocuments);
  return useMemo(() => {
    const preparing = new Set<string>([...awaiting, ...writing, ...composing]);
    for (const placement of documentSubjects) {
      if (placement.subjectId !== null && preparations.has(placement.documentId)) preparing.add(placement.subjectId);
    }
    return preparing;
  }, [awaiting, writing, composing, documentSubjects, preparations]);
}

/** The subjects whose flashcards or study guide are being composed now, read from the native side. */
function useSubjectsComposing(subjectIds: readonly string[]): ReadonlySet<string> {
  const lane = useRequestLane();
  const [composing, setComposing] = useState<ReadonlySet<string>>(() => new Set());
  const key = [...subjectIds].sort().join("\n");

  useEffect(() => {
    const ids = key ? key.split("\n") : [];
    if (ids.length === 0) {
      setComposing(new Set());
      return;
    }
    let disposed = false;
    const read = () => {
      void latestResult(lane, () =>
        Promise.all(ids.map((id) =>
          getStudyTargetBundle({ target: { kind: "collection", id } }).then(
            (status) => (status && RUNNING_BATCH_STATES.has(status.latestBatchState ?? "") ? id : null),
            // An unreadable status hides nothing: the subject's own page reports it.
            () => null,
          ))),
      ).then((running) => {
        if (disposed || running === undefined) return;
        const next = running.filter((id): id is string => id !== null);
        setComposing((current) =>
          current.size === next.length && next.every((id) => current.has(id)) ? current : new Set(next));
      });
    };
    read();
    const timer = window.setInterval(read, MATERIALS_STATUS_INTERVAL_MS);
    return () => {
      disposed = true;
      window.clearInterval(timer);
    };
  }, [key, lane]);

  return composing;
}
