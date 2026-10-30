import {
  useCallback,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
} from "react";

import type { AnnotatedPage } from "../../../ipc/documentAnnotations";
import type { DocumentClip } from "../../../ipc/notebooks";
import { cn } from "../../../ui/internal/cn";
import { createAnnotationId, isStickyNote, topmostAt, type Annotation, type AnnotatorTool } from "./annotationModel";
import { AnnotatorPage, StickyNoteEditor, type PageDraft, type PaperPages } from "./AnnotatorPage";
import { AnnotatorGestures, type GestureContext, type GestureEffect, type GestureSurface, type PointerSample } from "./gestures";
import { PageImageCache } from "./pageImages";
import { createTileCache } from "./pageTiles";
import { createPageTextCache, type PageTextCache } from "./textSelection";
import { attachableAt, isSegment } from "./connectors";
import { isTopic, parseList, visibleMarks } from "./mindMap";
import type { DocumentAnnotatorController } from "./useDocumentAnnotator";
import { MAX_SCALE, type useAnnotatorViewport } from "./useAnnotatorViewport";

type Viewport = ReturnType<typeof useAnnotatorViewport>;

const EMPTY: readonly Annotation[] = [];
const NUDGE = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] } as const;

const CURSORS: Readonly<Record<AnnotatorTool, string>> = {
  select: "tw:cursor-default",
  lasso: "tw:cursor-crosshair",
  pen: "tw:cursor-crosshair",
  highlighter: "tw:cursor-crosshair",
  text: "tw:cursor-text",
  sticky: "tw:cursor-copy",
  curve: "tw:cursor-crosshair",
  line: "tw:cursor-crosshair",
  arrow: "tw:cursor-crosshair",
  rectangle: "tw:cursor-crosshair",
  ellipse: "tw:cursor-crosshair",
  eraser: "tw:cursor-cell",
  clip: "tw:cursor-crosshair",
  hand: "tw:cursor-grab tw:active:cursor-grabbing",
  mindmap: "tw:cursor-cell",
};

interface AnnotatorCanvasProps {
  documentId: string;
  title: string;
  pages: readonly AnnotatedPage[];
  controller: DocumentAnnotatorController;
  viewport: Viewport;
  viewportRef: RefObject<HTMLDivElement | null>;
  contentRef: RefObject<HTMLDivElement | null>;
  coarse: boolean;
  /** A page to outline, such as the one a card's source opened at. */
  markedPage?: number | null;
  /** Pages of paper, as a notebook's are, instead of a document's page images. */
  paperPages?: PaperPages;
  /** A part of a page the Clip tool took, in points on its page. */
  onClip?: (pageNumber: number, rect: { x: number; y: number; width: number; height: number }) => void;
  /** The notebooks each page was clipped into, and how to open one. */
  clipsByPage?: ReadonlyMap<number, readonly DocumentClip[]>;
  onOpenClip?: (clip: DocumentClip) => void;
  /** A right-click or long press on a mind-map topic, which is selected first; `client` is where it was. */
  /** A right-click or long press on a mind-map topic or a line, with where it was. */
  onMarkMenu?: (mark: Annotation, client: { x: number; y: number }) => void;
}

function pointerKind(type: string): PointerSample["kind"] {
  return type === "pen" || type === "touch" ? type : "mouse";
}

function sampleOf(event: PointerEvent, button = 0): PointerSample {
  return {
    id: event.pointerId,
    kind: pointerKind(event.pointerType),
    clientX: event.clientX,
    clientY: event.clientY,
    pressure: event.pressure,
    button,
    shiftKey: event.shiftKey,
    time: event.timeStamp,
  };
}

/**
 * The scrolling pages and everything a pointer does to them. Pointer input
 * goes through the gesture state machine; its effects become drafts, edits,
 * scrolling and zoom here. Strokes in progress draw once per frame, on their
 * own page only.
 */
export function AnnotatorCanvas({
  documentId, title, pages, controller, viewport, viewportRef, contentRef, coarse, markedPage = null, paperPages,
  onClip, clipsByPage, onOpenClip, onMarkMenu,
}: AnnotatorCanvasProps) {
  const helpId = useId();
  const gestures = useMemo(() => new AnnotatorGestures(), []);
  // The cache lives as long as this effect, so a remounted canvas gets a fresh one
  // rather than a disposed one that never reads again.
  const [images, setImages] = useState<PageImageCache | null>(null);
  const [tiles, setTiles] = useState<PageImageCache | null>(null);
  useEffect(() => {
    const cache = new PageImageCache(documentId);
    const tileCache = createTileCache(documentId);
    setImages(cache);
    setTiles(tileCache);
    return () => {
      cache.dispose();
      tileCache.dispose();
    };
  }, [documentId]);
  const pageText = useRef<PageTextCache | null>(null);
  useEffect(() => {
    const cache = createPageTextCache(documentId);
    pageText.current = cache;
    return () => {
      cache.dispose();
      if (pageText.current === cache) pageText.current = null;
    };
  }, [documentId]);
  const [draft, setDraft] = useState<PageDraft | null>(null);
  const [preview, setPreview] = useState<Annotation | null>(null);
  const [erasing, setErasing] = useState<ReadonlyMap<number, ReadonlySet<string>>>(() => new Map());
  const [loop, setLoop] = useState<{ pageNumber: number; points: readonly number[] } | null>(null);
  const [groupPreview, setGroupPreview] = useState<readonly Annotation[] | null>(null);
  // Strokes the eraser is cutting, by page: each drawn as what is left of it.
  const [cutting, setCutting] = useState<ReadonlyMap<number, ReadonlyMap<string, readonly Annotation[]>>>(() => new Map());
  const pendingDraft = useRef<PageDraft | null>(null);
  const draftFrame = useRef<number | null>(null);
  const pageSizes = useMemo(() => new Map(pages.map((page) => [page.pageNumber, page])), [pages]);
  const rects = useRef<Map<number, DOMRect> | null>(null);

  const { scale, near, settled } = viewport;
  const latest = useRef({ controller, scale, viewport, onClip });
  latest.current = { controller, scale, viewport, onClip };

  // The highlighter snaps to text, so it reads the text of the pages near the view.
  // Paper has no text to read.
  const highlighting = controller.tool === "highlighter" && !paperPages;
  useEffect(() => {
    if (!highlighting) return;
    for (const pageNumber of near) {
      const page = pageSizes.get(pageNumber);
      if (page) pageText.current?.request(page);
    }
  }, [highlighting, near, pageSizes]);

  // Page positions only change with scrolling, zoom and resizing.
  useEffect(() => { rects.current = null; }, [scale, near, pages]);

  const pageRects = useCallback((): Map<number, DOMRect> => {
    if (rects.current) return rects.current;
    const measured = new Map<number, DOMRect>();
    contentRef.current?.querySelectorAll<HTMLElement>("[data-annotator-page]").forEach((element) => {
      const pageNumber = Number(element.dataset.annotatorPage);
      if (latest.current.viewport.near.has(pageNumber)) measured.set(pageNumber, element.getBoundingClientRect());
    });
    rects.current = measured;
    return measured;
  }, [contentRef]);

  const surface = useMemo<GestureSurface>(() => ({
    pageAt(clientX, clientY) {
      for (const [pageNumber, rect] of pageRects()) {
        if (clientX >= rect.left && clientX <= rect.right && clientY >= rect.top && clientY <= rect.bottom) return pageNumber;
      }
      return null;
    },
    nearestPage(clientX, clientY) {
      let best: { pageNumber: number; distance: number } | null = null;
      for (const [pageNumber, rect] of pageRects()) {
        const dx = Math.max(rect.left - clientX, 0, clientX - rect.right);
        const dy = Math.max(rect.top - clientY, 0, clientY - rect.bottom);
        const distance = Math.hypot(dx, dy);
        if (!best || distance < best.distance) best = { pageNumber, distance };
      }
      return best?.pageNumber ?? null;
    },
    toPage(pageNumber, clientX, clientY) {
      const rect = pageRects().get(pageNumber);
      const scale = latest.current.scale;
      return rect ? { x: (clientX - rect.left) / scale, y: (clientY - rect.top) / scale } : { x: 0, y: 0 };
    },
    pageSize(pageNumber) {
      return pageSizes.get(pageNumber) ?? null;
    },
  }), [pageRects, pageSizes]);

  // An outline pasted onto a notebook page becomes a mind map in the middle of the view.
  const paste = useCallback((event: ClipboardEvent<HTMLDivElement>) => {
    const { controller: current } = latest.current;
    const element = viewportRef.current;
    if (!current.notebook || !element || (event.target instanceof HTMLElement && event.target.closest("textarea, input, [contenteditable]"))) return;
    const items = parseList(event.clipboardData.getData("text/plain"));
    if (items.length < 2) return;
    const view = element.getBoundingClientRect();
    const middle = { x: view.left + view.width / 2, y: view.top + view.height / 2 };
    const pageNumber = surface.pageAt(middle.x, middle.y) ?? surface.nearestPage(middle.x, middle.y);
    const size = pageNumber === null ? null : surface.pageSize(pageNumber);
    if (pageNumber === null || !size) return;
    event.preventDefault();
    current.pasteList(items, pageNumber, surface.toPage(pageNumber, middle.x, middle.y), size);
  }, [surface, viewportRef]);

  const context = useCallback((): GestureContext => {
    const { controller: current, scale: currentScale } = latest.current;
    return {
      tool: current.tool,
      styles: current.styles,
      penOnly: current.penOnly,
      eraseMode: current.eraseMode,
      scale: currentScale,
      minScale: latest.current.viewport.minScale,
      maxScale: MAX_SCALE,
      selected: current.selected,
      group: current.group,
      // Topics folded away can be neither seen nor picked.
      annotationsOn: (pageNumber) => visibleMarks(current.annotationsRef.current.get(pageNumber) ?? EMPTY),
      textHeight: current.textHeight,
      pageText: (pageNumber) => pageText.current?.get(pageNumber) ?? null,
      newId: createAnnotationId,
      now: current.now,
    };
  }, []);

  const showDraft = useCallback((next: PageDraft | null) => {
    pendingDraft.current = next;
    if (next === null) {
      if (draftFrame.current !== null) cancelAnimationFrame(draftFrame.current);
      draftFrame.current = null;
      setDraft(null);
      return;
    }
    draftFrame.current ??= requestAnimationFrame(() => {
      draftFrame.current = null;
      setDraft(pendingDraft.current);
    });
  }, []);
  useEffect(() => () => { if (draftFrame.current !== null) cancelAnimationFrame(draftFrame.current); }, []);

  const run = useCallback((effects: readonly GestureEffect[]) => {
    const { controller: current, viewport: view } = latest.current;
    for (const effect of effects) {
      switch (effect.kind) {
        case "draft":
          showDraft(effect.draft);
          break;
        case "create":
          current.commit([{ id: effect.annotation.id, before: null, after: effect.annotation }]);
          break;
        case "preview":
          setPreview(effect.annotation);
          break;
        case "change":
          current.commit([{ id: effect.after.id, before: effect.before, after: effect.after }]);
          break;
        case "erasing": {
          const byPage = new Map<number, Set<string>>();
          for (const annotation of effect.annotations) {
            const ids = byPage.get(annotation.pageNumber) ?? new Set<string>();
            ids.add(annotation.id);
            byPage.set(annotation.pageNumber, ids);
          }
          setErasing(byPage);
          const cutByPage = new Map<number, Map<string, readonly Annotation[]>>();
          for (const { original, kept } of effect.cuts ?? []) {
            const page = cutByPage.get(original.pageNumber) ?? new Map<string, readonly Annotation[]>();
            page.set(original.id, kept);
            cutByPage.set(original.pageNumber, page);
          }
          setCutting(cutByPage);
          break;
        }
        case "erase":
          // A cut stroke is replaced by what is left of it, in one step to undo.
          current.commit([
            ...effect.annotations.map((annotation) => ({ id: annotation.id, before: annotation, after: null })),
            ...(effect.cuts ?? []).flatMap(({ original, kept }) => [
              { id: original.id, before: original, after: null },
              ...kept.map((annotation) => ({ id: annotation.id, before: null, after: annotation })),
            ]),
          ]);
          break;
        case "select":
          current.select(effect.id);
          break;
        case "lasso":
          setLoop(effect.loop);
          break;
        case "clip":
          latest.current.onClip?.(effect.pageNumber, effect.rect);
          break;
        case "select-group":
          current.selectGroup(effect.pageNumber, effect.ids);
          break;
        case "group-preview":
          setGroupPreview(effect.annotations);
          break;
        case "group-change":
          current.moveGroup(effect.after);
          break;
        case "edit-text":
          current.editText(effect.annotation);
          break;
        case "new-text": {
          const size = pageSizes.get(effect.pageNumber);
          if (size) current.newText(effect.pageNumber, effect.point, size, effect.sticky);
          break;
        }
        case "new-map": {
          const size = pageSizes.get(effect.pageNumber);
          if (size && effect.free) current.newFree(effect.pageNumber, effect.point, size);
          else if (size) current.newMap(effect.pageNumber, effect.point, size);
          break;
        }
        case "scroll-by": {
          const element = viewportRef.current;
          if (element) {
            element.scrollLeft += effect.dx;
            element.scrollTop += effect.dy;
          }
          break;
        }
        case "fling":
          view.startFling(effect.velocityX, effect.velocityY);
          break;
        case "zoom":
          view.zoomTo(effect.scale, effect.anchor, effect.clientX, effect.clientY);
          break;
        case "pen-seen":
          if (!current.penSeen) current.notePen();
          break;
      }
    }
  }, [pageSizes, showDraft, viewportRef]);

  // A new tool starts clean: nothing half drawn or half moved carries over.
  useEffect(() => { run(gestures.reset()); }, [controller.tool, gestures, run]);

  // A right-click (or a long press) on a topic or a line opens what can be done with it.
  const contextMenu = useCallback((event: MouseEvent<HTMLDivElement>) => {
    const { controller: current } = latest.current;
    if (!onMarkMenu || (event.target instanceof HTMLElement && event.target.closest("textarea, input, [contenteditable]"))) return;
    // The toolbar above may have changed height since the pages were last measured.
    rects.current = null;
    const pageNumber = surface.pageAt(event.clientX, event.clientY);
    if (pageNumber === null) return;
    const point = surface.toPage(pageNumber, event.clientX, event.clientY);
    const marks = visibleMarks(current.annotationsRef.current.get(pageNumber) ?? EMPTY);
    const mark = topmostAt(marks, point, 4, current.textHeight, (annotation) => isTopic(annotation) || isSegment(annotation));
    if (!mark) return;
    event.preventDefault();
    run(gestures.reset());
    current.finishEditing();
    current.select(mark.id, pageNumber);
    onMarkMenu(mark, { x: event.clientX, y: event.clientY });
  }, [gestures, onMarkMenu, run, surface]);

  function pointerDown(event: ReactPointerEvent<HTMLDivElement>) {
    if ((event.target as HTMLElement).closest("textarea")) return;
    viewport.stopFling();
    rects.current = null;
    // Focus moves here, which also closes an open text note.
    event.currentTarget.focus({ preventScroll: true });
    if (controller.linking) {
      // Linking: the mark pressed is the other end; pressing anything else stops.
      const pageNumber = surface.pageAt(event.clientX, event.clientY);
      const target = pageNumber === null ? null : attachableAt(
        visibleMarks(controller.annotationsRef.current.get(pageNumber) ?? EMPTY),
        surface.toPage(pageNumber, event.clientX, event.clientY),
        controller.textHeight,
      );
      if (!target || !controller.linkTo(target)) controller.cancelLink();
      return;
    }
    try {
      event.currentTarget.setPointerCapture(event.pointerId);
    } catch {
      // A pointer the browser already released cannot be captured; its events still arrive.
    }
    run(gestures.down(sampleOf(event.nativeEvent, event.button), surface, context()));
  }

  function pointerMove(event: ReactPointerEvent<HTMLDivElement>) {
    const native = event.nativeEvent;
    const samples = typeof native.getCoalescedEvents === "function" ? native.getCoalescedEvents() : [];
    for (const sample of samples.length > 0 ? samples : [native]) {
      run(gestures.move(sampleOf(sample), surface, context()));
    }
  }

  function pointerUp(event: ReactPointerEvent<HTMLDivElement>) {
    run(gestures.up(sampleOf(event.nativeEvent), surface, context()));
  }

  function pointerCancel(event: ReactPointerEvent<HTMLDivElement>) {
    run(gestures.cancel(event.pointerId));
  }

  function keyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (controller.linking && event.key === "Escape") {
      // Stops linking, and nothing else.
      event.preventDefault();
      event.stopPropagation();
      controller.cancelLink();
      return;
    }
    const selected = controller.selected;
    const lassoed = controller.group[0];
    if (lassoed && !selected && !event.altKey && !event.ctrlKey && !event.metaKey) {
      const direction = NUDGE[event.key as keyof typeof NUDGE];
      const size = pageSizes.get(lassoed.pageNumber);
      if (!direction || !size) return;
      event.preventDefault();
      const step = event.shiftKey ? 10 : 1;
      controller.nudgeGroup(direction[0] * step, direction[1] * step, size);
      return;
    }
    if (!selected || event.altKey || event.ctrlKey || event.metaKey) return;
    if (isTopic(selected) && event.target === event.currentTarget && topicKey(event)) {
      event.preventDefault();
      return;
    }
    if (event.key === "Enter" && selected.shape.kind === "text" && event.target === event.currentTarget) {
      event.preventDefault();
      controller.editText(selected);
      return;
    }
    const direction = NUDGE[event.key as keyof typeof NUDGE];
    const size = pageSizes.get(selected.pageNumber);
    if (!direction || !size) return;
    event.preventDefault();
    const step = event.shiftKey ? 10 : 1;
    controller.nudgeSelected(direction[0] * step, direction[1] * step, size);
  }

  /**
   * A mind map's keys on a selected topic: Tab adds a branch, Enter the next
   * topic, Shift+Tab moves it up a level, the arrows go from topic to topic,
   * F2 types in it and / folds it.
   */
  function topicKey(event: KeyboardEvent<HTMLDivElement>): boolean {
    switch (event.key) {
      case "Tab":
        if (event.shiftKey) controller.promoteTopic();
        else controller.addTopic("child");
        return true;
      case "Enter":
        if (event.shiftKey) return false;
        controller.addTopic("sibling");
        return true;
      case "F2":
        if (controller.selected) controller.editText(controller.selected);
        return true;
      case "/":
        controller.toggleFolded();
        return true;
      case "ArrowLeft":
      case "ArrowRight":
      case "ArrowUp":
      case "ArrowDown":
        return !event.shiftKey && controller.goToTopic(event.key);
      default:
        return false;
    }
  }

  const focusAnnotation = useCallback((annotation: Annotation) => {
    latest.current.controller.select(annotation.id, annotation.pageNumber);
  }, []);
  const openAnnotation = useCallback((annotation: Annotation) => {
    const current = latest.current.controller;
    if (annotation.shape.kind === "text") current.editText(annotation);
    else current.select(annotation.id, annotation.pageNumber);
  }, []);
  const changeText = useCallback((annotation: Annotation, text: string) => {
    if (annotation.shape.kind !== "text" || annotation.shape.text === text) return;
    latest.current.controller.commit([{ id: annotation.id, before: annotation, after: { ...annotation, shape: { ...annotation.shape, text } } }]);
  }, []);

  // A keyboard opening on a phone shrinks the view; keep the note being typed in sight.
  const typing = useRef(false);
  typing.current = controller.editing !== null;
  useEffect(() => {
    if (!typing.current) return;
    viewportRef.current?.querySelector("textarea")?.scrollIntoView?.({ block: "nearest", inline: "nearest" });
  }, [viewport.height, viewportRef]);

  // A topic closed with Escape leaves the keys with the map, for its next topic.
  const wasEditing = useRef(false);
  useEffect(() => {
    const element = viewportRef.current;
    if (wasEditing.current && !controller.editing && element
      && (document.activeElement === null || document.activeElement === document.body)) {
      element.focus({ preventScroll: true });
    }
    wasEditing.current = controller.editing !== null;
  }, [controller.editing, viewportRef]);

  const selectable = controller.tool === "select" || controller.tool === "mindmap";
  return (
    <>
      <p id={helpId} className="tw:sr-only">
        Choose a tool in the toolbar, then draw on a page. Two fingers scroll and zoom. With the Select tool, Tab
        moves through the annotations on the pages in view, arrow keys move the selected one, Delete removes it and
        Enter opens a text or sticky note. On a mind map's selected topic, Tab adds a branch, Enter adds the next
        topic, Shift+Tab moves it up a level, arrow keys go from topic to topic, F2 types in it and / folds it.
        Ctrl+Z undoes.
      </p>
      <div
        ref={viewportRef}
        tabIndex={0}
        role="group"
        aria-roledescription="annotation canvas"
        aria-label={`Pages of ${title}`}
        aria-describedby={helpId}
        data-annotator-viewport="true"
        className={cn(
          "tw:relative tw:min-w-0 tw:touch-none tw:overflow-auto tw:overscroll-contain tw:bg-muted tw:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-ring tw:focus-visible:ring-inset",
          controller.linking ? "tw:cursor-crosshair" : CURSORS[controller.tool],
        )}
        style={{ height: viewport.height }}
        onPointerDown={pointerDown}
        onPointerMove={pointerMove}
        onPointerUp={pointerUp}
        onPointerCancel={pointerCancel}
        onLostPointerCapture={pointerCancel}
        onScroll={() => { rects.current = null; }}
        onKeyDown={keyDown}
        onPaste={paste}
        onContextMenu={contextMenu}
      >
        <div ref={contentRef} className="tw:relative tw:w-max tw:min-w-full tw:p-4 tw:compact:p-2">
          {pages.map((page) => {
            const pageNumber = page.pageNumber;
            return (
              <AnnotatorPage
                key={pageNumber}
                page={page}
                scale={scale}
                near={near.has(pageNumber)}
                marked={markedPage === pageNumber}
                paperPages={paperPages}
                images={images}
                tiles={tiles}
                tileScale={settled?.scale ?? 0}
                pixelRatio={settled?.pixelRatio ?? 1}
                region={settled?.regions.get(pageNumber) ?? null}
                annotations={withCuts(controller.annotations.get(pageNumber) ?? EMPTY, cutting.get(pageNumber))}
                draft={draft?.pageNumber === pageNumber ? draft : null}
                preview={preview?.pageNumber === pageNumber ? preview : null}
                group={controller.group[0]?.pageNumber === pageNumber ? (groupPreview ?? controller.group) : EMPTY}
                loop={loop?.pageNumber === pageNumber ? loop.points : null}
                erasing={erasing.get(pageNumber) ?? null}
                selected={controller.selected?.pageNumber === pageNumber ? controller.selected : null}
                editing={controller.editing?.annotation.pageNumber === pageNumber ? controller.editing : null}
                selectable={selectable}
                coarse={coarse}
                textHeight={controller.textHeight}
                connects={controller.notebook}
                onMeasureText={controller.measureText}
                onTypeText={controller.typeText}
                onFinishText={controller.finishEditing}
                onAddTopic={controller.addTopic}
                onFocusAnnotation={focusAnnotation}
                onOpenAnnotation={openAnnotation}
                onChangeText={changeText}
                clips={clipsByPage?.get(pageNumber)}
                onOpenClip={onOpenClip}
              />
            );
          })}
        </div>
      </div>
      {/* Outside the pages, so pointers in the dialog never reach the drawing gestures. */}
      {controller.editing && isStickyNote(controller.editing.annotation.shape) ? (
        <StickyNoteEditor editing={controller.editing} onType={controller.typeText} onFinish={controller.finishEditing} />
      ) : null}
    </>
  );
}

/** A page's annotations with each stroke being cut drawn as what is left of it. */
function withCuts(annotations: readonly Annotation[], cuts: ReadonlyMap<string, readonly Annotation[]> | undefined): readonly Annotation[] {
  if (!cuts || cuts.size === 0) return annotations;
  return annotations.flatMap((annotation) => cuts.get(annotation.id) ?? [annotation]);
}
