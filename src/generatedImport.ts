import { invoke } from "@tauri-apps/api/core";

export interface FlashcardImportResult {
  deckId: string;
  createdNoteCount: number;
  createdCardCount: number;
  skippedDuplicateCount: number;
}

export async function importGeneratedFlashcards(
  sourcePath: string,
): Promise<FlashcardImportResult> {
  return invoke<FlashcardImportResult>("import_generated_flashcards", { sourcePath });
}
