export const flashcardsWorkspaceNames = [
  "decks",
  "templates",
  "search",
  "transfer",
  "create",
] as const;

export type FlashcardsWorkspaceName = (typeof flashcardsWorkspaceNames)[number];

export function isFlashcardsWorkspaceName(value: string): value is FlashcardsWorkspaceName {
  return (flashcardsWorkspaceNames as readonly string[]).includes(value);
}
