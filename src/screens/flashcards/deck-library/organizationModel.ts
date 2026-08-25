import type { DeckSummary } from "../../../ipc/decks";
import type {
  DeckLibraryOrganization,
  DeckPlacement,
} from "../../../ipc/deckFolders";
import type { LibraryMoveIntent } from "./types";

const SORT_ORDER_STEP = 1_000;

export interface DeckLifecyclePartition {
  activeDecks: DeckSummary[];
  archivedDecks: DeckSummary[];
}

export function partitionDecksByLifecycle(decks: readonly DeckSummary[]): DeckLifecyclePartition {
  return decks.reduce<DeckLifecyclePartition>(
    (partition, deck) => {
      (deck.archivedAt === null ? partition.activeDecks : partition.archivedDecks).push(deck);
      return partition;
    },
    { activeDecks: [], archivedDecks: [] },
  );
}

export function deckLifecycleSignature(decks: readonly DeckSummary[]): string {
  return decks
    .map((deck) => `${deck.id}:${deck.archivedAt ?? "active"}`)
    .sort()
    .join("|");
}

export function placementIndex(
  organization: Pick<DeckLibraryOrganization, "placements"> | null,
): Map<string, DeckPlacement> {
  return new Map(
    (organization?.placements ?? []).map((placement) => [placement.deckId, placement]),
  );
}

export function orderedDecksForFolder(
  decks: readonly DeckSummary[],
  placements: ReadonlyMap<string, Pick<DeckPlacement, "folderId" | "sortOrder">>,
  folderId: string | null,
): DeckSummary[] {
  return decks
    .filter((deck) => (placements.get(deck.id)?.folderId ?? null) === folderId)
    .slice()
    .sort((left, right) => {
      const leftOrder = placements.get(left.id)?.sortOrder ?? Number.MAX_SAFE_INTEGER;
      const rightOrder = placements.get(right.id)?.sortOrder ?? Number.MAX_SAFE_INTEGER;
      return leftOrder - rightOrder || left.name.localeCompare(right.name);
    });
}

export function boundMoveIndex(targetIndex: number, itemCount: number): number {
  if (itemCount <= 0) return 0;
  return Math.min(Math.max(0, targetIndex), itemCount - 1);
}

export function normalizeLibraryMoveIntent(
  organization: DeckLibraryOrganization,
  intent: LibraryMoveIntent,
): LibraryMoveIntent {
  if (intent.kind === "folder") {
    return {
      ...intent,
      targetIndex: boundMoveIndex(intent.targetIndex, organization.folders.length),
    };
  }

  const targetCount = organization.placements.filter(
    (placement) =>
      placement.folderId === intent.targetFolderId && placement.deckId !== intent.deckId,
  ).length;
  return {
    ...intent,
    targetIndex: Math.min(Math.max(0, intent.targetIndex), targetCount),
  };
}

export function isNoopLibraryMove(
  organization: DeckLibraryOrganization,
  activeDecks: readonly DeckSummary[],
  intent: LibraryMoveIntent,
): boolean {
  const normalized = normalizeLibraryMoveIntent(organization, intent);
  if (normalized.kind === "folder") {
    const currentIndex = organization.folders.findIndex(
      (folder) => folder.id === normalized.folderId,
    );
    return currentIndex < 0 || currentIndex === normalized.targetIndex;
  }

  const placements = placementIndex(organization);
  const currentPlacement = placements.get(normalized.deckId);
  if (!currentPlacement) return true;
  if (currentPlacement.folderId !== normalized.targetFolderId) return false;
  const currentGroup = orderedDecksForFolder(activeDecks, placements, normalized.targetFolderId);
  const currentIndex = currentGroup.findIndex((deck) => deck.id === normalized.deckId);
  return currentIndex < 0 || currentIndex === normalized.targetIndex;
}

export function applyOptimisticLibraryMove(
  organization: DeckLibraryOrganization,
  intent: LibraryMoveIntent,
): DeckLibraryOrganization {
  const normalized = normalizeLibraryMoveIntent(organization, intent);
  return normalized.kind === "deck"
    ? applyOptimisticDeckMove(organization, normalized)
    : applyOptimisticFolderMove(organization, normalized);
}

function applyOptimisticDeckMove(
  organization: DeckLibraryOrganization,
  intent: Extract<LibraryMoveIntent, { kind: "deck" }>,
): DeckLibraryOrganization {
  const placements = organization.placements.map((placement) => ({ ...placement }));
  const moved = placements.find((placement) => placement.deckId === intent.deckId);
  if (!moved) return organization;

  const previousFolderId = moved.folderId;
  moved.folderId = intent.targetFolderId;
  normalizePlacementGroup(placements, intent.targetFolderId, intent.deckId, intent.targetIndex);
  if (previousFolderId !== intent.targetFolderId) {
    normalizePlacementGroup(placements, previousFolderId, null, 0);
  }

  return {
    ...organization,
    revision: organization.revision + 1,
    placements,
  };
}

function applyOptimisticFolderMove(
  organization: DeckLibraryOrganization,
  intent: Extract<LibraryMoveIntent, { kind: "folder" }>,
): DeckLibraryOrganization {
  const moved = organization.folders.find((folder) => folder.id === intent.folderId);
  if (!moved) return organization;

  const folders = organization.folders
    .filter((folder) => folder.id !== intent.folderId)
    .map((folder) => ({ ...folder }));
  folders.splice(intent.targetIndex, 0, { ...moved });
  return {
    ...organization,
    revision: organization.revision + 1,
    folders: folders.map((folder, index) => ({
      ...folder,
      sortOrder: (index + 1) * SORT_ORDER_STEP,
    })),
  };
}

function normalizePlacementGroup(
  placements: DeckPlacement[],
  folderId: string | null,
  insertedDeckId: string | null,
  targetIndex: number,
): void {
  const deckIds = placements
    .filter(
      (placement) =>
        placement.folderId === folderId && placement.deckId !== insertedDeckId,
    )
    .sort((left, right) => left.sortOrder - right.sortOrder)
    .map((placement) => placement.deckId);

  if (insertedDeckId) {
    deckIds.splice(Math.min(Math.max(0, targetIndex), deckIds.length), 0, insertedDeckId);
  }
  const orderByDeckId = new Map(deckIds.map((deckId, index) => [deckId, index]));
  for (const placement of placements) {
    const order = orderByDeckId.get(placement.deckId);
    if (order !== undefined) placement.sortOrder = (order + 1) * SORT_ORDER_STEP;
  }
}
