import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { CardsController } from "../../../controllers/cardsController";
import { useRequestLane } from "../../../controllers/requestLane";
import { searchCards, type CardBrowserRow, type CardSearchInput, type SelectionCard } from "../../../ipc/cards";
import { getErrorMessage } from "../../../ipc/common";
import { selectionFromRow, type CardFilter } from "./workbenchModel";

export const PAGE_SIZE = 100;

export interface WorkbenchCardsState {
  rows: CardBrowserRow[];
  totalCount: number;
  todayStartAt: number;
  loading: boolean;
  loadingMore: boolean;
  error: string | null;
  /** Card counts for the quick filters in the current scope, once known. */
  filterCounts: Partial<Record<CardFilter, number>>;
  selected: Map<string, SelectionCard>;
  selectingAll: boolean;
  loadMore: () => void;
  reload: () => Promise<void>;
  toggle: (row: CardBrowserRow) => void;
  /** Extends the selection from the last toggled row to this one, as Shift+click does. */
  selectRange: (row: CardBrowserRow) => void;
  selectAllMatching: () => Promise<void>;
  clearSelection: () => void;
  controller: CardsController;
}

/**
 * The card list for one search: pages of 100 appended as the list scrolls,
 * a selection that survives paging, and the quick-filter counts. A new search
 * clears the selection, as Search/Filter always has.
 */
export function useWorkbenchCards(
  input: CardSearchInput | null,
  countInputs: Partial<Record<CardFilter, CardSearchInput>> | null,
): WorkbenchCardsState {
  const controllerRef = useRef<CardsController | null>(null);
  if (controllerRef.current === null) controllerRef.current = new CardsController();
  const controller = controllerRef.current;
  const countsLane = useRequestLane();
  const [rows, setRows] = useState<CardBrowserRow[]>([]);
  const [totalCount, setTotalCount] = useState(0);
  const [todayStartAt, setTodayStartAt] = useState(0);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filterCounts, setFilterCounts] = useState<Partial<Record<CardFilter, number>>>({});
  const [selected, setSelected] = useState<Map<string, SelectionCard>>(new Map());
  const [selectingAll, setSelectingAll] = useState(false);
  const anchorRef = useRef<string | null>(null);
  const inputKey = input ? JSON.stringify(input) : "none";
  const countKey = countInputs ? JSON.stringify(countInputs) : "none";
  // The memoized inputs change only when their content does.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const stableInput = useMemo(() => input, [inputKey]);
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const stableCounts = useMemo(() => countInputs, [countKey]);

  const load = useCallback(async (offset: number) => {
    if (!stableInput) {
      setRows([]);
      setTotalCount(0);
      setLoading(false);
      return;
    }
    if (offset === 0) setLoading(true);
    else setLoadingMore(true);
    setError(null);
    try {
      const page = await controller.search({ ...stableInput, offset });
      if (!page) return;
      setRows((current) => (offset === 0 ? page.cards : [...current, ...page.cards]));
      setTotalCount(page.totalCount);
      setTodayStartAt(page.todayStartAt);
    } catch (caught: unknown) {
      setError(getErrorMessage(caught));
    } finally {
      setLoading(false);
      setLoadingMore(false);
    }
  }, [controller, stableInput]);

  const loadCounts = useCallback(async () => {
    const token = countsLane.begin();
    if (!stableCounts) {
      setFilterCounts({});
      return;
    }
    try {
      const entries = await Promise.all(Object.entries(stableCounts).map(async ([filter, countInput]) => {
        // One row is enough: the page carries the total.
        const page = await searchCards({ ...countInput, limit: 1, offset: 0 });
        return [filter, page.totalCount] as const;
      }));
      if (countsLane.isCurrent(token)) setFilterCounts(Object.fromEntries(entries));
    } catch {
      // Counts decorate the filters; the list itself reports its own errors.
      if (countsLane.isCurrent(token)) setFilterCounts({});
    }
  }, [countsLane, stableCounts]);

  useEffect(() => () => controller.invalidate(), [controller]);
  useEffect(() => {
    setSelected(new Map());
    anchorRef.current = null;
    // A short pause lets typing in the search box settle into one request.
    const timer = window.setTimeout(() => void load(0), 120);
    return () => window.clearTimeout(timer);
  }, [load]);
  useEffect(() => {
    void loadCounts();
  }, [loadCounts]);

  const loadMore = useCallback(() => {
    if (loading || loadingMore || rows.length >= totalCount || error) return;
    void load(rows.length);
  }, [error, load, loading, loadingMore, rows.length, totalCount]);

  const reload = useCallback(async () => {
    setSelected(new Map());
    await Promise.all([load(0), loadCounts()]);
  }, [load, loadCounts]);

  const toggle = useCallback((row: CardBrowserRow) => {
    anchorRef.current = row.cardId;
    setSelected((current) => {
      const next = new Map(current);
      if (next.has(row.cardId)) next.delete(row.cardId);
      else next.set(row.cardId, selectionFromRow(row));
      return next;
    });
  }, []);

  const selectRange = useCallback((row: CardBrowserRow) => {
    const anchorIndex = rows.findIndex((candidate) => candidate.cardId === anchorRef.current);
    const index = rows.findIndex((candidate) => candidate.cardId === row.cardId);
    if (anchorIndex < 0 || index < 0) {
      toggle(row);
      return;
    }
    const [start, end] = anchorIndex < index ? [anchorIndex, index] : [index, anchorIndex];
    setSelected((current) => {
      const next = new Map(current);
      for (const candidate of rows.slice(start, end + 1)) next.set(candidate.cardId, selectionFromRow(candidate));
      return next;
    });
  }, [rows, toggle]);

  const selectAllMatching = useCallback(async () => {
    if (!stableInput || selectingAll || totalCount === 0) return;
    setSelectingAll(true);
    setError(null);
    try {
      const matches = await controller.matching(stableInput);
      if (matches) setSelected(new Map(matches.map((card) => [card.cardId, card])));
    } catch (caught: unknown) {
      setError(getErrorMessage(caught));
    } finally {
      setSelectingAll(false);
    }
  }, [controller, selectingAll, stableInput, totalCount]);

  return {
    rows,
    totalCount,
    todayStartAt,
    loading,
    loadingMore,
    error,
    filterCounts,
    selected,
    selectingAll,
    loadMore,
    reload,
    toggle,
    selectRange,
    selectAllMatching,
    clearSelection: () => setSelected(new Map()),
    controller,
  };
}
