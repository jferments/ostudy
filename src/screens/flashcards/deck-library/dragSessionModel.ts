import type { DeckLibrarySnapshot, MoveIntentValidation } from "./organizationModel";
import { validateMoveIntent } from "./organizationModel";
import type { LibraryDragPayload, LibraryMoveIntent } from "./types";

export interface DragPoint {
  x: number;
  y: number;
}

export interface DragRect {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface DragGeometryRow {
  key: string;
  kind: "deck" | "folder";
  id: string;
  folderId: string | null;
  index: number;
  name: string;
  rect: DragRect;
}

export interface DragGeometryDestination {
  key: string;
  folderId: string | null;
  name: string;
  collapsed: boolean;
  rect: DragRect;
}

export interface DragGeometrySnapshot {
  rows: readonly DragGeometryRow[];
  destinations: readonly DragGeometryDestination[];
}

export type DragCandidateStatus = "valid" | "noop" | "invalid";

export interface DragCandidate {
  key: string;
  status: DragCandidateStatus;
  intent: LibraryMoveIntent | null;
  targetKey: string | null;
  destinationFolderId: string | null;
  destinationName: string;
  insertionLabel: string;
  announcement: string;
}

export function pointInsideRect(point: DragPoint, rect: DragRect): boolean {
  return (
    point.x >= rect.left &&
    point.x <= rect.right &&
    point.y >= rect.top &&
    point.y <= rect.bottom
  );
}

export function resolveDragCandidate(
  payload: LibraryDragPayload,
  point: DragPoint,
  geometry: DragGeometrySnapshot,
  organization: DeckLibrarySnapshot,
): DragCandidate {
  return payload.kind === "folder"
    ? resolveFolderCandidate(payload.id, point, geometry, organization)
    : resolveDeckCandidate(payload.id, point, geometry, organization);
}

function resolveDeckCandidate(
  deckId: string,
  point: DragPoint,
  geometry: DragGeometrySnapshot,
  organization: DeckLibrarySnapshot,
): DragCandidate {
  const destination = smallestContainingDestination(point, geometry.destinations);
  if (!destination) {
    return invalidCandidate("No deck destination", "Move over a folder or Unsorted decks.");
  }

  const rows = geometry.rows
    .filter(
      (row) =>
        row.kind === "deck" &&
        row.id !== deckId &&
        row.folderId === destination.folderId,
    )
    .sort((left, right) => left.index - right.index);
  const targetIndex = insertionIndex(point.y, rows);
  const intent: LibraryMoveIntent = {
    kind: "deck",
    deckId,
    targetFolderId: destination.folderId,
    targetIndex,
    source: "pointer",
  };
  return candidateFromValidation(
    validateMoveIntent(organization, intent),
    destination.key,
    destination.folderId,
    destination.name,
    `position ${targetIndex + 1}`,
  );
}

function resolveFolderCandidate(
  folderId: string,
  point: DragPoint,
  geometry: DragGeometrySnapshot,
  organization: DeckLibrarySnapshot,
): DragCandidate {
  const rows = geometry.rows
    .filter((row) => row.kind === "folder")
    .sort((left, right) => left.index - right.index);
  const containing = rows.find((row) => pointInsideRect(point, row.rect));
  if (!containing) {
    return invalidCandidate("No folder position", "Move over another folder position.");
  }
  const sourceIndex = organization.folders.findIndex((folder) => folder.id === folderId);
  if (containing.id === folderId) {
    const intent: LibraryMoveIntent = {
      kind: "folder",
      folderId,
      targetIndex: sourceIndex,
      source: "pointer",
    };
    return candidateFromValidation(
      validateMoveIntent(organization, intent),
      containing.key,
      null,
      "Folders",
      `position ${Math.max(0, sourceIndex) + 1}`,
    );
  }
  const remainingRows = rows.filter((row) => row.id !== folderId);
  const containingIndex = remainingRows.findIndex((row) => row.id === containing.id);
  const targetIndex = point.y < midpoint(containing.rect)
    ? containingIndex
    : containingIndex + 1;
  const intent: LibraryMoveIntent = {
    kind: "folder",
    folderId,
    targetIndex,
    source: "pointer",
  };
  return candidateFromValidation(
    validateMoveIntent(organization, intent),
    containing.key,
    null,
    "Folders",
    `position ${targetIndex + 1}`,
  );
}

function smallestContainingDestination(
  point: DragPoint,
  destinations: readonly DragGeometryDestination[],
): DragGeometryDestination | null {
  return (
    destinations
      .filter((destination) => pointInsideRect(point, destination.rect))
      .sort((left, right) => rectArea(left.rect) - rectArea(right.rect))[0] ?? null
  );
}

function insertionIndex(y: number, rows: readonly DragGeometryRow[]): number {
  const firstAfterPointer = rows.findIndex((row) => y < midpoint(row.rect));
  return firstAfterPointer < 0 ? rows.length : firstAfterPointer;
}

function candidateFromValidation(
  validation: MoveIntentValidation,
  targetKey: string,
  destinationFolderId: string | null,
  destinationName: string,
  insertionLabel: string,
): DragCandidate {
  if (validation.status === "invalid") {
    return invalidCandidate(
      `${destinationName}:${insertionLabel}`,
      `${destinationName}, ${insertionLabel}. Invalid move.`,
      targetKey,
      destinationFolderId,
      destinationName,
      insertionLabel,
    );
  }
  const status = validation.status;
  const announcement = `${destinationName}, ${insertionLabel}. ${
    status === "valid" ? "Valid move." : "Already in this position."
  }`;
  return {
    key: `${status}:${targetKey}:${insertionLabel}`,
    status,
    intent: validation.intent,
    targetKey,
    destinationFolderId,
    destinationName,
    insertionLabel,
    announcement,
  };
}

function invalidCandidate(
  key: string,
  announcement: string,
  targetKey: string | null = null,
  destinationFolderId: string | null = null,
  destinationName = "Unavailable",
  insertionLabel = "No insertion position",
): DragCandidate {
  return {
    key: `invalid:${key}`,
    status: "invalid",
    intent: null,
    targetKey,
    destinationFolderId,
    destinationName,
    insertionLabel,
    announcement,
  };
}

function midpoint(rect: DragRect): number {
  return rect.top + (rect.bottom - rect.top) / 2;
}

function rectArea(rect: DragRect): number {
  return Math.max(0, rect.right - rect.left) * Math.max(0, rect.bottom - rect.top);
}
