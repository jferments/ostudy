import type { DeckSummary } from "../../../ipc/decks";
import type {
  PracticeWorkloadOverview,
  SelectedPracticeRequest,
} from "../../../ipc/practice";
import type { TagSummary } from "../../../ipc/tags";
import type { DeckSelectionAnalysis } from "./selectionModel";

export const MAX_PRACTICE_DECKS = 100;
export const MAX_PRACTICE_TAGS = 32;

export interface PracticeDeckSnapshot {
  id: string;
  name: string;
  revision: number;
  generatedCardCount: number;
}

export interface PracticeDeckScope {
  deckIds: string[];
  decks: PracticeDeckSnapshot[];
}

export type SetupResource<T> =
  | { key: string; status: "loading" }
  | { key: string; status: "ready"; value: T }
  | { key: string; status: "error"; message: string };

export interface PracticeWorkloadQuery {
  key: string;
  deckIds: string[];
  tagIds: string[];
}

export interface PracticeModeAvailability {
  scheduled: { available: boolean; count: number; explanation: string };
  extra: { available: boolean; count: number; explanation: string };
}

export type PracticeScopeValidation =
  | { valid: true }
  | { valid: false; message: string };

export function practiceSelectionValidation(
  analysis: DeckSelectionAnalysis,
): PracticeScopeValidation {
  const count = analysis.selectedIds.length;
  if (count === 0) {
    return { valid: false, message: "Select at least one deck to open Practice setup." };
  }
  if (count > MAX_PRACTICE_DECKS) {
    return {
      valid: false,
      message: `Practice supports at most ${MAX_PRACTICE_DECKS} decks. Clear part of this selection and try again.`,
    };
  }
  if (analysis.missingIds.length > 0) {
    return {
      valid: false,
      message: "A selected deck is no longer available. Clear and reselect the intended decks.",
    };
  }
  if (analysis.staleIds.length > 0) {
    return {
      valid: false,
      message: "A selected deck changed after selection. Clear and reselect the intended decks.",
    };
  }
  if (analysis.archivedIds.length > 0 || analysis.isMixedLifecycle) {
    return {
      valid: false,
      message: "Practice setup requires every selected deck to be active. Restore or clear archived decks.",
    };
  }
  if (analysis.emptyIds.length > 0 || analysis.generatedIds.length !== count) {
    return {
      valid: false,
      message: "Practice setup requires every selected deck to contain generated cards.",
    };
  }
  return { valid: true };
}

export function snapshotPracticeDeckScope(
  analysis: DeckSelectionAnalysis,
): PracticeDeckScope | null {
  if (!practiceSelectionValidation(analysis).valid) return null;
  const decks = analysis.decks
    .map((deck) => ({
      id: deck.id,
      name: deck.name,
      revision: deck.revision,
      generatedCardCount: deck.generatedCardCount,
    }))
    .sort((left, right) => left.id.localeCompare(right.id));
  return { deckIds: decks.map((deck) => deck.id), decks };
}

export function validatePracticeDeckScope(
  scope: PracticeDeckScope,
  currentDecks: readonly DeckSummary[],
): PracticeScopeValidation {
  if (!practiceDeckScopeShapeIsValid(scope)) {
    return {
      valid: false,
      message:
        "This selected deck scope is no longer valid. Close setup, then clear and reselect the decks you want to practice.",
    };
  }
  const currentById = new Map(currentDecks.map((deck) => [deck.id, deck]));
  for (const snapshot of scope.decks) {
    const current = currentById.get(snapshot.id);
    if (
      !current ||
      current.archivedAt !== null ||
      current.generatedCardCount === 0 ||
      current.revision !== snapshot.revision
    ) {
      return {
        valid: false,
        message:
          "This selected deck scope changed while setup was open. Close setup, then clear and reselect the decks you want to practice.",
      };
    }
  }
  return { valid: true };
}

export function canonicalTagIds(tagIds: Iterable<string>): string[] {
  return [...new Set(tagIds)].sort();
}

export function canonicalPracticeWorkloadQuery(
  deckIds: Iterable<string>,
  tagIds: Iterable<string>,
): PracticeWorkloadQuery {
  const canonicalDeckIds = [...new Set(deckIds)].sort();
  const canonicalTags = canonicalTagIds(tagIds);
  return {
    key: JSON.stringify({ deckIds: canonicalDeckIds, tagIds: canonicalTags }),
    deckIds: canonicalDeckIds,
    tagIds: canonicalTags,
  };
}

export function reconcilePracticeTagIds(
  tagIds: Iterable<string>,
  availableTags: readonly TagSummary[],
): string[] {
  const available = new Set(availableTags.map((tag) => tag.id));
  return canonicalTagIds(tagIds).filter((tagId) => available.has(tagId));
}

export function canAddPracticeTag(tagIds: Iterable<string>, tagId: string): boolean {
  const selected = new Set(tagIds);
  return selected.has(tagId) || selected.size < MAX_PRACTICE_TAGS;
}

export function practiceModeAvailability(
  workload: PracticeWorkloadOverview,
): PracticeModeAvailability {
  const scheduled = workload.scheduledReviews + workload.scheduledNew;
  const extra = workload.extraAvailable;
  return {
    scheduled: {
      available: scheduled > 0,
      count: scheduled,
      explanation:
        scheduled > 0
          ? `${scheduled} cards fit the current schedule and daily limits.`
          : "No cards in these decks and tags are available for scheduled study right now.",
    },
    extra: {
      available: extra > 0,
      count: extra,
      explanation:
        extra > 0
          ? `${extra} studyable cards are available for an explicit extra session.`
          : "No studyable cards in these decks and tags are available for extra study.",
    },
  };
}

export function createPracticeSetupRequest(
  scope: PracticeDeckScope,
  tagIds: Iterable<string>,
  shuffle: boolean,
  mode: "scheduled" | "extra",
  workload: PracticeWorkloadOverview,
): SelectedPracticeRequest | null {
  if (!practiceDeckScopeShapeIsValid(scope)) return null;
  const canonicalTags = canonicalTagIds(tagIds);
  if (canonicalTags.length > MAX_PRACTICE_TAGS) return null;
  const availability = practiceModeAvailability(workload)[mode];
  if (!availability.available) return null;
  return {
    deckIds: [...scope.deckIds],
    tagIds: canonicalTags,
    shuffle,
    mode,
  };
}

function practiceDeckScopeShapeIsValid(scope: PracticeDeckScope): boolean {
  if (
    scope.deckIds.length === 0 ||
    scope.deckIds.length > MAX_PRACTICE_DECKS ||
    scope.decks.length !== scope.deckIds.length ||
    new Set(scope.deckIds).size !== scope.deckIds.length
  ) {
    return false;
  }
  return scope.decks.every(
    (deck, index) =>
      deck.id === scope.deckIds[index] &&
      deck.generatedCardCount > 0,
  );
}
