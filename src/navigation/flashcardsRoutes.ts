export const flashcardsWorkspaceNames = [
  "decks",
  "templates",
  "search",
  "transfer",
] as const;

export type FlashcardsWorkspaceName = (typeof flashcardsWorkspaceNames)[number];
export type FlashcardsDeckView = "library" | "detail" | "editor";

export interface FlashcardsDeckRoute {
  workspace: "decks";
  view: FlashcardsDeckView;
  deckId: string | null;
  requestId: number;
}

export interface FlashcardsRootRoute {
  workspace: Exclude<FlashcardsWorkspaceName, "decks">;
  view: "root";
  deckId: null;
  requestId: number;
}

export type FlashcardsRoute = FlashcardsDeckRoute | FlashcardsRootRoute;

export interface DeckOpenRequest {
  requestId: number;
  deckId: string;
}

export function isFlashcardsWorkspaceName(value: string): value is FlashcardsWorkspaceName {
  return (flashcardsWorkspaceNames as readonly string[]).includes(value);
}

export function normalizeFlashcardsWorkspaceName(
  value: string | null | undefined,
): FlashcardsWorkspaceName {
  return value && isFlashcardsWorkspaceName(value) ? value : "decks";
}

export function flashcardsRootRoute(
  workspace: FlashcardsWorkspaceName,
  requestId: number,
): FlashcardsRoute {
  return workspace === "decks"
    ? { workspace: "decks", view: "library", deckId: null, requestId }
    : { workspace, view: "root", deckId: null, requestId };
}

export function persistentFlashcardsView(route: FlashcardsRoute): "root" | "deck" {
  return route.workspace === "decks" && route.view !== "library" && route.deckId
    ? "deck"
    : "root";
}
