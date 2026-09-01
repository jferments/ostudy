import type { DeckSummary } from "../../../ipc/decks";
import type { TransientDeckSelectionEntry } from "../../../navigation/flashcardsRoutes";

export type DeckSelectionLifecycle = "active" | "archived";

export interface DeckSelectionEntry {
  id: string;
  revision: number;
  lifecycle: DeckSelectionLifecycle;
}

export interface DeckSelectionState {
  entries: ReadonlyMap<string, DeckSelectionEntry>;
}

export interface DeckSelectionAnalysis {
  selectedIds: string[];
  decks: DeckSummary[];
  missingIds: string[];
  staleIds: string[];
  activeIds: string[];
  archivedIds: string[];
  emptyIds: string[];
  generatedIds: string[];
  isMixedLifecycle: boolean;
}

export function emptyDeckSelection(): DeckSelectionState {
  return { entries: new Map() };
}

export function restoreDeckSelection(
  restored: readonly TransientDeckSelectionEntry[],
  decks: readonly DeckSummary[],
): DeckSelectionState {
  const decksById = new Map(decks.map((deck) => [deck.id, deck]));
  const entries = new Map<string, DeckSelectionEntry>();
  for (const entry of restored) {
    const deck = decksById.get(entry.id);
    if (
      !deck ||
      deck.revision !== entry.revision ||
      deck.archivedAt !== null
    ) {
      continue;
    }
    entries.set(deck.id, selectionEntry(deck));
  }
  return { entries };
}

export function deckSelectionIds(selection: DeckSelectionState): ReadonlySet<string> {
  return new Set(selection.entries.keys());
}

export function toggleDeckSelection(
  selection: DeckSelectionState,
  deck: DeckSummary,
): DeckSelectionState {
  const entries = new Map(selection.entries);
  if (entries.has(deck.id)) entries.delete(deck.id);
  else entries.set(deck.id, selectionEntry(deck));
  return { entries };
}

export function addDecksToSelection(
  selection: DeckSelectionState,
  decks: readonly DeckSummary[],
): DeckSelectionState {
  const entries = new Map(selection.entries);
  for (const deck of decks) entries.set(deck.id, selectionEntry(deck));
  return { entries };
}

export function clearDeckSelection(): DeckSelectionState {
  return emptyDeckSelection();
}

export function removeDecksFromSelection(
  selection: DeckSelectionState,
  deckIds: readonly string[],
): DeckSelectionState {
  if (deckIds.length === 0) return selection;
  const entries = new Map(selection.entries);
  for (const deckId of deckIds) entries.delete(deckId);
  return entries.size === selection.entries.size ? selection : { entries };
}

/**
 * Reconciliation is intentionally based on the current lifecycle scope, not
 * organization order or row visibility. Collapsing a folder therefore never
 * changes selection, while a missing deck, lifecycle transition, or scope
 * change removes the obsolete entry deterministically. Revision changes stay
 * selected so the mutation boundary can report a stale selection explicitly.
 */
export function reconcileDeckSelection(
  selection: DeckSelectionState,
  scopeDecks: readonly DeckSummary[],
): DeckSelectionState {
  const scopeById = new Map(scopeDecks.map((deck) => [deck.id, deck]));
  const entries = new Map<string, DeckSelectionEntry>();
  for (const [id, entry] of selection.entries) {
    const deck = scopeById.get(id);
    if (!deck || lifecycleOf(deck) !== entry.lifecycle) continue;
    entries.set(id, entry);
  }
  if (sameEntries(selection.entries, entries)) return selection;
  return { entries };
}

export function analyzeDeckSelection(
  selection: DeckSelectionState,
  decks: readonly DeckSummary[],
): DeckSelectionAnalysis {
  const decksById = new Map(decks.map((deck) => [deck.id, deck]));
  const selectedIds = [...selection.entries.keys()];
  const selectedDecks: DeckSummary[] = [];
  const missingIds: string[] = [];
  const staleIds: string[] = [];
  const activeIds: string[] = [];
  const archivedIds: string[] = [];
  const emptyIds: string[] = [];
  const generatedIds: string[] = [];

  for (const [id, entry] of selection.entries) {
    const deck = decksById.get(id);
    if (!deck || lifecycleOf(deck) !== entry.lifecycle) {
      missingIds.push(id);
      continue;
    }
    selectedDecks.push(deck);
    if (deck.revision !== entry.revision) staleIds.push(id);
    if (deck.archivedAt === null) activeIds.push(id);
    else archivedIds.push(id);
    if (deck.generatedCardCount === 0) emptyIds.push(id);
    else generatedIds.push(id);
  }

  return {
    selectedIds,
    decks: selectedDecks,
    missingIds,
    staleIds,
    activeIds,
    archivedIds,
    emptyIds,
    generatedIds,
    isMixedLifecycle: activeIds.length > 0 && archivedIds.length > 0,
  };
}

function selectionEntry(deck: DeckSummary): DeckSelectionEntry {
  return { id: deck.id, revision: deck.revision, lifecycle: lifecycleOf(deck) };
}

function lifecycleOf(deck: DeckSummary): DeckSelectionLifecycle {
  return deck.archivedAt === null ? "active" : "archived";
}

function sameEntries(
  left: ReadonlyMap<string, DeckSelectionEntry>,
  right: ReadonlyMap<string, DeckSelectionEntry>,
): boolean {
  if (left.size !== right.size) return false;
  for (const [id, entry] of left) {
    if (right.get(id) !== entry) return false;
  }
  return true;
}
