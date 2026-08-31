import { describe, expect, it } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import type { PracticeWorkloadOverview } from "../../../ipc/practice";
import type { DeckSelectionAnalysis } from "./selectionModel";
import {
  MAX_PRACTICE_DECKS,
  MAX_PRACTICE_TAGS,
  canAddPracticeTag,
  createPracticeSetupRequest,
  practiceModeAvailability,
  practiceSelectionValidation,
  reconcilePracticeTagIds,
  snapshotPracticeDeckScope,
  validatePracticeDeckScope,
} from "./practiceSetupModel";

const currentDeck = deck("a", "Anatomy");
const workload: PracticeWorkloadOverview = {
  newLimit: 20,
  reviewLimit: 200,
  newCompleted: 1,
  reviewsCompleted: 2,
  newRemaining: 19,
  reviewsRemaining: 198,
  studyableDueReviews: 8,
  studyableDueNew: 4,
  scheduledReviews: 5,
  scheduledNew: 3,
  omittedDueReviews: 3,
  omittedDueNew: 1,
  needsMorePractice: 2,
  extraAvailable: 12,
};

describe("selected-deck practice setup model", () => {
  it("rejects the entire missing, stale, archived, empty, and over-limit scope", () => {
    for (const overrides of [
      { missingIds: ["missing"] },
      { staleIds: [currentDeck.id] },
      { archivedIds: [currentDeck.id], activeIds: [] },
      { emptyIds: [currentDeck.id], generatedIds: [] },
      { selectedIds: Array.from({ length: MAX_PRACTICE_DECKS + 1 }, (_, index) => String(index)) },
    ] satisfies Array<Partial<DeckSelectionAnalysis>>) {
      expect(practiceSelectionValidation(analysis(overrides)).valid).toBe(false);
    }
  });

  it("snapshots one or many canonical current decks and invalidates on any scope drift", () => {
    const second = deck("b", "Biology");
    const scope = snapshotPracticeDeckScope(
      analysis({
        selectedIds: [second.id, currentDeck.id],
        decks: [second, currentDeck],
        activeIds: [second.id, currentDeck.id],
        generatedIds: [second.id, currentDeck.id],
      }),
    );
    expect(scope?.deckIds).toEqual([currentDeck.id, second.id]);
    expect(validatePracticeDeckScope(scope!, [currentDeck, second])).toEqual({ valid: true });
    expect(
      validatePracticeDeckScope(scope!, [{ ...currentDeck, revision: 2 }, second]).valid,
    ).toBe(false);
    expect(validatePracticeDeckScope(scope!, [currentDeck]).valid).toBe(false);
    expect(
      validatePracticeDeckScope(
        { ...scope!, deckIds: [...scope!.deckIds, scope!.deckIds[0]!] },
        [currentDeck, second],
      ).valid,
    ).toBe(false);
  });

  it("reconciles tags against the constrained choices and enforces the native limit", () => {
    const tags = [
      { id: "a", name: "A", sourceNoteCount: 1, generatedCardCount: 1, studyableDueCount: 1 },
      { id: "b", name: "B", sourceNoteCount: 1, generatedCardCount: 1, studyableDueCount: 0 },
    ];
    expect(reconcilePracticeTagIds(["missing", "b", "a", "a"], tags)).toEqual(["a", "b"]);
    const atLimit = Array.from({ length: MAX_PRACTICE_TAGS }, (_, index) => String(index));
    expect(canAddPracticeTag(atLimit, "new")).toBe(false);
    expect(canAddPracticeTag(atLimit, atLimit[0]!)).toBe(true);
  });

  it("keeps scheduled and extra availability explicit and builds only the selected mode", () => {
    const scope = snapshotPracticeDeckScope(analysis())!;
    expect(practiceModeAvailability(workload)).toEqual({
      scheduled: expect.objectContaining({ available: true, count: 8 }),
      extra: expect.objectContaining({ available: true, count: 12 }),
    });
    expect(createPracticeSetupRequest(scope, ["b", "a", "b"], false, "extra", workload)).toEqual({
      deckIds: [currentDeck.id],
      tagIds: ["a", "b"],
      shuffle: false,
      mode: "extra",
    });
    expect(
      createPracticeSetupRequest(
        scope,
        [],
        true,
        "scheduled",
        { ...workload, scheduledReviews: 0, scheduledNew: 0 },
      ),
    ).toBeNull();
  });
});

function analysis(overrides: Partial<DeckSelectionAnalysis> = {}): DeckSelectionAnalysis {
  return {
    selectedIds: [currentDeck.id],
    decks: [currentDeck],
    missingIds: [],
    staleIds: [],
    activeIds: [currentDeck.id],
    archivedIds: [],
    emptyIds: [],
    generatedIds: [currentDeck.id],
    isMixedLifecycle: false,
    ...overrides,
  };
}

function deck(seed: string, name: string): DeckSummary {
  return {
    id: seed.repeat(32),
    name,
    description: "",
    sourceNoteCount: 2,
    generatedCardCount: 2,
    studyableDueCount: 1,
    archivedAt: null,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}
