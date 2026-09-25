import type { CompletedLibraryRestore } from "../ipc/librarySession";

export function restoredLibraryMessage(result: CompletedLibraryRestore): string {
  if (!result.completed || result.restartRequired || ![
    result.deckCount, result.cardCount, result.documentCount,
  ].every((count) => Number.isSafeInteger(count) && count >= 0)) {
    throw new Error("The backup was not confirmed as the active library. Its recovery files were preserved.");
  }
  const label = (count: number, singular: string) => `${count} ${singular}${count === 1 ? "" : "s"}`;
  return `Restored ${label(result.deckCount, "deck")}, ${label(result.cardCount, "card")}, and ${label(result.documentCount, "document")}. Your library is ready.`;
}
