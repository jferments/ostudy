export const flashcardsWorkspaceNames = ["decks", "search"] as const;

/**
 * `templates` and `transfer` are retained only as compatibility inputs for
 * historical checkpoints and callers. New UI surfaces must use one of the two
 * values in `flashcardsWorkspaceNames`.
 */
export type FlashcardsWorkspaceName =
  | (typeof flashcardsWorkspaceNames)[number]
  | "templates"
  | "transfer";

export type CurrentFlashcardsWorkspaceName =
  (typeof flashcardsWorkspaceNames)[number];

/** Process-local selection restoration. This is intentionally not checkpoint data. */
export interface TransientDeckSelectionEntry {
  id: string;
  revision: number;
}

export type NonEmptyTagIds = readonly [string, ...string[]];

/** Process-local intent from Analytics. This must never enter a UI checkpoint. */
export interface AnalyticsSearchLaunch {
  requestId: number;
  source: "analytics";
  tagIds: NonEmptyTagIds;
}

export interface FlashcardsRootRoute {
  requestId: number;
  workspace: FlashcardsWorkspaceName;
  view: "root";
  restoreDeckSelection?: TransientDeckSelectionEntry[];
  analyticsSearchLaunch?: AnalyticsSearchLaunch;
}

export interface FlashcardsDeckRoute {
  requestId: number;
  workspace: "decks";
  view: "deck";
  deckId: string;
  /** Process-local library state restored when this detail route returns. */
  returnDeckSelection?: TransientDeckSelectionEntry[];
}

export type FlashcardsRoute = FlashcardsRootRoute | FlashcardsDeckRoute;

export function isFlashcardsWorkspaceName(value: string): value is FlashcardsWorkspaceName {
  return (
    value === "decks" ||
    value === "search" ||
    value === "templates" ||
    value === "transfer"
  );
}

/**
 * Templates and Transfer remain accepted as legacy checkpoint strings at
 * runtime, but both recover to Decks. Only Decks and Search/Filter are current
 * Flashcards destinations.
 */
export function normalizeFlashcardsWorkspace(
  value: string | null | undefined,
): CurrentFlashcardsWorkspaceName {
  return value === "search" ? "search" : "decks";
}

export function createFlashcardsRootRoute(
  workspace: FlashcardsWorkspaceName,
  requestId: number,
  restoreDeckSelection?: TransientDeckSelectionEntry[],
): FlashcardsRootRoute {
  const normalizedWorkspace = normalizeFlashcardsWorkspace(workspace);
  return restoreDeckSelection && restoreDeckSelection.length > 0
    ? {
        requestId,
        workspace: normalizedWorkspace,
        view: "root",
        restoreDeckSelection,
      }
    : { requestId, workspace: normalizedWorkspace, view: "root" };
}

export function createFlashcardsDeckRoute(
  deckId: string,
  requestId: number,
  returnDeckSelection?: readonly TransientDeckSelectionEntry[],
): FlashcardsDeckRoute {
  return returnDeckSelection && returnDeckSelection.length > 0
    ? {
        requestId,
        workspace: "decks",
        view: "deck",
        deckId,
        returnDeckSelection: [...returnDeckSelection],
      }
    : { requestId, workspace: "decks", view: "deck", deckId };
}

export function createAnalyticsSearchRoute(
  tagId: string,
  requestId: number,
): FlashcardsRootRoute {
  return {
    requestId,
    workspace: "search",
    view: "root",
    analyticsSearchLaunch: {
      requestId,
      source: "analytics",
      tagIds: [tagId],
    },
  };
}
