import type { DeckSummary } from "../../../ipc/decks";
import type {
  DeckFolder,
  DeckLibraryOrganization,
  DeckPlacement,
} from "../../../ipc/deckFolders";
import type {
  DeckLifecyclePartition,
  LibraryMoveIntent,
} from "./types";

const SORT_ORDER_STEP = 1_000;

export interface DeckLibrarySnapshot {
  organization: DeckLibraryOrganization;
  placementByDeck: ReadonlyMap<string, DeckPlacement>;
  folders: DeckFolder[];
  activeDecks: DeckSummary[];
  archivedDecks: DeckSummary[];
  unsortedDecks: DeckSummary[];
  decksByFolder: ReadonlyMap<string, DeckSummary[]>;
}

export type MoveIntentValidation =
  | { status: "invalid"; reason: string }
  | { status: "noop"; intent: LibraryMoveIntent }
  | { status: "valid"; intent: LibraryMoveIntent };

function compareFolders(left: DeckFolder, right: DeckFolder): number {
  return (
    left.sortOrder - right.sortOrder ||
    left.name.localeCompare(right.name) ||
    left.id.localeCompare(right.id)
  );
}

function compareDecks(
  placementByDeck: ReadonlyMap<string, Pick<DeckPlacement, "sortOrder">>,
  left: DeckSummary,
  right: DeckSummary,
): number {
  const leftOrder = placementByDeck.get(left.id)?.sortOrder ?? Number.MAX_SAFE_INTEGER;
  const rightOrder = placementByDeck.get(right.id)?.sortOrder ?? Number.MAX_SAFE_INTEGER;
  return leftOrder - rightOrder || left.name.localeCompare(right.name) || left.id.localeCompare(right.id);
}

export function partitionDecksByLifecycle(decks: readonly DeckSummary[]): DeckLifecyclePartition {
  const active: DeckSummary[] = [];
  const archived: DeckSummary[] = [];
  for (const deck of decks) {
    (deck.archivedAt === null ? active : archived).push(deck);
  }
  return { active, archived };
}

export function deckLifecycleSignature(decks: readonly DeckSummary[]): string {
  return decks
    .map((deck) => `${deck.id}:${deck.archivedAt ?? "active"}`)
    .sort()
    .join("|");
}

export function createPlacementIndex(
  placements: readonly DeckPlacement[],
): ReadonlyMap<string, DeckPlacement> {
  return new Map(placements.map((placement) => [placement.deckId, placement]));
}

export function orderedFolders(organization: DeckLibraryOrganization): DeckFolder[] {
  return [...organization.folders].sort(compareFolders);
}

export function orderedDecksForFolder(
  decks: readonly DeckSummary[],
  placementByDeck: ReadonlyMap<string, Pick<DeckPlacement, "folderId" | "sortOrder">>,
  folderId: string | null,
): DeckSummary[] {
  return decks
    .filter((deck) => (placementByDeck.get(deck.id)?.folderId ?? null) === folderId)
    .sort((left, right) => compareDecks(placementByDeck, left, right));
}

export function createDeckLibrarySnapshot(
  decks: readonly DeckSummary[],
  organization: DeckLibraryOrganization,
): DeckLibrarySnapshot {
  const placementByDeck = createPlacementIndex(organization.placements);
  const lifecycle = partitionDecksByLifecycle(decks);
  const folders = orderedFolders(organization);
  const decksByFolder = new Map<string, DeckSummary[]>();
  for (const folder of folders) {
    decksByFolder.set(
      folder.id,
      orderedDecksForFolder(lifecycle.active, placementByDeck, folder.id),
    );
  }
  return {
    organization,
    placementByDeck,
    folders,
    activeDecks: lifecycle.active,
    archivedDecks: lifecycle.archived,
    unsortedDecks: orderedDecksForFolder(lifecycle.active, placementByDeck, null),
    decksByFolder,
  };
}

function clampIndex(targetIndex: number, maximum: number): number {
  if (!Number.isFinite(targetIndex)) return 0;
  return Math.min(Math.max(0, Math.trunc(targetIndex)), Math.max(0, maximum));
}

export function validateMoveIntent(
  snapshot: DeckLibrarySnapshot,
  intent: LibraryMoveIntent,
): MoveIntentValidation {
  if (intent.kind === "folder") {
    const currentIndex = snapshot.folders.findIndex((folder) => folder.id === intent.folderId);
    if (currentIndex < 0) {
      return { status: "invalid", reason: `Unknown folder: ${intent.folderId}` };
    }
    const targetIndex = clampIndex(intent.targetIndex, snapshot.folders.length - 1);
    const normalized = { ...intent, targetIndex };
    return currentIndex === targetIndex
      ? { status: "noop", intent: normalized }
      : { status: "valid", intent: normalized };
  }

  const deck = snapshot.activeDecks.find((candidate) => candidate.id === intent.deckId);
  if (!deck) {
    return { status: "invalid", reason: `Unknown or archived deck: ${intent.deckId}` };
  }
  if (
    intent.targetFolderId !== null &&
    !snapshot.folders.some((folder) => folder.id === intent.targetFolderId)
  ) {
    return { status: "invalid", reason: `Unknown target folder: ${intent.targetFolderId}` };
  }

  const currentPlacement = snapshot.placementByDeck.get(intent.deckId);
  const currentFolderId = currentPlacement?.folderId ?? null;
  const currentGroup = orderedDecksForFolder(
    snapshot.activeDecks,
    snapshot.placementByDeck,
    currentFolderId,
  );
  const currentIndex = currentGroup.findIndex((candidate) => candidate.id === intent.deckId);
  const targetGroup = orderedDecksForFolder(
    snapshot.activeDecks.filter((candidate) => candidate.id !== intent.deckId),
    snapshot.placementByDeck,
    intent.targetFolderId,
  );
  const targetIndex = clampIndex(intent.targetIndex, targetGroup.length);
  const normalized = { ...intent, targetIndex };

  if (currentFolderId === intent.targetFolderId && currentIndex === targetIndex) {
    return { status: "noop", intent: normalized };
  }
  return { status: "valid", intent: normalized };
}

function normalizePlacementGroup(
  placements: DeckPlacement[],
  folderId: string | null,
  orderedIds: readonly string[],
): void {
  const placementById = new Map(placements.map((placement) => [placement.deckId, placement]));
  orderedIds.forEach((deckId, index) => {
    const placement = placementById.get(deckId);
    if (placement && placement.folderId === folderId) {
      placement.sortOrder = (index + 1) * SORT_ORDER_STEP;
    }
  });
}

export function applyOptimisticMove(
  organization: DeckLibraryOrganization,
  intent: LibraryMoveIntent,
): DeckLibraryOrganization {
  if (intent.kind === "folder") {
    const moved = organization.folders.find((folder) => folder.id === intent.folderId);
    if (!moved) return organization;
    const folders = orderedFolders(organization)
      .filter((folder) => folder.id !== intent.folderId)
      .map((folder) => ({ ...folder }));
    const targetIndex = clampIndex(intent.targetIndex, folders.length);
    folders.splice(targetIndex, 0, { ...moved });
    return {
      ...organization,
      revision: organization.revision + 1,
      folders: folders.map((folder, index) => ({
        ...folder,
        sortOrder: (index + 1) * SORT_ORDER_STEP,
      })),
      placements: organization.placements.map((placement) => ({ ...placement })),
    };
  }

  const placements = organization.placements.map((placement) => ({ ...placement }));
  const moved = placements.find((placement) => placement.deckId === intent.deckId);
  if (!moved) return organization;

  const previousFolderId = moved.folderId;
  const targetIds = placements
    .filter(
      (placement) =>
        placement.folderId === intent.targetFolderId && placement.deckId !== intent.deckId,
    )
    .sort((left, right) => left.sortOrder - right.sortOrder || left.deckId.localeCompare(right.deckId))
    .map((placement) => placement.deckId);
  const targetIndex = clampIndex(intent.targetIndex, targetIds.length);
  targetIds.splice(targetIndex, 0, intent.deckId);
  moved.folderId = intent.targetFolderId;
  normalizePlacementGroup(placements, intent.targetFolderId, targetIds);

  if (previousFolderId !== intent.targetFolderId) {
    const previousIds = placements
      .filter((placement) => placement.folderId === previousFolderId)
      .sort((left, right) => left.sortOrder - right.sortOrder || left.deckId.localeCompare(right.deckId))
      .map((placement) => placement.deckId);
    normalizePlacementGroup(placements, previousFolderId, previousIds);
  }

  return {
    ...organization,
    revision: organization.revision + 1,
    folders: organization.folders.map((folder) => ({ ...folder })),
    placements,
  };
}
