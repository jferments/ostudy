export const MAX_GENERATION_INSTRUCTIONS_BYTES = 16 * 1024;

export interface GenerationInstructionsInput {
  generationInstructions?: string;
}

/** UI validation mirrors native admission; native remains authoritative. Empty
 * guidance preserves the existing wire request instead of inventing a default. */
export function generationInstructionsInput(value?: string | null): GenerationInstructionsInput {
  if (value == null) return {};
  if (new TextEncoder().encode(value).length > MAX_GENERATION_INSTRUCTIONS_BYTES) {
    throw new Error("Generation instructions must fit within 16 KiB of UTF-8 text. Shorten them before starting; they have not been truncated.");
  }
  if (/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/u.test(value)) {
    throw new Error("Generation instructions contain unsupported control characters. Use ordinary text, tabs, and line breaks.");
  }
  const generationInstructions = value.replace(/\r\n?/gu, "\n").trim();
  return generationInstructions ? { generationInstructions } : {};
}
