import type { DeckSummary } from "../../ipc/decks";

const STORAGE_KEY = "ostudy.practice.setup.v1";
const MAX_STORED_DECK_IDS = 2_000;

interface StoredPracticeSetupScope {
  schemaVersion: 1;
  deckIds: string[];
}

export function eligiblePracticeDeckIds(decks: DeckSummary[]): string[] {
  return decks
    .filter((deck) => deck.archivedAt === null && deck.generatedCardCount > 0)
    .map((deck) => deck.id);
}

export function resolvePracticeSetupDeckIds(
  decks: DeckSummary[],
  storedDeckIds: readonly string[] | null = readPracticeSetupDeckIds(),
): string[] {
  const eligible = eligiblePracticeDeckIds(decks);
  if (eligible.length === 0) return [];
  if (!storedDeckIds || storedDeckIds.length === 0) return eligible;

  const eligibleSet = new Set(eligible);
  const restored = [...new Set(storedDeckIds)].filter((deckId) => eligibleSet.has(deckId));
  return restored.length > 0 ? restored : eligible;
}

export function readPracticeSetupDeckIds(): string[] | null {
  try {
    const serialized = window.localStorage.getItem(STORAGE_KEY);
    if (!serialized) return null;
    const parsed = JSON.parse(serialized) as Partial<StoredPracticeSetupScope>;
    if (parsed.schemaVersion !== 1 || !Array.isArray(parsed.deckIds)) return null;
    if (parsed.deckIds.length > MAX_STORED_DECK_IDS) return null;
    const deckIds = parsed.deckIds.filter(
      (value): value is string => typeof value === "string" && value.length > 0,
    );
    return deckIds.length === parsed.deckIds.length ? deckIds : null;
  } catch {
    return null;
  }
}

export function savePracticeSetupDeckIds(deckIds: Iterable<string>): void {
  const canonical = [...new Set(deckIds)].sort();
  if (canonical.length > MAX_STORED_DECK_IDS) return;
  const scope: StoredPracticeSetupScope = {
    schemaVersion: 1,
    deckIds: canonical,
  };
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(scope));
  } catch {
    // Practice setup scope is optional device-local presentation state.
  }
}
