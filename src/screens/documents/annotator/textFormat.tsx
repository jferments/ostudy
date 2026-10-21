import { useCallback, useMemo, useRef, useState, type ReactNode } from "react";

import type { FormulaEditSession, FormulaValue } from "../../../editor/formula";
import { FormulaDialog } from "../../../editor/FormulaDialog";
import { isStickyNote, type Annotation, type TextShape } from "./annotationModel";
import { setListStyle, type ListStyle } from "./noteText";
import type { DocumentAnnotatorController } from "./useDocumentAnnotator";

/**
 * The field of the text note being typed, while one is open. Formatting from
 * the toolbar acts on its selected lines, and `held` keeps the note open while
 * the formula editor has the focus.
 */
export const openNoteField: { current: HTMLTextAreaElement | null; held: boolean } = { current: null, held: false };

/** Lists and formulas for the text note being typed, or the selected one. */
export interface TextFormat {
  onList: (style: ListStyle) => void;
  onFormula: () => void;
}

type TextNote = Annotation & { shape: TextShape };

function plainTextNote(annotation: Annotation | null): TextNote | null {
  return annotation?.shape.kind === "text" && !isStickyNote(annotation.shape) && !annotation.shape.link
    ? annotation as TextNote : null;
}

/** A formula as note text: `$…$` within a line, or `$$…$$` on a line of its own. */
export function formulaText({ latex, placement }: FormulaValue): string {
  const source = latex.trim();
  return placement === "block" ? `$$${source}$$` : `$${source}$`;
}

/**
 * The toolbar's text formatting, while a text note is typed or selected, and
 * the formula editor it opens. A formula goes in at the caret of the note being
 * typed, or at the end of the selected note on a line of its own.
 */
export function useTextFormat(controller: DocumentAnnotatorController): { format: TextFormat | null; dialog: ReactNode } {
  const [session, setSession] = useState<FormulaEditSession | null>(null);
  const latest = useRef(controller);
  latest.current = controller;
  const editingNote = controller.editing && !isStickyNote(controller.editing.annotation.shape) ? controller.editing : null;
  const target = editingNote ? editingNote.annotation : plainTextNote(controller.selected);

  const replaceText = useCallback((note: TextNote, text: string) => {
    if (note.shape.text === text) return;
    latest.current.commit([{ id: note.id, before: note, after: { ...note, shape: { ...note.shape, text } } }]);
  }, []);

  const onList = useCallback((style: ListStyle) => {
    const current = latest.current;
    const field = openNoteField.current;
    if (current.editing && field) {
      const changed = setListStyle(current.editing.text, field.selectionStart, field.selectionEnd, style);
      current.typeText(changed.text);
      requestAnimationFrame(() => field.setSelectionRange(changed.end, changed.end));
      return;
    }
    const note = plainTextNote(current.selected);
    if (note) replaceText(note, setListStyle(note.shape.text, 0, note.shape.text.length, style).text);
  }, [replaceText]);

  const onFormula = useCallback(() => {
    const field = openNoteField.current;
    const typing = latest.current.editing !== null && field !== null;
    const caret = typing ? { start: field.selectionStart, end: field.selectionEnd } : null;
    if (typing) openNoteField.held = true;
    const finish = () => {
      setSession(null);
      openNoteField.held = false;
      if (typing && field.isConnected) field.focus({ preventScroll: true });
    };
    setSession({
      initialValue: { latex: "", placement: "inline" },
      save(value) {
        const current = latest.current;
        const inserted = formulaText(value);
        if (caret && current.editing) {
          const text = current.editing.text;
          current.typeText(`${text.slice(0, caret.start)}${inserted}${text.slice(caret.end)}`);
          const at = caret.start + inserted.length;
          requestAnimationFrame(() => field?.setSelectionRange(at, at));
        } else {
          const note = plainTextNote(current.selected);
          if (note) replaceText(note, note.shape.text ? `${note.shape.text}\n${inserted}` : inserted);
        }
        finish();
        return { ok: true };
      },
      cancel: finish,
    });
  }, [replaceText]);

  const format = useMemo(() => (target ? { onList, onFormula } : null), [onFormula, onList, target]);
  const dialog = session ? (
    // Pointers and keys in the dialog stay out of the page's gestures and shortcuts.
    <div onPointerDown={(event) => event.stopPropagation()} onKeyDown={(event) => event.stopPropagation()}>
      <FormulaDialog session={session} onClose={() => setSession(null)} />
    </div>
  ) : null;
  return { format, dialog };
}
