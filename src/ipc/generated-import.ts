import { invoke } from "@tauri-apps/api/core";

// A generated JSON file and its path are untrusted input. Rust owns format-
// version checks, unknown-field rejection, Markdown conversion, card-type
// validation, duplicate handling, destination resolution, and the transaction.
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
