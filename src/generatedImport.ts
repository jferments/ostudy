import { invoke } from "@tauri-apps/api/core";

// This module is transport-only. A generated JSON file and its path are
// untrusted input: Rust owns format-version checks, unknown-field rejection,
// Markdown conversion, card-type validation, duplicate handling, destination
// resolution, and the all-or-nothing database transaction. The interface below
// describes the successful response; it is not a browser-side validation model.
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
