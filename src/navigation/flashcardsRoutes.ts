export const flashcardsWorkspaceNames = [
  "decks",
  "templates",
  "search",
  "transfer",
] as const;

export type FlashcardsWorkspaceName = (typeof flashcardsWorkspaceNames)[number];

/** Process-local selection restoration. This is intentionally not checkpoint data. */
export interface TransientDeckSelectionEntry {
  id: string;
  revision: number;
}

export interface FlashcardsRootRoute {
  requestId: number;
  workspace: FlashcardsWorkspaceName;
  view: "root";
  restoreDeckSelection?: TransientDeckSelectionEntry[];
}

export interface FlashcardsDeckRoute {
  requestId: number;
  workspace: "decks";
  view: "deck";
  deckId: string;
}

export type FlashcardsRoute = FlashcardsRootRoute | FlashcardsDeckRoute;

export function isFlashcardsWorkspaceName(value: string): value is FlashcardsWorkspaceName {
  return (flashcardsWorkspaceNames as readonly string[]).includes(value);
}

export function normalizeFlashcardsWorkspace(
  value: string | null | undefined,
): FlashcardsWorkspaceName {
  return value && isFlashcardsWorkspaceName(value) ? value : "decks";
}

export function createFlashcardsRootRoute(
  workspace: FlashcardsWorkspaceName,
  requestId: number,
  restoreDeckSelection?: TransientDeckSelectionEntry[],
): FlashcardsRootRoute {
  return restoreDeckSelection && restoreDeckSelection.length > 0
    ? { requestId, workspace, view: "root", restoreDeckSelection }
    : { requestId, workspace, view: "root" };
}

export function createFlashcardsDeckRoute(
  deckId: string,
  requestId: number,
): FlashcardsDeckRoute {
  return { requestId, workspace: "decks", view: "deck", deckId };
}
