import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { ShowToast } from "../../../components/Toast";
import { getErrorMessage } from "../../../ipc/common";
import {
  applyDocumentAnnotationChanges,
  getDocumentAnnotations,
  type AnnotatedPage,
  type DocumentAnnotationChange,
  type DocumentAnnotations,
  type MapLayout,
} from "../../../ipc/documentAnnotations";
import {
  applyEdits,
  EMPTY_HISTORY,
  groupByPage,
  recordStep,
  redoStep,
  undoStep,
  type AnnotationEdit,
  type AnnotationHistory,
  type PageAnnotations,
} from "./annotationHistory";
import {
  cleanNoteText,
  createAnnotationId,
  DEFAULT_STYLES,
  DEFAULT_TEXT_WIDTH,
  estimateTextHeight,
  isShapeTool,
  isStickyNote,
  isStorable,
  moveGroupWithinPage,
  moveWithinPage,
  restyle,
  sharedGroup,
  shapeStyle,
  shapeToolFor,
  styleFamilyForShape,
  styleFamilyForTool,
  STICKY_HEIGHT,
  STICKY_WIDTH,
  stickyHeight,
  TEXT_LINE_HEIGHT,
  withGroup,
  withTheirGroups,
  type ActiveStyle,
  type Annotation,
  type AnnotatorTool,
  type EraseMode,
  type Point,
  type TextShape,
  type ToolStyle,
  type ToolStyles,
} from "./annotationModel";
import { AnnotationSaveQueue, type SaveState } from "./annotationSaveQueue";
import {
  arrangeMaps,
  isTopic,
  mapsOn,
  newCentralTopic,
  newTopic,
  promote,
  retopic,
  topicToward,
  type MapTree,
  type Topic,
} from "./mindMap";

export type AnnotatorLoad =
  | { state: "loading" }
  | { state: "failed"; message: string }
  | { state: "ready"; pages: readonly AnnotatedPage[] };

/** The text note open for typing; `text` is what the field holds now. */
export interface TextEditing {
  annotation: Annotation & { shape: TextShape };
  isNew: boolean;
  text: string;
}

/**
 * Where the annotator's pages and marks come from and go: a document's, by
 * default, or a notebook's. `key` names the document or notebook.
 */
export interface AnnotatorSource {
  load: (key: string) => Promise<Pick<DocumentAnnotations, "pages" | "annotations">>;
  send: (key: string, changes: DocumentAnnotationChange[]) => Promise<unknown>;
}

const DOCUMENT_SOURCE: AnnotatorSource = { load: getDocumentAnnotations, send: applyDocumentAnnotationChanges };

/**
 * The annotator's document state: pages and annotations, the learner's tool
 * and style choices, undo history, the open text note, and background saving.
 * Every change goes through `commit`, which records one undo step and queues
 * the write.
 */
const EMPTY_GROUP: readonly Annotation[] = [];

export function useDocumentAnnotator(documentId: string, showToast: ShowToast, source: AnnotatorSource = DOCUMENT_SOURCE) {
  const [load, setLoad] = useState<AnnotatorLoad>({ state: "loading" });
  const [attempt, setAttempt] = useState(0);
  const [annotations, setAnnotations] = useState<PageAnnotations>(() => new Map());
  const annotationsRef = useRef(annotations);
  const [history, setHistoryState] = useState<AnnotationHistory>(EMPTY_HISTORY);
  const historyRef = useRef(history);
  const [selection, setSelection] = useState<{ id: string; pageNumber: number } | null>(null);
  const selectionRef = useRef(selection);
  selectionRef.current = selection;
  /** What the lasso or a Select rectangle took: one page's annotations. Selecting one annotation drops it. */
  const [lassoed, setLassoed] = useState<{ ids: readonly string[]; pageNumber: number } | null>(null);
  const [tool, setToolState] = useState<AnnotatorTool>("pen");
  const [styles, setStyles] = useState<ToolStyles>(DEFAULT_STYLES);
  const [penOnly, setPenOnly] = useState(false);
  const [eraseMode, setEraseMode] = useState<EraseMode>("whole");
  const [penSeen, setPenSeen] = useState(false);
  /** The learner's own pen-only choice, which a detected pen never overrides. */
  const penChoice = useRef<boolean | null>(null);
  const [editing, setEditingState] = useState<TextEditing | null>(null);
  const editingRef = useRef(editing);
  const [saveState, setSaveState] = useState<SaveState>({ state: "idle" });
  const lastCreated = useRef(0);
  const textHeights = useRef(new Map<string, { shape: TextShape; height: number }>());
  const pageSizes = useRef(new Map<number, { width: number; height: number }>());

  const queue = useMemo(
    () => new AnnotationSaveQueue({ documentId, send: source.send, onState: setSaveState }),
    [documentId, source],
  );

  useEffect(() => {
    let current = true;
    setLoad({ state: "loading" });
    source.load(documentId).then((loaded) => {
      if (!current) return;
      const grouped = groupByPage(loaded.annotations.map((annotation) => ({
        id: annotation.id,
        pageNumber: annotation.pageNumber,
        shape: annotation.shape,
        createdAt: annotation.createdAt,
      })));
      lastCreated.current = Math.max(0, ...loaded.annotations.map((annotation) => annotation.createdAt));
      annotationsRef.current = grouped;
      setAnnotations(grouped);
      pageSizes.current = new Map(loaded.pages.map((page) => [page.pageNumber, { width: page.width, height: page.height }]));
      setLoad({ state: "ready", pages: loaded.pages });
    }, (error: unknown) => {
      if (current) setLoad({ state: "failed", message: getErrorMessage(error) });
    });
    return () => { current = false; };
  }, [documentId, attempt, source]);

  const setHistory = useCallback((next: AnnotationHistory) => {
    historyRef.current = next;
    setHistoryState(next);
  }, []);

  const setEditing = useCallback((next: TextEditing | null) => {
    editingRef.current = next;
    setEditingState(next);
  }, []);

  const apply = useCallback((edits: readonly AnnotationEdit[]) => {
    if (edits.length === 0) return;
    const next = applyEdits(annotationsRef.current, edits);
    annotationsRef.current = next;
    setAnnotations(next);
    queue.record(edits);
  }, [queue]);

  const textHeight = useCallback((annotation: Annotation): number => {
    if (annotation.shape.kind !== "text") return 0;
    if (isStickyNote(annotation.shape)) return stickyHeight(annotation.shape);
    const measured = textHeights.current.get(annotation.id);
    return measured?.shape === annotation.shape ? measured.height : estimateTextHeight(annotation.shape);
  }, []);

  /** Applies edits as one undoable step; mind maps they touch are tidied in the same step. */
  const commit = useCallback((edits: readonly AnnotationEdit[]) => {
    const storable = edits.filter((edit) => !edit.after || isStorable(edit.after.shape));
    const kept = arrangeMaps(storable, annotationsRef.current, textHeight, (pageNumber) => pageSizes.current.get(pageNumber) ?? null);
    if (kept.length === 0) return;
    apply(kept);
    setHistory(recordStep(historyRef.current, kept));
  }, [apply, setHistory, textHeight]);

  const find = useCallback((id: string, pageNumber: number): Annotation | null => (
    annotationsRef.current.get(pageNumber)?.find((annotation) => annotation.id === id) ?? null
  ), []);

  const selected = selection
    ? annotations.get(selection.pageNumber)?.find((annotation) => annotation.id === selection.id) ?? null
    : null;
  // An undo or an erase can remove the selected annotation.
  useEffect(() => {
    if (selection && !selected) setSelection(null);
  }, [selected, selection]);
  useEffect(() => {
    if (selection) setLassoed(null);
  }, [selection]);

  const group = useMemo(() => {
    if (!lassoed) return EMPTY_GROUP;
    const page = annotations.get(lassoed.pageNumber) ?? [];
    return page.filter((annotation) => lassoed.ids.includes(annotation.id));
  }, [annotations, lassoed]);
  // An undo or an erase can take away the whole group too.
  useEffect(() => {
    if (lassoed && group.length === 0) setLassoed(null);
  }, [group, lassoed]);

  /** Takes these annotations of one page as the group; none clears it. */
  const selectGroup = useCallback((pageNumber: number, ids: readonly string[]) => {
    setSelection(null);
    setLassoed(ids.length > 0 ? { ids, pageNumber } : null);
  }, []);


  /** Topics whose drawn height differs from the one their map was laid out with. */
  const remeasured = useRef(new Map<string, number>());
  const relayout = useCallback(() => {
    const pending = [...remeasured.current];
    remeasured.current.clear();
    const edits: AnnotationEdit[] = pending.flatMap(([id, pageNumber]) => {
      const topic = annotationsRef.current.get(pageNumber)?.find((annotation) => annotation.id === id);
      return topic ? [{ id, before: topic, after: topic }] : [];
    });
    // Not a step of its own: the map only settles around what was drawn.
    apply(arrangeMaps(edits, annotationsRef.current, textHeight, (pageNumber) => pageSizes.current.get(pageNumber) ?? null));
  }, [apply, textHeight]);

  const measureText = useCallback((annotation: Annotation, height: number) => {
    if (annotation.shape.kind !== "text") return;
    const laidOut = textHeight(annotation);
    textHeights.current.set(annotation.id, { shape: annotation.shape, height });
    if (!isTopic(annotation) || Math.abs(laidOut - height) < 0.5) return;
    if (remeasured.current.size === 0) queueMicrotask(relayout);
    remeasured.current.set(annotation.id, annotation.pageNumber);
  }, [relayout, textHeight]);

  /** Creation times only increase, so a new annotation is always drawn on top. */
  const now = useCallback(() => {
    lastCreated.current = Math.max(Date.now(), lastCreated.current + 1);
    return lastCreated.current;
  }, []);

  /** Selects one annotation; one in a group selects its whole group instead. */
  const select = useCallback((id: string | null, pageNumber?: number) => {
    if (id === null) {
      setSelection(null);
      return;
    }
    const pages = pageNumber !== undefined ? [pageNumber] : [...annotationsRef.current.keys()];
    for (const page of pages) {
      const list = annotationsRef.current.get(page) ?? [];
      const annotation = list.find((candidate) => candidate.id === id);
      if (!annotation) continue;
      if (annotation.shape.group) {
        setSelection(null);
        setLassoed({ ids: withTheirGroups(list, [annotation]).map((member) => member.id), pageNumber: page });
      } else {
        setSelection({ id, pageNumber: page });
      }
      return;
    }
    if (pageNumber !== undefined) setSelection({ id, pageNumber });
  }, []);

  /** Closes the open text note, keeping what was typed; an emptied note is removed. */
  const finishEditing = useCallback(() => {
    const open = editingRef.current;
    if (!open) return;
    setEditing(null);
    const text = cleanNoteText(open.text);
    const before = open.isNew ? null : find(open.annotation.id, open.annotation.pageNumber);
    if (text.trim().length === 0) {
      if (before) commit([{ id: before.id, before, after: null }]);
      return;
    }
    const after = { ...open.annotation, shape: { ...open.annotation.shape, text } };
    if (before && before.shape.kind === "text" && before.shape.text === text) {
      setSelection({ id: before.id, pageNumber: before.pageNumber });
      return;
    }
    commit([{ id: after.id, before, after }]);
    setSelection({ id: after.id, pageNumber: after.pageNumber });
  }, [commit, find, setEditing]);

  const editText = useCallback((annotation: Annotation) => {
    if (annotation.shape.kind !== "text") return;
    finishEditing();
    setSelection({ id: annotation.id, pageNumber: annotation.pageNumber });
    setEditing({ annotation: { ...annotation, shape: annotation.shape }, isNew: false, text: annotation.shape.text });
  }, [finishEditing, setEditing]);

  /**
   * Opens a new text note with its first line at `point`, or a sticky note with
   * its corner there, kept on the page.
   */
  const newText = useCallback((pageNumber: number, point: Point, page: { width: number; height: number }, sticky = false) => {
    finishEditing();
    if (sticky) {
      const width = Math.min(STICKY_WIDTH, page.width);
      const note: TextShape = {
        kind: "text",
        x: Math.round(Math.max(0, Math.min(point.x, page.width - width)) * 100) / 100,
        y: Math.round(Math.max(0, Math.min(point.y, page.height - STICKY_HEIGHT)) * 100) / 100,
        width,
        text: "",
        color: styles.sticky.color,
        fontSize: styles.sticky.size,
        sticky: true,
      };
      setSelection(null);
      setEditing({ annotation: { id: createAnnotationId(), pageNumber, shape: note, createdAt: now() }, isNew: true, text: "" });
      return;
    }
    const fontSize = styles.text.size;
    const width = Math.min(DEFAULT_TEXT_WIDTH, page.width);
    const lineHeight = fontSize * TEXT_LINE_HEIGHT;
    const shape: TextShape = {
      kind: "text",
      x: Math.round(Math.max(0, Math.min(point.x, page.width - width)) * 100) / 100,
      y: Math.round(Math.max(0, Math.min(point.y - lineHeight / 2, page.height - lineHeight)) * 100) / 100,
      width,
      text: "",
      color: styles.text.color,
      fontSize,
    };
    setSelection(null);
    setEditing({ annotation: { id: createAnnotationId(), pageNumber, shape, createdAt: now() }, isNew: true, text: "" });
  }, [finishEditing, now, setEditing, styles.sticky, styles.text]);

  /** Opens a new central topic around `point`: a mind map starts there once it has words. */
  const newMap = useCallback((pageNumber: number, point: Point, page: { width: number; height: number }) => {
    finishEditing();
    setSelection(null);
    setEditing({ annotation: newCentralTopic(pageNumber, point, page, now()), isNew: true, text: "" });
  }, [finishEditing, now, setEditing]);

  /** The topic being typed, or else the selected one. */
  const topicTarget = useCallback((): { id: string; pageNumber: number } | null => {
    const open = editingRef.current;
    return open ? { id: open.annotation.id, pageNumber: open.annotation.pageNumber } : selectionRef.current;
  }, []);

  /** A topic as it is now in its map. */
  const topicIn = useCallback((target: { id: string; pageNumber: number } | null): { topic: Topic; tree: MapTree } | null => {
    if (!target) return null;
    const page = annotationsRef.current.get(target.pageNumber) ?? [];
    const topic = page.find((annotation) => annotation.id === target.id);
    if (!isTopic(topic)) return null;
    const tree = mapsOn(page).get(topic.shape.topic.map);
    return tree ? { topic, tree } : null;
  }, []);
  const currentTopic = useCallback(() => topicIn(topicTarget()), [topicIn, topicTarget]);

  /**
   * Opens a new topic for typing: a branch of the current topic, or the one
   * after it (a branch of the central topic, from there). The map makes room
   * for it once it has words.
   */
  const addTopic = useCallback((relation: "child" | "sibling") => {
    const target = topicTarget();
    finishEditing();
    const found = topicIn(target);
    if (!found) return;
    let { topic, tree } = found;
    const asChild = relation === "child" || topic.shape.topic.parent === undefined;
    if (asChild && topic.shape.topic.folded) {
      // A branch added under a folded topic is shown with the rest.
      commit([{ id: topic.id, before: topic, after: retopic(topic, { folded: undefined }) }]);
      const again = topicIn(target);
      if (!again) return;
      ({ topic, tree } = again);
    }
    const created = newTopic(tree, asChild ? { parent: topic } : { after: topic }, now());
    if (!created) return;
    // Opened where the map will put it.
    const placed = arrangeMaps([{ id: created.id, before: null, after: created }], annotationsRef.current, textHeight,
      (pageNumber) => pageSizes.current.get(pageNumber) ?? null).find((edit) => edit.id === created.id)?.after;
    const annotation = isTopic(placed) ? placed : created;
    setSelection(null);
    setEditing({ annotation, isNew: true, text: "" });
  }, [commit, finishEditing, now, setEditing, textHeight, topicIn, topicTarget]);

  /** Folds the selected topic's branches away, or shows them again. */
  const toggleFolded = useCallback(() => {
    const found = currentTopic();
    if (!found || (found.tree.children.get(found.topic.id) ?? []).length === 0) return;
    const { topic } = found;
    commit([{ id: topic.id, before: topic, after: retopic(topic, { folded: topic.shape.topic.folded ? undefined : true }) }]);
  }, [commit, currentTopic]);

  /** Moves the selected topic up a level, beside its parent. */
  const promoteTopic = useCallback(() => {
    const found = currentTopic();
    if (found) commit(promote(found.tree, found.topic));
  }, [commit, currentTopic]);

  /** Spreads the selected topic's map another way. */
  const setMapLayout = useCallback((layout: MapLayout) => {
    const found = currentTopic();
    const root = found?.tree.root;
    if (!root) return;
    const next = retopic(root, { layout: layout === "both" ? undefined : layout });
    if (next.shape.topic.layout !== root.shape.topic.layout) commit([{ id: root.id, before: root, after: next }]);
  }, [commit, currentTopic]);

  /** Selects the topic an arrow key leads to from the selected one; false when there is none. */
  const goToTopic = useCallback((key: "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown"): boolean => {
    const found = currentTopic();
    if (!found) return false;
    const next = topicToward(found.tree, found.topic, key);
    if (next) setSelection({ id: next.id, pageNumber: next.pageNumber });
    return true;
  }, [currentTopic]);

  const typeText = useCallback((text: string) => {
    const open = editingRef.current;
    if (open) setEditing({ ...open, text });
  }, [setEditing]);

  const setTool = useCallback((next: AnnotatorTool) => {
    finishEditing();
    if (next !== "select") setSelection(null);
    if (next !== "lasso" && next !== "select") setLassoed(null);
    setToolState(next);
  }, [finishEditing]);

  /** Restyles the selected annotation, or sets the style the current tool draws with. */
  const chooseStyle = useCallback((patch: Partial<ToolStyle>) => {
    if (selected) {
      const shape = restyle(selected.shape, patch);
      commit([{ id: selected.id, before: selected, after: { ...selected, shape } }]);
      return;
    }
    const family = styleFamilyForTool(tool);
    if (!family) return;
    setStyles((current) => ({ ...current, [family]: { ...current[family], ...patch } }));
  }, [commit, selected, tool]);

  const selectedStyle = selected ? shapeStyle(selected.shape) : null;
  const activeStyle: ActiveStyle | null = selected
    // A picture has no color or size to choose.
    ? selectedStyle && { family: styleFamilyForShape(selected.shape), style: selectedStyle, shape: shapeToolFor(selected.shape) }
    : (() => {
      const family = styleFamilyForTool(tool);
      return family ? { family, style: styles[family], shape: isShapeTool(tool) ? tool : null } : null;
    })();

  const deleteSelected = useCallback(() => {
    if (!selected) return;
    commit([{ id: selected.id, before: selected, after: null }]);
    setSelection(null);
  }, [commit, selected]);

  /**
   * Folds the selected sticky note down to its icon, or opens it back out,
   * kept on the page, as one step to undo.
   */
  const toggleStickyCollapsed = useCallback((page: { width: number; height: number }) => {
    if (!selected || !isStickyNote(selected.shape)) return;
    const { collapsed, ...open } = selected.shape;
    const shape: TextShape = collapsed
      ? {
        ...open,
        x: Math.round(Math.max(0, Math.min(open.x, page.width - open.width)) * 100) / 100,
        y: Math.round(Math.max(0, Math.min(open.y, page.height - STICKY_HEIGHT)) * 100) / 100,
      }
      : { ...open, collapsed: true };
    commit([{ id: selected.id, before: selected, after: { ...selected, shape } }]);
  }, [commit, selected]);

  /** Joins the selected annotations into one group, or takes them out of their group, as one step to undo. */
  const setGrouped = useCallback((grouped: boolean) => {
    if (group.length === 0) return;
    const name = grouped ? createAnnotationId() : null;
    commit(group.flatMap((annotation) => {
      const shape = withGroup(annotation.shape, name);
      return shape.group === annotation.shape.group ? [] : [{ id: annotation.id, before: annotation, after: { ...annotation, shape } }];
    }));
  }, [commit, group]);

  const deleteGroup = useCallback(() => {
    if (group.length === 0) return;
    commit(group.map((annotation) => ({ id: annotation.id, before: annotation, after: null })));
    setLassoed(null);
  }, [commit, group]);

  /** Moves the lassoed group together, kept on its page, as one step to undo. */
  const moveGroup = useCallback((after: readonly Annotation[]) => {
    const edits = after.flatMap((moved) => {
      const before = group.find((annotation) => annotation.id === moved.id);
      return before ? [{ id: moved.id, before, after: moved }] : [];
    });
    commit(edits);
  }, [commit, group]);

  const nudgeGroup = useCallback((dx: number, dy: number, page: { width: number; height: number }) => {
    moveGroup(moveGroupWithinPage(group, dx, dy, page, textHeight));
  }, [group, moveGroup, textHeight]);

  const nudgeSelected = useCallback((dx: number, dy: number, page: { width: number; height: number }) => {
    if (!selected) return;
    const shape = moveWithinPage(selected.shape, dx, dy, page, textHeight(selected));
    commit([{ id: selected.id, before: selected, after: { ...selected, shape } }]);
  }, [commit, selected, textHeight]);

  const undo = useCallback(() => {
    finishEditing();
    const step = undoStep(historyRef.current);
    if (!step) return;
    setHistory(step.history);
    apply(step.edits);
  }, [apply, finishEditing, setHistory]);

  const redo = useCallback(() => {
    finishEditing();
    const step = redoStep(historyRef.current);
    if (!step) return;
    setHistory(step.history);
    apply(step.edits);
  }, [apply, finishEditing, setHistory]);

  /** A pen touched the page: from now on fingers pan, unless the learner chose otherwise. */
  const notePen = useCallback(() => {
    setPenSeen(true);
    if (penChoice.current === null) setPenOnly(true);
  }, []);

  const togglePenOnly = useCallback(() => {
    setPenOnly((current) => {
      penChoice.current = !current;
      return !current;
    });
  }, []);

  /** Saves everything, including an open note; resolves false if something is still unsaved. */
  const flush = useCallback(async () => {
    finishEditing();
    const saved = await queue.flush();
    return saved && !queue.hasUnsaved();
  }, [finishEditing, queue]);

  // Leaving by another route still keeps the open note and saves in the background.
  useEffect(() => () => {
    queue.dispose();
    const open = editingRef.current;
    if (open) {
      const text = cleanNoteText(open.text);
      const before = open.isNew ? null : open.annotation;
      const after = text.trim() ? { ...open.annotation, shape: { ...open.annotation.shape, text } } : null;
      // A new note left empty was never saved, and an unchanged one needs no write.
      if ((before || after) && before?.shape.text !== after?.shape.text) {
        queue.record([{ id: open.annotation.id, before, after }]);
      }
    }
    if (!queue.hasUnsaved()) return;
    void queue.flush().then((saved) => {
      if (!saved) showToast("error", "Some annotations could not be saved.");
    });
  }, [queue, showToast]);

  return {
    load,
    retryLoad: () => setAttempt((value) => value + 1),
    annotations,
    annotationsRef,
    selected,
    select,
    group,
    selectGroup,
    deleteGroup,
    setGrouped,
    /** Whether the selected annotations are exactly one saved group. */
    isOneGroup: sharedGroup(group) !== null,
    toggleStickyCollapsed,
    moveGroup,
    nudgeGroup,
    tool,
    setTool,
    styles,
    activeStyle,
    chooseStyle,
    penOnly,
    eraseMode,
    setEraseMode,
    penSeen,
    notePen,
    togglePenOnly,
    editing,
    editText,
    newText,
    newMap,
    addTopic,
    toggleFolded,
    promoteTopic,
    setMapLayout,
    goToTopic,
    typeText,
    finishEditing,
    commit,
    deleteSelected,
    nudgeSelected,
    canUndo: history.undo.length > 0,
    canRedo: history.redo.length > 0,
    undo,
    redo,
    textHeight,
    measureText,
    now,
    saveState,
    retrySave: () => { void queue.flush(); },
    flush,
  };
}

export type DocumentAnnotatorController = ReturnType<typeof useDocumentAnnotator>;
