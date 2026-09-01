import { describe, expect, it } from "vitest";

import type { DeckSummary } from "../../../ipc/decks";
import type { DeckLibraryOrganization } from "../../../ipc/deckFolders";
import { applyOptimisticMove, createDeckLibrarySnapshot } from "./organizationModel";
import {
  resolveDragCandidate,
  type DragGeometrySnapshot,
} from "./dragSessionModel";

const folderA = folder("f", "Nursing", 1_000);
const folderB = folder("g", "Clinical", 2_000);
const deckA = deck("a", "Anatomy");
const deckB = deck("b", "Biology");
const deckC = deck("c", "Chemistry");
const organization: DeckLibraryOrganization = {
  revision: 4,
  folders: [folderA, folderB],
  placements: [
    { deckId: deckA.id, folderId: folderA.id, sortOrder: 1_000 },
    { deckId: deckB.id, folderId: folderA.id, sortOrder: 2_000 },
    { deckId: deckC.id, folderId: null, sortOrder: 1_000 },
  ],
};
const snapshot = createDeckLibrarySnapshot([deckA, deckB, deckC], organization);

const geometry: DragGeometrySnapshot = {
  destinations: [
    {
      key: "folder:a",
      folderId: folderA.id,
      name: folderA.name,
      collapsed: false,
      rect: rect(0, 0, 400, 300),
    },
    {
      key: "folder:b",
      folderId: folderB.id,
      name: folderB.name,
      collapsed: true,
      rect: rect(0, 320, 400, 440),
    },
    {
      key: "destination:unsorted",
      folderId: null,
      name: "Unsorted decks",
      collapsed: false,
      rect: rect(0, 460, 400, 700),
    },
  ],
  rows: [
    row("folder:a", "folder", folderA.id, null, 0, folderA.name, 0, 300),
    row("folder:b", "folder", folderB.id, null, 1, folderB.name, 320, 440),
    row(`deck:${deckA.id}`, "deck", deckA.id, folderA.id, 0, deckA.name, 80, 140),
    row(`deck:${deckB.id}`, "deck", deckB.id, folderA.id, 1, deckB.name, 160, 220),
    row(`deck:${deckC.id}`, "deck", deckC.id, null, 0, deckC.name, 520, 580),
  ],
};

describe("drag session candidate model", () => {
  it("uses the complete folder surface and deterministic insertion midpoints", () => {
    const emptyPadding = resolveDragCandidate(
      { kind: "deck", id: deckC.id },
      { x: 350, y: 40 },
      geometry,
      snapshot,
    );
    expect(emptyPadding).toMatchObject({
      status: "valid",
      destinationName: "Nursing",
      insertionLabel: "position 1",
      insertionTargetKey: `deck:${deckA.id}`,
      insertionEdge: "before",
      intent: {
        kind: "deck",
        deckId: deckC.id,
        targetFolderId: folderA.id,
        targetIndex: 0,
        source: "pointer",
      },
    });

    const afterFirst = resolveDragCandidate(
      { kind: "deck", id: deckC.id },
      { x: 40, y: 150 },
      geometry,
      snapshot,
    );
    expect(afterFirst.intent).toMatchObject({ targetIndex: 1 });
  });

  it("anchors before, end, and empty insertion boundaries separately from destination state", () => {
    const before = resolveDragCandidate(
      { kind: "deck", id: deckC.id },
      { x: 40, y: 100 },
      geometry,
      snapshot,
    );
    expect(before).toMatchObject({
      targetKey: "folder:a",
      insertionTargetKey: `deck:${deckA.id}`,
      insertionEdge: "before",
    });

    const end = resolveDragCandidate(
      { kind: "deck", id: deckC.id },
      { x: 40, y: 280 },
      geometry,
      snapshot,
    );
    expect(end).toMatchObject({
      targetKey: "folder:a",
      insertionTargetKey: `deck:${deckB.id}`,
      insertionEdge: "after",
    });

    const empty = resolveDragCandidate(
      { kind: "deck", id: deckC.id },
      { x: 40, y: 360 },
      geometry,
      snapshot,
    );
    expect(empty).toMatchObject({
      targetKey: "folder:b",
      insertionTargetKey: "folder:b",
      insertionEdge: "empty",
    });
  });

  it("distinguishes valid, no-op, and invalid candidates", () => {
    expect(
      resolveDragCandidate(
        { kind: "deck", id: deckA.id },
        { x: 40, y: 90 },
        geometry,
        snapshot,
      ).status,
    ).toBe("noop");
    expect(
      resolveDragCandidate(
        { kind: "deck", id: deckA.id },
        { x: 40, y: 360 },
        geometry,
        snapshot,
      ).status,
    ).toBe("valid");
    expect(
      resolveDragCandidate(
        { kind: "folder", id: folderA.id },
        { x: 40, y: 600 },
        geometry,
        snapshot,
      ),
    ).toMatchObject({ status: "invalid", intent: null });
  });

  it("resolves folder reorder positions independently of deck rows", () => {
    expect(
      resolveDragCandidate(
        { kind: "folder", id: folderA.id },
        { x: 20, y: 430 },
        geometry,
        snapshot,
      ),
    ).toMatchObject({
      status: "valid",
      intent: { kind: "folder", folderId: folderA.id, targetIndex: 1 },
    });
  });

  it.each([
    ["A below B", "a", 150, 1, ["b", "a", "c"]],
    ["A below C", "a", 250, 2, ["b", "c", "a"]],
    ["C above B", "c", 110, 1, ["a", "c", "b"]],
  ] as const)(
    "normalizes folder insertion after source removal for %s",
    (_label, sourceSeed, pointerY, targetIndex, expectedOrder) => {
      const folders = [
        folder("a", "A", 1_000),
        folder("b", "B", 2_000),
        folder("c", "C", 3_000),
      ];
      const threeFolderOrganization: DeckLibraryOrganization = {
        revision: 9,
        folders,
        placements: [],
      };
      const threeFolderSnapshot = createDeckLibrarySnapshot([], threeFolderOrganization);
      const threeFolderGeometry: DragGeometrySnapshot = {
        destinations: [],
        rows: folders.map((candidate, index) =>
          row(
            `folder:${candidate.id}`,
            "folder",
            candidate.id,
            null,
            index,
            candidate.name,
            index * 100,
            index * 100 + 100,
          ),
        ),
      };

      const candidate = resolveDragCandidate(
        { kind: "folder", id: sourceSeed.repeat(32) },
        { x: 20, y: pointerY },
        threeFolderGeometry,
        threeFolderSnapshot,
      );

      expect(candidate).toMatchObject({
        status: "valid",
        intent: { kind: "folder", targetIndex },
      });
      expect(candidate.intent).not.toBeNull();
      if (!candidate.intent) throw new Error("Expected a valid folder move intent.");
      const moved = applyOptimisticMove(
        threeFolderOrganization,
        candidate.intent,
      );
      expect(moved.folders.map((candidate) => candidate.name.toLowerCase())).toEqual(
        expectedOrder,
      );
    },
  );

  it("keeps hovering the dragged folder itself as an explicit no-op", () => {
    const folders = [
      folder("a", "A", 1_000),
      folder("b", "B", 2_000),
      folder("c", "C", 3_000),
    ];
    const threeFolderSnapshot = createDeckLibrarySnapshot([], {
      revision: 9,
      folders,
      placements: [],
    });
    const candidate = resolveDragCandidate(
      { kind: "folder", id: folders[1].id },
      { x: 20, y: 190 },
      {
        destinations: [],
        rows: folders.map((current, index) =>
          row(
            `folder:${current.id}`,
            "folder",
            current.id,
            null,
            index,
            current.name,
            index * 100,
            index * 100 + 100,
          ),
        ),
      },
      threeFolderSnapshot,
    );

    expect(candidate).toMatchObject({
      status: "noop",
      intent: { kind: "folder", folderId: folders[1].id, targetIndex: 1 },
    });
  });

  it("uses the smallest compatible nested destination before its ancestor", () => {
    const nestedGeometry: DragGeometrySnapshot = {
      ...geometry,
      destinations: [
        ...geometry.destinations,
        {
          key: "folder:nested",
          folderId: folderB.id,
          name: "Nested Clinical target",
          collapsed: false,
          rect: rect(40, 40, 180, 120),
        },
      ],
    };
    expect(
      resolveDragCandidate(
        { kind: "deck", id: deckC.id },
        { x: 80, y: 80 },
        nestedGeometry,
        snapshot,
      ),
    ).toMatchObject({
      destinationName: "Nested Clinical target",
      intent: { kind: "deck", targetFolderId: folderB.id, targetIndex: 0 },
    });
  });

  it("resolves Unsorted padding and rejects an unknown dragged deck", () => {
    expect(
      resolveDragCandidate(
        { kind: "deck", id: deckA.id },
        { x: 350, y: 680 },
        geometry,
        snapshot,
      ),
    ).toMatchObject({
      status: "valid",
      destinationName: "Unsorted decks",
      intent: { kind: "deck", targetFolderId: null, targetIndex: 1 },
    });
    expect(
      resolveDragCandidate(
        { kind: "deck", id: "missing" },
        { x: 350, y: 40 },
        geometry,
        snapshot,
      ),
    ).toMatchObject({ status: "invalid", intent: null });
  });
});

function rect(left: number, top: number, right: number, bottom: number) {
  return { left, top, right, bottom };
}

function row(
  key: string,
  kind: "deck" | "folder",
  id: string,
  folderId: string | null,
  index: number,
  name: string,
  top: number,
  bottom: number,
) {
  return { key, kind, id, folderId, index, name, rect: rect(0, top, 400, bottom) };
}

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
