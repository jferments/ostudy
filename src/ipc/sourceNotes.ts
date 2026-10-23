import { createOperationId } from "./common";
import type { StagedExternalFile } from "./externalFiles";
import { invoke } from "./transport";

/** The longest prompt a study set can be written from. */
export const SOURCE_NOTES_PROMPT_MAX_CHARS = 4000;

/**
 * Writes study notes on what `prompt` asks to learn with the default
 * document model and stages them as a PDF document, ready to import like a
 * chosen file. A study set made without documents reads these notes.
 */
export async function writeSourceNotes(
  prompt: string,
  operationId: string = createOperationId(),
): Promise<StagedExternalFile> {
  return invoke<StagedExternalFile>("write_source_notes", { input: { operationId, prompt } });
}
