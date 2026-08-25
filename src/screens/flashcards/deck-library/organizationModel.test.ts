import { describe, expect, it } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import type { DeckLibraryOrganization } from "../../../ipc/deckFolders";
import {
  applyOptimisticMove,
  createDeckLibrarySnapshot,
  deckLifecycleSignature,
  orderedDecksForFolder,
  partitionDecksByLifecycle,
  validateMoveIntent,
} from "./organizationModel";

const folderA = folder("a", "Nursing", 2_000);
const folderB = folder("b", "Clinical", 1_000);
const activeA = deck("1", "Zulu");
const activeB = deck("2", "Alpha");
const activeC = deck("3", "Beta");
const archived = { ...deck("4", "Archive"), archivedAt: 123 };
const organization: DeckLibraryOrganization = {
  revision: 7,
  folders: [folderA, folderB],
  placements: [
    { deckId: activeA.id, folderId: folderA.id, sortOrder: 2_000 },
    { deckId: activeB.id, folderId: folderA.id, sortOrder: 1_000 },
    { deckId: activeC.id, folderId: null, sortOrder: 1_000 },
    { deckId: archived.id, folderId: null, sortOrder: 2_000 },
  ],
};

function deepFreeze<T>(value: T): T {
  if (value && typeof value === "object" && !Object.isFrozen(value)) {
    Object.freeze(value);
    for (const nested of Object.values(value as Record<string, unknown>)) deepFreeze(nested);
  }
  return value;
}

describe("deck-library organization model", () => {
  it("partitions lifecycle state without mutating or reordering the input", () => {
    const input = deepFreeze([activeA, archived, activeB]);
    expect(partitionDecksByLifecycle(input)).toEqual({
      active: [activeA, activeB],
      archived: [archived],
    });
    expect(input).toEqual([activeA, archived, activeB]);
  });

  it("produces a stable lifecycle signature independent of input order", () => {
    expect(deckLifecycleSignature([activeA, archived])).toBe(
      deckLifecycleSignature([archived, activeA]),
    );
  });

  it("orders folders and decks by normalized policy rather than backend array order", () => {
    const snapshot = createDeckLibrarySnapshot(
      [activeA, activeB, activeC, archived],
      organization,
    );
    expect(snapshot.folders.map((folder) => folder.name)).toEqual(["Clinical", "Nursing"]);
    expect(snapshot.decksByFolder.get(folderA.id)?.map((deck) => deck.name)).toEqual([
      "Alpha",
      "Zulu",
    ]);
    expect(snapshot.unsortedDecks.map((deck) => deck.name)).toEqual(["Beta"]);
    expect(snapshot.archivedDecks).toEqual([archived]);
  });

  it.each([
    {
      name: "bounds a folder move",
      intent: { kind: "folder", folderId: folderA.id, targetIndex: 99, source: "keyboard" } as const,
      status: "noop",
      targetIndex: 1,
    },
    {
      name: "rejects an unknown folder",
      intent: { kind: "folder", folderId: "missing", targetIndex: 0, source: "pointer" } as const,
      status: "invalid",
      targetIndex: undefined,
    },
    {
      name: "bounds a deck move against the target group excluding the moved deck",
      intent: {
        kind: "deck",
        deckId: activeC.id,
        targetFolderId: folderA.id,
        targetIndex: 99,
        source: "html-drag",
      } as const,
      status: "valid",
      targetIndex: 2,
    },
    {
      name: "rejects archived deck movement",
      intent: {
        kind: "deck",
        deckId: archived.id,
        targetFolderId: null,
        targetIndex: 0,
        source: "fallback",
      } as const,
      status: "invalid",
      targetIndex: undefined,
    },
  ])("$name", ({ intent, status, targetIndex }) => {
    const result = validateMoveIntent(
      createDeckLibrarySnapshot([activeA, activeB, activeC, archived], organization),
      intent,
    );
    expect(result.status).toBe(status);
    if (result.status !== "invalid") expect(result.intent.targetIndex).toBe(targetIndex);
  });

  it("applies an immutable cross-folder deck move and normalizes both groups", () => {
    const frozen = deepFreeze(structuredClone(organization));
    const result = applyOptimisticMove(frozen, {
      kind: "deck",
      deckId: activeA.id,
      targetFolderId: null,
      targetIndex: 0,
      source: "keyboard",
    });

    expect(result).not.toBe(frozen);
    expect(result.revision).toBe(8);
    expect(result.placements).toEqual([
      { deckId: activeA.id, folderId: null, sortOrder: 1_000 },
      { deckId: activeB.id, folderId: folderA.id, sortOrder: 1_000 },
      { deckId: activeC.id, folderId: null, sortOrder: 2_000 },
      { deckId: archived.id, folderId: null, sortOrder: 3_000 },
    ]);
    expect(frozen).toEqual(organization);
  });

  it("applies an immutable folder move and normalizes sort orders", () => {
    const frozen = deepFreeze(structuredClone(organization));
    const result = applyOptimisticMove(frozen, {
      kind: "folder",
      folderId: folderA.id,
      targetIndex: 0,
      source: "pointer",
    });

    expect(result.folders.map((folder) => [folder.id, folder.sortOrder])).toEqual([
      [folderA.id, 1_000],
      [folderB.id, 2_000],
    ]);
    expect(result.placements).not.toBe(frozen.placements);
    expect(frozen).toEqual(organization);
  });

  it("uses deterministic name and id tie-breakers for missing/equal placement orders", () => {
    const placementByDeck = new Map([
      [activeA.id, { folderId: null, sortOrder: 1_000 }],
      [activeB.id, { folderId: null, sortOrder: 1_000 }],
    ]);
    expect(
      orderedDecksForFolder([activeA, activeB], placementByDeck, null).map((deck) => deck.name),
    ).toEqual(["Alpha", "Zulu"]);
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

function folder(seed: string, name: string, sortOrder: number) {
  return {
    id: seed.repeat(32),
    name,
    sortOrder,
    createdAt: 1,
    updatedAt: 1,
    revision: 1,
  };
}
