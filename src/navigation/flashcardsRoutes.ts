export const flashcardsWorkspaceNames = [
  "decks",
  "templates",
  "search",
  "transfer",
] as const;

export type FlashcardsWorkspaceName = (typeof flashcardsWorkspaceNames)[number];
export type LegacyFlashcardsWorkspaceName = FlashcardsWorkspaceName | "create";
export type FlashcardsDeckView = "library" | "detail" | "editor";

export interface FlashcardsRouteState {
  workspace: FlashcardsWorkspaceName;
  deckView: FlashcardsDeckView;
  deckId: string | null;
}

export interface FlashcardsRootRequest {
  requestId: number;
  workspace: FlashcardsWorkspaceName;
}

export function isFlashcardsWorkspaceName(value: unknown): value is FlashcardsWorkspaceName {
  return flashcardsWorkspaceNames.includes(value as FlashcardsWorkspaceName);
}

export function normalizeFlashcardsWorkspaceName(
  value: unknown,
  fallback: FlashcardsWorkspaceName = "decks",
): FlashcardsWorkspaceName {
  if (value === "create") return "decks";
  return isFlashcardsWorkspaceName(value) ? value : fallback;
}
