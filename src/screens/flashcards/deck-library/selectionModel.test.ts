import { describe, expect, it } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import {
  addDecksToSelection,
  analyzeDeckSelection,
  clearDeckSelection,
  deckSelectionIds,
  emptyDeckSelection,
  reconcileDeckSelection,
  removeDecksFromSelection,
  restoreDeckSelection,
  toggleDeckSelection,
} from "./selectionModel";

const activeA = deck("a", "Active A");
const activeB = deck("b", "Active B", { generatedCardCount: 0 });
const archived = deck("c", "Archived", { archivedAt: 50 });

describe("deck selection model", () => {
  it("keeps selected IDs independent of organization order, collapse, activation, and drag state", () => {
    let selection = emptyDeckSelection();
    selection = toggleDeckSelection(selection, activeB);
    selection = toggleDeckSelection(selection, activeA);

    expect([...deckSelectionIds(selection)]).toEqual([activeB.id, activeA.id]);
    expect(
      reconcileDeckSelection(selection, [activeA, activeB]),
    ).toBe(selection);
    expect([...deckSelectionIds(selection)]).toEqual([activeB.id, activeA.id]);
  });

  it("adds visible decks without discarding an existing in-scope selection and clears explicitly", () => {
    const archivedSelection = addDecksToSelection(emptyDeckSelection(), [archived]);
    const selection = addDecksToSelection(archivedSelection, [activeA, activeB]);
    expect([...deckSelectionIds(selection)]).toEqual([
      archived.id,
      activeA.id,
      activeB.id,
    ]);
    expect(deckSelectionIds(clearDeckSelection()).size).toBe(0);
    expect([...deckSelectionIds(removeDecksFromSelection(selection, [activeA.id]))]).toEqual([
      archived.id,
      activeB.id,
    ]);
  });

  it("drops missing, lifecycle-changed, and out-of-scope decks but retains stale revisions", () => {
    const selection = addDecksToSelection(emptyDeckSelection(), [activeA, activeB, archived]);
    const revisedA = { ...activeA, revision: activeA.revision + 1 };
    const archivedB = { ...activeB, archivedAt: 100, revision: activeB.revision + 1 };

    const reconciled = reconcileDeckSelection(selection, [revisedA, archivedB]);
    expect([...deckSelectionIds(reconciled)]).toEqual([activeA.id]);
    expect(analyzeDeckSelection(reconciled, [revisedA]).staleIds).toEqual([activeA.id]);
  });

  it("reports mixed lifecycle, empty, generated, missing, and stale members explicitly", () => {
    const missing = deck("d", "Missing");
    const selection = addDecksToSelection(emptyDeckSelection(), [
      activeA,
      activeB,
      archived,
      missing,
    ]);
    const analysis = analyzeDeckSelection(selection, [
      { ...activeA, revision: 2 },
      activeB,
      archived,
    ]);

    expect(analysis.isMixedLifecycle).toBe(true);
    expect(analysis.activeIds).toEqual([activeA.id, activeB.id]);
    expect(analysis.archivedIds).toEqual([archived.id]);
    expect(analysis.emptyIds).toEqual([activeB.id]);
    expect(analysis.generatedIds).toEqual([activeA.id, archived.id]);
    expect(analysis.missingIds).toEqual([missing.id]);
    expect(analysis.staleIds).toEqual([activeA.id]);
  });

  it("restores current active in-process return entries regardless of card count", () => {
    const revised = { ...activeA, revision: 2 };
    const restored = restoreDeckSelection(
      [
        { id: revised.id, revision: 2 },
        { id: activeB.id, revision: activeB.revision },
        { id: archived.id, revision: archived.revision },
        { id: "missing", revision: 1 },
      ],
      [revised, activeB, archived],
    );
    expect([...deckSelectionIds(restored)]).toEqual([revised.id, activeB.id]);
  });
});

function deck(
  seed: string,
  name: string,
  overrides: Partial<DeckSummary> = {},
): DeckSummary {
  return {
    id: seed.repeat(32),
    name,
    description: `${name} deck`,
    sourceNoteCount: 2,
    generatedCardCount: 2,
    studyableDueCount: 1,
    archivedAt: null,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
    ...overrides,
  };
}
