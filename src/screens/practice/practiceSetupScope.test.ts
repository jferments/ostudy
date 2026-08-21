import { beforeEach, describe, expect, it } from "vitest";

import type { DeckSummary } from "../../ipc/decks";
import {
  eligiblePracticeDeckIds,
  readPracticeSetupDeckIds,
  resolvePracticeSetupDeckIds,
  savePracticeSetupDeckIds,
} from "./practiceSetupScope";

const activeDeck: DeckSummary = {
  id: "a".repeat(32),
  name: "Active",
  description: "",
  sourceNoteCount: 1,
  generatedCardCount: 2,
  studyableDueCount: 1,
  archivedAt: null,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};
const secondDeck: DeckSummary = { ...activeDeck, id: "b".repeat(32), name: "Second" };
const archivedDeck: DeckSummary = {
  ...activeDeck,
  id: "c".repeat(32),
  name: "Archived",
  archivedAt: 10,
};
const emptyDeck: DeckSummary = {
  ...activeDeck,
  id: "d".repeat(32),
  name: "No cards",
  generatedCardCount: 0,
  studyableDueCount: 0,
};

describe("practice setup scope", () => {
  beforeEach(() => window.localStorage.clear());

  it("treats active decks with generated cards as eligible even when nothing is due", () => {
    expect(
      eligiblePracticeDeckIds([
        { ...activeDeck, studyableDueCount: 0 },
        archivedDeck,
        emptyDeck,
      ]),
    ).toEqual([activeDeck.id]);
  });

  it("restores the still-valid portion of the last deck scope", () => {
    expect(
      resolvePracticeSetupDeckIds([activeDeck, secondDeck], [secondDeck.id, archivedDeck.id]),
    ).toEqual([secondDeck.id]);
  });

  it("falls back to every eligible deck when the stored scope is no longer valid", () => {
    expect(resolvePracticeSetupDeckIds([activeDeck, secondDeck], [archivedDeck.id])).toEqual([
      activeDeck.id,
      secondDeck.id,
    ]);
  });

  it("round-trips a canonical device-local scope without requiring durable learner data", () => {
    savePracticeSetupDeckIds([secondDeck.id, activeDeck.id, secondDeck.id]);
    expect(readPracticeSetupDeckIds()).toEqual([activeDeck.id, secondDeck.id]);
  });
});
