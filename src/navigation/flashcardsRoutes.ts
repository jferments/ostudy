export const flashcardsWorkspaceNames = [
  "decks",
  "templates",
  "search",
  "transfer",
] as const;

export type FlashcardsWorkspaceName = (typeof flashcardsWorkspaceNames)[number];

export interface FlashcardsRootRoute {
  requestId: number;
  workspace: FlashcardsWorkspaceName;
  view: "root";
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
): FlashcardsRootRoute {
  return { requestId, workspace, view: "root" };
}

export function createFlashcardsDeckRoute(
  deckId: string,
  requestId: number,
): FlashcardsDeckRoute {
  return { requestId, workspace: "decks", view: "deck", deckId };
}
