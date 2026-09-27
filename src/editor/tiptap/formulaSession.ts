import type { Editor } from "@tiptap/core";
import { closeHistory } from "@tiptap/pm/history";
import { NodeSelection, TextSelection, type Transaction } from "@tiptap/pm/state";

import { formulaSourceError, type FormulaEditSession, type FormulaSaveResult } from "../formula";

const mathNames = new Set(["inlineMath", "blockMath"]);

/** Capture the concrete selection here; dialogs consume only FormulaEditSession. */
export function beginTiptapFormulaEdit(editor: Editor): FormulaEditSession | null {
  if (editor.isDestroyed || !editor.isEditable) return null;
  const { doc, selection, schema } = editor.state;
  const selectedMath = selection instanceof NodeSelection && mathNames.has(selection.node.type.name)
    ? selection.node
    : null;
  if (!selectedMath && !(
    selection instanceof TextSelection &&
    selection.$from.sameParent(selection.$to) &&
    !selection.$from.parent.type.spec.code
  )) return null;
  const inlineMath = schema.nodes.inlineMath;
  const blockMath = schema.nodes.blockMath;
  const paragraph = schema.nodes.paragraph;
  if (!inlineMath || !blockMath || !paragraph) return null;

  let finished = false;
  const initialValue = Object.freeze({
    latex: selectedMath && typeof selectedMath.attrs.latex === "string" ? selectedMath.attrs.latex : "",
    placement: selectedMath?.type.name === "blockMath" ? "block" as const : "inline" as const,
  });
  const restoreSelection = () => {
    if (editor.isDestroyed || !editor.isEditable || editor.state.doc !== doc) return;
    editor.view.dispatch(editor.state.tr.setSelection(selection).setMeta("addToHistory", false));
    editor.view.focus();
  };

  return {
    initialValue,
    save(value): FormulaSaveResult {
      if (finished || editor.isDestroyed || !editor.isEditable) {
        return { ok: false, reason: "unavailable", message: "This formula editor is no longer active. Reopen it to edit the formula." };
      }
      // Content may have been replaced by navigation, a remote reconciliation,
      // or another edit. Never apply a remembered numeric position to a new doc.
      if (editor.state.doc !== doc) {
        return { ok: false, reason: "stale-document", message: "The card content changed. Close this dialog and reopen the formula editor." };
      }
      const sourceError = formulaSourceError(value.latex);
      if (sourceError) return { ok: false, reason: "invalid-source", message: sourceError };
      if (value.placement !== "inline" && value.placement !== "block") {
        return { ok: false, reason: "invalid-placement", message: "Choose inline or block placement." };
      }
      const formulaType = value.placement === "inline" ? inlineMath : blockMath;
      const formula = formulaType.create({ latex: value.latex }, null, []);
      let transaction: Transaction;
      try {
        transaction = editor.state.tr.setSelection(selection);
        if (selectedMath?.type === formula.type) {
          transaction.setNodeMarkup(selection.from, formula.type, formula.attrs, []);
        } else {
          // A block cannot be replaced with a bare inline child of the document,
          // list, quote or table cell. Wrap it without flattening that container.
          const replacement = value.placement === "inline" && selectedMath?.isBlock
            ? paragraph.create(null, formula)
            : formula;
          transaction.replaceSelectionWith(replacement, false);
        }
        transaction.doc.check();
      } catch (error) {
        if (!(error instanceof RangeError)) throw error;
        return { ok: false, reason: "unsupported-selection", message: "A formula cannot be placed at this selection. Choose a position in ordinary text." };
      }

      // Identical edits must not consume an undo entry. They still close the
      // session and restore the captured selection rather than the live one.
      finished = true;
      if (transaction.doc.eq(doc)) {
        restoreSelection();
        return { ok: true };
      }
      // Separate both sides of this one edit from surrounding typing. Do not
      // catch dispatch/listener failures as though a committed edit had failed.
      editor.view.dispatch(closeHistory(transaction));
      if (!editor.isDestroyed) {
        editor.view.dispatch(closeHistory(editor.state.tr));
        editor.view.focus();
      }
      return { ok: true };
    },
    cancel() {
      if (finished) return;
      finished = true;
      restoreSelection();
    },
  };
}
