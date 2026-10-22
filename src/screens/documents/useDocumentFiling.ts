import { useCallback, useEffect, useState } from "react";

import { useOptionalHomeSummary } from "../../controllers/HomeSummaryProvider";
import { useRequestLane } from "../../controllers/requestLane";
import type { SourceRangeInput } from "../../ipc/collections";
import { getErrorMessage } from "../../ipc/common";
import {
  getSubjectLibrary,
  moveDocumentToSubject,
  setDocumentStudyRanges,
  setSubjectLeftOutDocuments,
  type DocumentStudyRange,
  type SubjectLibrary,
} from "../../ipc/subjects";

export interface DocumentFiling {
  /** Subjects, which document is in which, and each document's reading range; null while it loads. */
  library: SubjectLibrary | null;
  busy: boolean;
  error: string | null;
  /** The document's subject, or null for No subject. */
  subjectOf: (documentId: string) => string | null;
  rangesOf: (documentId: string) => DocumentStudyRange[];
  /** Whether the document is left out of its subject's study materials. */
  isLeftOut: (documentId: string) => boolean;
  moveDocument: (documentId: string, subjectId: string | null) => Promise<boolean>;
  setRanges: (documentId: string, ranges: SourceRangeInput[]) => Promise<boolean>;
  /** Leaves exactly these of the subject's documents out of its study materials. */
  setLeftOut: (subjectId: string, documentIds: string[]) => Promise<boolean>;
}

/**
 * Where each document is filed and which part of it the subject studies. It
 * reads the subject library Home and Library share when there is one, and
 * shares each change back, so the subject list's counts follow a moved
 * document. Writes read the current filing revision first, so a change made
 * elsewhere does not refuse them.
 */
export function useDocumentFiling(): DocumentFiling {
  const home = useOptionalHomeSummary();
  const shared = home?.library ?? null;
  const acceptShared = home?.acceptLibrary;
  const lane = useRequestLane();
  const [own, setOwn] = useState<SubjectLibrary | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const standalone = home === null;

  // Outside the app shell (tests, a hosted view) the page reads its own copy.
  useEffect(() => {
    if (!standalone) return;
    const token = lane.begin();
    void getSubjectLibrary().then(
      (value) => {
        if (lane.isCurrent(token)) setOwn(value);
      },
      (caught: unknown) => {
        if (lane.isCurrent(token)) setError(getErrorMessage(caught));
      },
    );
  }, [lane, standalone]);

  const library = standalone ? own : shared;

  const write = useCallback(
    async (run: (current: SubjectLibrary) => Promise<SubjectLibrary>): Promise<boolean> => {
      setBusy(true);
      setError(null);
      try {
        const next = await run(await getSubjectLibrary());
        lane.invalidate();
        setOwn(next);
        acceptShared?.(next);
        return true;
      } catch (caught: unknown) {
        setError(getErrorMessage(caught));
        return false;
      } finally {
        setBusy(false);
      }
    },
    [acceptShared, lane],
  );

  return {
    library,
    busy,
    error,
    subjectOf: (documentId) =>
      library?.documents.find((placement) => placement.documentId === documentId)?.subjectId ?? null,
    rangesOf: (documentId) => library?.studyRanges.filter((range) => range.documentId === documentId) ?? [],
    isLeftOut: (documentId) => library?.leftOutDocumentIds.includes(documentId) ?? false,
    moveDocument: (documentId, subjectId) =>
      write((current) =>
        moveDocumentToSubject({ documentId, subjectId, expectedDocumentFilingRevision: current.documentFilingRevision }),
      ),
    setRanges: (documentId, ranges) =>
      write((current) =>
        setDocumentStudyRanges({ documentId, ranges, expectedDocumentFilingRevision: current.documentFilingRevision }),
      ),
    setLeftOut: (subjectId, documentIds) =>
      write((current) =>
        setSubjectLeftOutDocuments({ subjectId, documentIds, expectedDocumentFilingRevision: current.documentFilingRevision }),
      ),
  };
}
