import { useCallback, useEffect, useMemo, useState } from "react";

import type { ShowToast } from "../../../components/Toast";
import { useRequestLane } from "../../../controllers/requestLane";
import { listTags, type TagSummary } from "../../../ipc/cards";
import { getErrorMessage } from "../../../ipc/common";
import type { DeckSummary } from "../../../ipc/decks";
import { listDocuments, type DocumentSummary } from "../../../ipc/documents";
import {
  deleteSubject,
  getSubjectLibrary,
  moveDeckToSubject,
  moveDocumentToSubject,
  moveSubject,
  renameSubject,
  type SubjectLibrary,
} from "../../../ipc/subjects";
import type { WorkbenchLibrary } from "./workbenchModel";

export interface WorkbenchLibraryState {
  library: WorkbenchLibrary;
  tags: TagSummary[];
  loading: boolean;
  error: string | null;
  /** Rereads subjects, documents, tags and the app's decks. */
  reload: () => Promise<void>;
  /** Takes a subject library a mutation answered with. */
  acceptOrganization: (organization: SubjectLibrary) => void;
  renameSubject: (subjectId: string, name: string) => Promise<boolean>;
  moveSubject: (subjectId: string, targetIndex: number) => Promise<void>;
  deleteSubject: (subjectId: string) => Promise<boolean>;
  moveDecks: (deckIds: readonly string[], subjectId: string | null) => Promise<boolean>;
  moveDocument: (documentId: string, subjectId: string | null) => Promise<boolean>;
}

/**
 * The Workbench's library: the app's deck list plus subjects, document filing
 * and tags. Each read has its own lane, so a slow one never overwrites a
 * newer answer. Subject writes read the current revision first, as Home's new
 * subject sheet does, so a change made on another screen does not refuse them.
 */
export function useWorkbenchLibrary(
  decks: DeckSummary[],
  refreshDecks: () => Promise<DeckSummary[]>,
  showToast: ShowToast,
): WorkbenchLibraryState {
  const organizationLane = useRequestLane();
  const documentsLane = useRequestLane();
  const tagsLane = useRequestLane();
  const [organization, setOrganization] = useState<SubjectLibrary | null>(null);
  const [documents, setDocuments] = useState<DocumentSummary[]>([]);
  const [tags, setTags] = useState<TagSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadOrganization = useCallback(async () => {
    const token = organizationLane.begin();
    try {
      const next = await getSubjectLibrary();
      if (organizationLane.isCurrent(token)) {
        setOrganization(next);
        setError(null);
      }
    } catch (caught: unknown) {
      if (organizationLane.isCurrent(token)) setError(getErrorMessage(caught));
    }
  }, [organizationLane]);

  const loadDocuments = useCallback(async () => {
    const token = documentsLane.begin();
    try {
      const next = await listDocuments();
      if (documentsLane.isCurrent(token)) setDocuments(next);
    } catch {
      // Documents only add entries to the tree; the cards stay usable without them.
      if (documentsLane.isCurrent(token)) setDocuments([]);
    }
  }, [documentsLane]);

  const loadTags = useCallback(async () => {
    const token = tagsLane.begin();
    try {
      const next = await listTags([]);
      if (tagsLane.isCurrent(token)) setTags(next);
    } catch {
      if (tagsLane.isCurrent(token)) setTags([]);
    }
  }, [tagsLane]);

  const reload = useCallback(async () => {
    await Promise.all([
      loadOrganization(),
      loadDocuments(),
      loadTags(),
      refreshDecks().catch((caught: unknown) => {
        showToast("error", getErrorMessage(caught));
        return decks;
      }),
    ]);
  }, [decks, loadDocuments, loadOrganization, loadTags, refreshDecks, showToast]);

  useEffect(() => {
    let active = true;
    void Promise.all([loadOrganization(), loadDocuments(), loadTags()]).finally(() => {
      if (active) setLoading(false);
    });
    return () => {
      active = false;
    };
  }, [loadDocuments, loadOrganization, loadTags]);

  const acceptOrganization = useCallback((next: SubjectLibrary) => {
    // A mutation's answer is newer than any read still in flight.
    organizationLane.invalidate();
    setOrganization(next);
  }, [organizationLane]);

  /** Runs one subject write against the current revision and keeps its answer. */
  const write = useCallback(async (
    run: (current: SubjectLibrary) => Promise<SubjectLibrary>,
  ): Promise<boolean> => {
    try {
      const current = await getSubjectLibrary();
      acceptOrganization(await run(current));
      return true;
    } catch (caught: unknown) {
      showToast("error", getErrorMessage(caught));
      void loadOrganization();
      return false;
    }
  }, [acceptOrganization, loadOrganization, showToast]);

  const subjectRevision = (current: SubjectLibrary, subjectId: string) =>
    current.subjects.find((subject) => subject.id === subjectId)?.revision ?? 0;

  const library = useMemo<WorkbenchLibrary>(() => ({
    subjects: organization?.subjects ?? [],
    decks,
    documents,
    organization,
  }), [decks, documents, organization]);

  return {
    library,
    tags,
    loading,
    error,
    reload,
    acceptOrganization,
    renameSubject: (subjectId, name) => write((current) => renameSubject({
      subjectId,
      name,
      revision: subjectRevision(current, subjectId),
      expectedLibraryRevision: current.libraryRevision,
    })),
    moveSubject: async (subjectId, targetIndex) => {
      await write((current) => moveSubject({ subjectId, targetIndex, expectedLibraryRevision: current.libraryRevision }));
    },
    deleteSubject: async (subjectId) => {
      const deleted = await write((current) => deleteSubject({
        subjectId,
        revision: subjectRevision(current, subjectId),
        expectedLibraryRevision: current.libraryRevision,
      }));
      // Deleting a subject also removes its guide and study target.
      if (deleted) void refreshDecks().catch(() => undefined);
      return deleted;
    },
    moveDecks: (deckIds, subjectId) => write(async (current) => {
      let next = current;
      // Each move answers with the new revision, which guards the next one.
      for (const deckId of deckIds) {
        next = await moveDeckToSubject({ deckId, subjectId, expectedLibraryRevision: next.libraryRevision });
      }
      return next;
    }),
    moveDocument: (documentId, subjectId) => write((current) => moveDocumentToSubject({
      documentId,
      subjectId,
      expectedDocumentFilingRevision: current.documentFilingRevision,
    })),
  };
}
