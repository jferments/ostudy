import { invoke } from "@tauri-apps/api/core";

export interface FlashcardImportSample {
  cardType: "basic" | "basic_reverse" | "cloze";
  question: string;
  answer: string;
  tags: string[];
}

export interface FlashcardImportPreview {
  fingerprint: string;
  deckName: string;
  deckDescription: string;
  sourceNoteCount: number;
  generatedCardCount: number;
  basicCount: number;
  reverseEnabledCount: number;
  clozeCount: number;
  duplicateCount: number;
  uniqueTags: string[];
  samples: FlashcardImportSample[];
}

export interface FlashcardImportResult {
  deckId: string;
  createdNoteCount: number;
  createdCardCount: number;
  skippedDuplicateCount: number;
}

export async function inspectFlashcardImport(
  sourcePath: string,
  destinationDeckId?: string,
): Promise<FlashcardImportPreview> {
  return invoke<FlashcardImportPreview>("inspect_flashcard_import", {
    input: { sourcePath, destinationDeckId },
  });
}

export async function commitFlashcardImport(input: {
  sourcePath: string;
  expectedFingerprint: string;
  destinationDeckId?: string;
  allowDuplicates: boolean;
}): Promise<FlashcardImportResult> {
  return invoke<FlashcardImportResult>("commit_flashcard_import", { input });
}
