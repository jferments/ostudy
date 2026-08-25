import { describe, expect, it } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import type { DeckLibraryOrganization } from "../../../ipc/deckFolders";
import {
  applyOptimisticLibraryMove,
  deckLifecycleSignature,
  isNoopLibraryMove,
  orderedDecksForFolder,
  partitionDecksByLifecycle,
  placementIndex,
} from "./organizationModel";

const folderA = {
  id: "f".repeat(32),
  name: "Nursing",
  sortOrder: 1_000,
  createdAt: 1,
  updatedAt: 1,
  revision: 1,
};
const folderB = { ...folderA, id: "g".repeat(32), name: "Clinical", sortOrder: 2_000 };
const alpha = deck("a", "Alpha");
const beta = deck("b", "Beta");
const gamma = deck("c", "Gamma");
const archived = { ...deck("d", "Archived"), archivedAt: 4 };

function organization(): DeckLibraryOrganization {
  return {
    revision: 7,
    folders: [folderA, folderB],
    placements: [
      { deckId: alpha.id, folderId: folderA.id, sortOrder: 2_000 },
      { deckId: beta.id, folderId: folderA.id, sortOrder: 1_000 },
      { deckId: gamma.id, folderId: null, sortOrder: 1_000 },
      { deckId: archived.id, folderId: null, sortOrder: 2_000 },
    ],
  };
}

describe("deck-library organization model", () => {
  it("partitions lifecycle state without mutating the input", () => {
    const input = Object.freeze([alpha, archived]);
    const result = partitionDecksByLifecycle(input);

    expect(result.activeDecks).toEqual([alpha]);
    expect(result.archivedDecks).toEqual([archived]);
    expect(input).toEqual([alpha, archived]);
    expect(deckLifecycleSignature([archived, alpha])).toBe(
      deckLifecycleSignature([alpha, archived]),
    );
  });

  it("orders decks by placement and then by stable name fallback", () => {
    const model = organization();
    const unplacedZulu = deck("z", "Zulu");
    const unplacedAble = deck("y", "Able");
    const placements = placementIndex(model);

    expect(orderedDecksForFolder([alpha, beta, gamma], placements, folderA.id)).toEqual([
      beta,
      alpha,
    ]);
    expect(
      orderedDecksForFolder([unplacedZulu, unplacedAble, gamma], placements, null),
    ).toEqual([gamma, unplacedAble, unplacedZulu]);
  });

  it("applies deck and folder moves immutably", () => {
    const input = organization();
    const snapshot = structuredClone(input);
    const movedDeck = applyOptimisticLibraryMove(input, {
      kind: "deck",
      deckId: alpha.id,
      targetFolderId: null,
      targetIndex: 0,
      source: "html-drag",
    });
    const movedFolder = applyOptimisticLibraryMove(input, {
      kind: "folder",
      folderId: folderA.id,
      targetIndex: 1,
      source: "pointer",
    });

    expect(input).toEqual(snapshot);
    expect(movedDeck.revision).toBe(8);
    expect(movedDeck.placements.find((placement) => placement.deckId === alpha.id)).toMatchObject({
      folderId: null,
      sortOrder: 1_000,
    });
    expect(movedDeck.placements.find((placement) => placement.deckId === beta.id)).toMatchObject({
      folderId: folderA.id,
      sortOrder: 1_000,
    });
    expect(movedFolder.folders.map((folder) => folder.id)).toEqual([folderB.id, folderA.id]);
    expect(movedFolder.folders.map((folder) => folder.sortOrder)).toEqual([1_000, 2_000]);
  });

  it("recognizes bounded no-op moves before a mutation starts", () => {
    const model = organization();
    expect(
      isNoopLibraryMove(model, [alpha, beta, gamma], {
        kind: "folder",
        folderId: folderA.id,
        targetIndex: -20,
        source: "fallback",
      }),
    ).toBe(true);
    expect(
      isNoopLibraryMove(model, [alpha, beta, gamma], {
        kind: "deck",
        deckId: beta.id,
        targetFolderId: folderA.id,
        targetIndex: 0,
        source: "keyboard",
      }),
    ).toBe(true);
    expect(
      isNoopLibraryMove(model, [alpha, beta, gamma], {
        kind: "deck",
        deckId: beta.id,
        targetFolderId: null,
        targetIndex: 99,
        source: "keyboard",
      }),
    ).toBe(false);
  });
});

function deck(seed: string, name: string): DeckSummary {
  return {
    id: seed.repeat(32),
    name,
    description: "",
    sourceNoteCount: 1,
    generatedCardCount: 1,
    studyableDueCount: 1,
    archivedAt: null,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}
