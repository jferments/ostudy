import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { ShowToast } from "../components/Toast";
import { createOperationId, getErrorMessage } from "../ipc/common";
import {
  deleteDocumentStudyGuide,
  listDocumentStudyGuidePage,
  type StudyGuidePageItem,
  type StudyGuideLibraryPage,
  type StudyGuideLibrarySort,
} from "../ipc/studyGuides";
import { useRequestLane } from "./requestLane";

export const STUDY_GUIDE_PAGE_SIZE = 40;

export interface StudyGuideOpenRequest {
  requestId: number;
  documentId: string;
  documentTitle: string;
}

export interface StudyGuideSelection {
  documentId: string;
  documentTitle: string;
}

export interface StudyGuideDeleteTarget extends StudyGuideSelection {
  guideId: string;
  guideRevision: number;
}

interface UseStudyGuidesControllerOptions {
  openRequest?: StudyGuideOpenRequest | null;
  onOpenRequestConsumed?: (requestId: number) => void;
  showToast: ShowToast;
}

const EMPTY_PAGE: StudyGuideLibraryPage = {
  items: [],
  totalCount: 0,
  offset: 0,
  limit: STUDY_GUIDE_PAGE_SIZE,
};

export function useStudyGuidesController({
  openRequest = null,
  onOpenRequestConsumed,
  showToast,
}: UseStudyGuidesControllerOptions) {
  const [page, setPage] = useState<StudyGuideLibraryPage>(EMPTY_PAGE);
  const [draftQuery, setDraftQuery] = useState("");
  const [query, setQuery] = useState("");
  const [sort, setSortState] = useState<StudyGuideLibrarySort>("recent");
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<StudyGuideSelection | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [deletingGuideId, setDeletingGuideId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const readLane = useRequestLane();
  const consumedRequest = useRef<number | null>(null);
  const deleteOperations = useRef(new Map<string, string>());

  const load = useCallback(
    async (refreshOnly = false): Promise<void> => {
      const token = readLane.begin();
      if (refreshOnly) setIsRefreshing(true);
      else setIsLoading(true);
      try {
        const loaded = await listDocumentStudyGuidePage({
          query,
          sort,
          offset,
          limit: STUDY_GUIDE_PAGE_SIZE,
        });
        if (!readLane.isCurrent(token)) return;
        setPage(loaded);
        setErrorMessage(null);
        if (loaded.items.length === 0 && loaded.totalCount > 0 && offset > 0) {
          setOffset(Math.max(0, offset - STUDY_GUIDE_PAGE_SIZE));
        }
      } catch (error: unknown) {
        if (!readLane.isCurrent(token)) return;
        setErrorMessage(getErrorMessage(error));
      } finally {
        if (readLane.isCurrent(token)) {
          setIsLoading(false);
          setIsRefreshing(false);
        }
      }
    },
    [offset, query, readLane, sort],
  );

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    if (!openRequest || consumedRequest.current === openRequest.requestId) return;
    consumedRequest.current = openRequest.requestId;
    setSelected({
      documentId: openRequest.documentId,
      documentTitle: openRequest.documentTitle,
    });
    setDraftQuery("");
    setQuery("");
    setOffset(0);
    onOpenRequestConsumed?.(openRequest.requestId);
  }, [onOpenRequestConsumed, openRequest]);

  const pageNumber = Math.floor(page.offset / page.limit) + 1;
  const pageCount = Math.max(1, Math.ceil(page.totalCount / page.limit));
  const canGoPrevious = page.offset > 0;
  const canGoNext = page.offset + page.items.length < page.totalCount;

  const visibleRange = useMemo(() => {
    if (page.totalCount === 0) return null;
    return {
      first: page.offset + 1,
      last: page.offset + page.items.length,
    };
  }, [page]);

  function submitSearch(): void {
    const normalized = draftQuery.trim();
    setOffset(0);
    setQuery(normalized);
  }

  function clearSearch(): void {
    setDraftQuery("");
    setQuery("");
    setOffset(0);
  }

  function setSort(next: StudyGuideLibrarySort): void {
    setSortState(next);
    setOffset(0);
  }

  function openGuide(item: StudyGuideSelection): void {
    setSelected(item);
  }

  function closeGuide(): void {
    setSelected(null);
    void load(true);
  }

  async function deleteGuide(target: StudyGuideDeleteTarget): Promise<boolean> {
    if (deletingGuideId) return false;
    const operationId =
      deleteOperations.current.get(target.guideId) ?? createOperationId();
    deleteOperations.current.set(target.guideId, operationId);
    setDeletingGuideId(target.guideId);
    try {
      const result = await deleteDocumentStudyGuide(
        target.documentId,
        target.guideId,
        target.guideRevision,
        operationId,
      );
      deleteOperations.current.delete(target.guideId);
      setSelected((current) =>
        current?.documentId === target.documentId ? null : current,
      );
      showToast(
        "success",
        result.removedRevisionCount === 1
          ? "Study guide deleted. The source, linked flashcards, and your stable notes were preserved."
          : `Study guide deleted with ${result.removedRevisionCount} generated revisions. The source, linked flashcards, and your stable notes were preserved.`,
      );
      await load(true);
      return true;
    } catch (error: unknown) {
      const message = getErrorMessage(error);
      setErrorMessage(message);
      showToast("error", message);
      await load(true);
      return false;
    } finally {
      setDeletingGuideId(null);
    }
  }

  return {
    page,
    draftQuery,
    query,
    sort,
    selected,
    isLoading,
    isRefreshing,
    deletingGuideId,
    errorMessage,
    pageNumber,
    pageCount,
    canGoPrevious,
    canGoNext,
    visibleRange,
    setDraftQuery,
    submitSearch,
    clearSearch,
    setSort,
    refresh: () => load(true),
    goPrevious: () => setOffset((current) => Math.max(0, current - STUDY_GUIDE_PAGE_SIZE)),
    goNext: () => setOffset((current) => current + STUDY_GUIDE_PAGE_SIZE),
    openGuide,
    closeGuide,
    deleteGuide,
  };
}

export function deleteTargetFromItem(
  item: StudyGuidePageItem,
): StudyGuideDeleteTarget {
  return {
    documentId: item.documentId,
    documentTitle: item.documentTitle,
    guideId: item.guideId,
    guideRevision: item.guideRevision,
  };
}
