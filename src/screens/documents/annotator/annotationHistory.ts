import type { Annotation } from "./annotationModel";

/** One annotation before and after an edit; `null` where it does not exist. */
export interface AnnotationEdit {
  id: string;
  before: Annotation | null;
  after: Annotation | null;
}

/** Each entry is one step the learner took, such as a stroke, a move or an erase. */
export interface AnnotationHistory {
  undo: readonly (readonly AnnotationEdit[])[];
  redo: readonly (readonly AnnotationEdit[])[];
}

export type PageAnnotations = ReadonlyMap<number, readonly Annotation[]>;

export const EMPTY_HISTORY: AnnotationHistory = { undo: [], redo: [] };
const HISTORY_LIMIT = 200;

/** Records a step and forgets what could be redone. */
export function recordStep(history: AnnotationHistory, edits: readonly AnnotationEdit[]): AnnotationHistory {
  if (edits.length === 0) return history;
  return { undo: [...history.undo, edits].slice(-HISTORY_LIMIT), redo: [] };
}

function invert(edits: readonly AnnotationEdit[]): AnnotationEdit[] {
  return [...edits].reverse().map((edit) => ({ id: edit.id, before: edit.after, after: edit.before }));
}

/** The edits that take back the latest step, and the history after it. */
export function undoStep(history: AnnotationHistory): { history: AnnotationHistory; edits: AnnotationEdit[] } | null {
  const step = history.undo[history.undo.length - 1];
  if (!step) return null;
  return {
    history: { undo: history.undo.slice(0, -1), redo: [...history.redo, step] },
    edits: invert(step),
  };
}

/** The edits that repeat the latest undone step, and the history after it. */
export function redoStep(history: AnnotationHistory): { history: AnnotationHistory; edits: AnnotationEdit[] } | null {
  const step = history.redo[history.redo.length - 1];
  if (!step) return null;
  return {
    history: { undo: [...history.undo, step], redo: history.redo.slice(0, -1) },
    edits: [...step],
  };
}

function drawingOrder(a: Annotation, b: Annotation): number {
  return a.createdAt - b.createdAt || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);
}

/** Annotations grouped by page, each page in drawing order. */
export function groupByPage(annotations: readonly Annotation[]): PageAnnotations {
  const pages = new Map<number, Annotation[]>();
  for (const annotation of annotations) {
    const page = pages.get(annotation.pageNumber);
    if (page) page.push(annotation);
    else pages.set(annotation.pageNumber, [annotation]);
  }
  for (const page of pages.values()) page.sort(drawingOrder);
  return pages;
}

/**
 * The pages after a set of edits. Pages the edits do not touch keep their
 * arrays, so a page that did not change does not draw again.
 */
export function applyEdits(pages: PageAnnotations, edits: readonly AnnotationEdit[]): PageAnnotations {
  if (edits.length === 0) return pages;
  const touched = new Map<number, { removed: Set<string>; added: Map<string, Annotation> }>();
  const changesOn = (pageNumber: number) => {
    let changes = touched.get(pageNumber);
    if (!changes) {
      changes = { removed: new Set(), added: new Map() };
      touched.set(pageNumber, changes);
    }
    return changes;
  };
  for (const edit of edits) {
    if (edit.before) {
      const changes = changesOn(edit.before.pageNumber);
      changes.removed.add(edit.id);
      changes.added.delete(edit.id);
    }
    if (edit.after) changesOn(edit.after.pageNumber).added.set(edit.id, edit.after);
  }
  const next = new Map(pages);
  for (const [pageNumber, changes] of touched) {
    const kept = (pages.get(pageNumber) ?? [])
      .filter((annotation) => !changes.removed.has(annotation.id) && !changes.added.has(annotation.id));
    const page = [...kept, ...changes.added.values()].sort(drawingOrder);
    if (page.length > 0) next.set(pageNumber, page);
    else next.delete(pageNumber);
  }
  return next;
}
