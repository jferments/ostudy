/** Stored mathematical source; renderer markup and editor-library state never cross this boundary. */
export type FormulaPlacement = "inline" | "block";

export interface FormulaValue {
  latex: string;
  placement: FormulaPlacement;
}

export type FormulaSaveResult =
  | { ok: true }
  | {
      ok: false;
      reason: "invalid-source" | "invalid-placement" | "unavailable" | "stale-document" | "unsupported-selection";
      message: string;
    };

/**
 * A single insert/edit operation captured before an overlay takes focus.
 * Invalid drafts can be corrected and saved again. A successful save or cancel
 * consumes the session. Saving never follows the editor's later live selection.
 */
export interface FormulaEditSession {
  readonly initialValue: Readonly<FormulaValue>;
  save(value: FormulaValue): FormulaSaveResult;
  cancel(): void;
}

/** Keep byte admission aligned with src-tauri/src/cards/math.rs. */
export const MAX_FORMULA_SOURCE_BYTES = 16 * 1024;

/** This admits source data, not executable TeX or a claim of typesetting validity. */
export function formulaSourceError(source: unknown): string | null {
  if (typeof source !== "string" || source.trim().length === 0) {
    return "Enter a formula before saving.";
  }
  // UTF-8 uses at least one byte per UTF-16 code unit. Bound work before encoding.
  if (source.length > MAX_FORMULA_SOURCE_BYTES) {
    return "A formula must not exceed 16,384 UTF-8 bytes.";
  }
  // TextEncoder replaces lone surrogates. Reject them rather than silently
  // persisting different source when JavaScript strings cross the UTF-8 IPC port.
  for (const character of source) {
    const code = character.codePointAt(0)!;
    if (code >= 0xd800 && code <= 0xdfff) return "A formula contains invalid Unicode.";
    if ((code < 0x20 && ![0x09, 0x0a, 0x0d].includes(code)) || (code >= 0x7f && code <= 0x9f)) {
      return "A formula contains invalid control characters.";
    }
  }
  if (new TextEncoder().encode(source).byteLength > MAX_FORMULA_SOURCE_BYTES) {
    return "A formula must not exceed 16,384 UTF-8 bytes.";
  }
  return null;
}
