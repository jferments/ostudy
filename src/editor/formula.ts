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

export { MAX_FORMULA_SOURCE_BYTES, formulaSourceError } from "../math/source";
