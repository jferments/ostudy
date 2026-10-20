import { useCallback, useEffect, useMemo, useState } from "react";

import type { ShowToast } from "../../../components/Toast";
import { useOptionalHomeSummary } from "../../../controllers/HomeSummaryProvider";
import { useRequestLane } from "../../../controllers/requestLane";
import { listTags, type TagSummary } from "../../../ipc/cards";
import { getErrorMessage } from "../../../ipc/common";
import type { DeckSummary } from "../../../ipc/decks";
import { getSubjectLibrary, moveDeckToSubject, type SubjectLibrary } from "../../../ipc/subjects";
import { decksInSubject, type WorkbenchLibrary } from "./workbenchModel";

export interface WorkbenchLibraryState {
  library: WorkbenchLibrary;
  /** The chosen subject's tags, or every tag under Everything. */
  tags: TagSummary[];
  loading: boolean;
  error: string | null;
  /** Rereads subjects, tags and the app's decks. */
  reload: () => Promise<void>;
  /** Takes a subject library a mutation answered with. */
  acceptOrganization: (organization: SubjectLibrary) => void;
  moveDecks: (deckIds: readonly string[], subjectId: string | null) => Promise<boolean>;
}

/**
 * The Workbench's library: the app's deck list plus subjects and the chosen subject's tags. Each read has its own lane, so a slow one
 * never overwrites a newer answer. Subject writes read the current revision
 * first, as Home's new subject sheet does, so a change made on another screen
 * does not refuse them. The Workbench and the rest of Library share the
 * newest subject library either side has, so a subject renamed in the header
 * or a deck moved here shows everywhere at once.
 */
export function useWorkbenchLibrary(
  decks: DeckSummary[],
  refreshDecks: () => Promise<DeckSummary[]>,
  showToast: ShowToast,
  /** Library's chosen subject (`null` is No subject), or undefined for Everything. */
  subjectId?: string | null,
): WorkbenchLibraryState {
  const home = useOptionalHomeSummary();
  const acceptHomeLibrary = home?.acceptLibrary;
  const homeLibrary = home?.library ?? null;
  const organizationLane = useRequestLane();
  const tagsLane = useRequestLane();
  const [organization, setOrganization] = useState<SubjectLibrary | null>(null);
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

  // A subject's tags are the ones its decks use; a subject without decks has none.
  const tagDeckKey = useMemo(() => {
    if (subjectId === undefined) return "";
    if (organization === null) return null;
    return decksInSubject({ subjects: organization.subjects, decks, organization }, subjectId)
      .map((deck) => deck.id)
      .join(",");
  }, [decks, organization, subjectId]);
  const loadTags = useCallback(async () => {
    const token = tagsLane.begin();
    if (tagDeckKey === null || (subjectId !== undefined && tagDeckKey === "")) {
      setTags([]);
      return;
    }
    try {
      const next = await listTags(tagDeckKey === "" ? [] : tagDeckKey.split(","));
      if (tagsLane.isCurrent(token)) setTags(next);
    } catch {
      if (tagsLane.isCurrent(token)) setTags([]);
    }
  }, [subjectId, tagDeckKey, tagsLane]);

  const reload = useCallback(async () => {
    await Promise.all([
      loadOrganization(),
      loadTags(),
      refreshDecks().catch((caught: unknown) => {
        showToast("error", getErrorMessage(caught));
        return decks;
      }),
    ]);
  }, [decks, loadOrganization, loadTags, refreshDecks, showToast]);

  useEffect(() => {
    let active = true;
    void loadOrganization().finally(() => {
      if (active) setLoading(false);
    });
    return () => {
      active = false;
    };
  }, [loadOrganization]);

  useEffect(() => {
    void loadTags();
  }, [loadTags]);

  // A newer library from elsewhere in Library, such as a subject renamed or
  // deleted from its header.
  useEffect(() => {
    if (homeLibrary === null) return;
    setOrganization((current) => (current !== null && !isNewer(homeLibrary, current) ? current : homeLibrary));
  }, [homeLibrary]);

  const acceptOrganization = useCallback((next: SubjectLibrary) => {
    // A mutation's answer is newer than any read still in flight.
    organizationLane.invalidate();
    setOrganization(next);
    acceptHomeLibrary?.(next);
  }, [acceptHomeLibrary, organizationLane]);

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

  const library = useMemo<WorkbenchLibrary>(() => ({
    subjects: organization?.subjects ?? [],
    decks,
    organization,
  }), [decks, organization]);

  return {
    library,
    tags,
    loading,
    error,
    reload,
    acceptOrganization,
    moveDecks: (deckIds, subjectId) => write(async (current) => {
      let next = current;
      // Each move answers with the new revision, which guards the next one.
      for (const deckId of deckIds) {
        next = await moveDeckToSubject({ deckId, subjectId, expectedLibraryRevision: next.libraryRevision });
      }
      return next;
    }),
  };
}

function isNewer(candidate: SubjectLibrary, current: SubjectLibrary): boolean {
  return (
    candidate.libraryRevision > current.libraryRevision ||
    (candidate.libraryRevision === current.libraryRevision && candidate.documentFilingRevision > current.documentFilingRevision)
  );
}
