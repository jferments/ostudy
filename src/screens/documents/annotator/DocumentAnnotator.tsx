import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";

import type { ShowToast } from "../../../components/Toast";
import type { AnnotatedPage } from "../../../ipc/documentAnnotations";
import { registerPlatformBackHandler } from "../../../platform/backNavigation";
import { useLayoutMode } from "../../../ui/layout/LayoutModeProvider";
import { InlineWorkspace } from "../../../ui/overlays/Dialog";
import { useConfirm } from "../../../ui/overlays/overlayContext";
import { Button } from "../../../ui/primitives/Button";
import { EmptyState, LoadingState } from "../../../ui/primitives/EmptyState";
import { InlineNotice } from "../../../ui/primitives/InlineNotice";
import { useLibraryFullPage } from "../../library/LibraryFrame";
import { isShapeTool, isStickyNote, type AnnotatorTool, type ShapeTool } from "./annotationModel";
import type { SaveState } from "./annotationSaveQueue";
import { AnnotatorCanvas } from "./AnnotatorCanvas";
import { AnnotatorToolbar } from "./AnnotatorToolbar";
import { ACTUAL_SIZE_SCALE, MAX_SCALE, MIN_SCALE, useAnnotatorViewport } from "./useAnnotatorViewport";
import { useDocumentAnnotator, type DocumentAnnotatorController } from "./useDocumentAnnotator";

interface DocumentAnnotatorProps {
  documentId: string;
  title: string;
  onClose: () => void;
  showToast: ShowToast;
}

/** How long a page opened from a card's source stays outlined. */
const CITED_PAGE_MARK_MS = 2500;

const SHORTCUTS: Readonly<Record<string, AnnotatorTool>> = {
  v: "select", p: "pen", h: "highlighter", t: "text", s: "sticky", l: "line", a: "arrow", r: "rectangle", o: "ellipse", e: "eraser",
};

/**
 * Draw, highlight and type over a document's pages. The page images are the
 * renders made when the document was imported; annotations are saved as the
 * learner works and never change the document itself.
 */
export function DocumentAnnotator({ documentId, title, onClose, showToast }: DocumentAnnotatorProps) {
  useLibraryFullPage(true);
  const controller = useDocumentAnnotator(documentId, showToast);
  const { close, closing } = useAnnotatorClose(controller, onClose);

  // Android Back closes the open note, then the selection, then the annotator.
  const back = useRef<() => void>(() => undefined);
  back.current = () => {
    if (controller.editing) controller.finishEditing();
    else if (controller.selected) controller.select(null);
    else void close();
  };
  useEffect(() => registerPlatformBackHandler(() => {
    back.current();
    return true;
  }), []);

  return (
    <InlineWorkspace
      presentation="page"
      title={title}
      description="Annotate"
      closeLabel="Back to documents"
      busy={closing}
      onClose={() => void close()}
      actions={<SaveStatus state={controller.saveState} onRetry={controller.retrySave} />}
    >
      <AnnotatorBody documentId={documentId} title={title} controller={controller} showToast={showToast} />
    </InlineWorkspace>
  );
}

/**
 * Leaving the annotator saves what is pending first, and asks before leaving
 * annotations that could not be saved.
 */
export function useAnnotatorClose(controller: DocumentAnnotatorController, onClose: () => void) {
  const confirm = useConfirm();
  const [closing, setClosing] = useState(false);
  const closingRef = useRef(false);
  const { flush } = controller;

  const close = useCallback(async () => {
    if (closingRef.current) return;
    closingRef.current = true;
    setClosing(true);
    try {
      const saved = await flush();
      if (!saved && !(await confirm({
        title: "Leave without saving?",
        description: "Some annotations could not be saved. If you leave now, they will be lost.",
        confirmLabel: "Leave anyway",
        cancelLabel: "Stay",
        destructive: true,
      }))) return;
      onClose();
    } finally {
      closingRef.current = false;
      setClosing(false);
    }
  }, [confirm, flush, onClose]);
  return { close, closing };
}

/**
 * The annotator under its frame: loading and failure states, then the toolbar
 * and pages. `focusPage` opens the pages at that page and points it out.
 */
export function AnnotatorBody({ documentId, title, controller, showToast, focusPage }: {
  documentId: string;
  title: string;
  controller: DocumentAnnotatorController;
  showToast: ShowToast;
  focusPage?: number;
}) {
  const { load } = controller;
  // A failed save is reported once; the header keeps showing it until a save works.
  const failed = controller.saveState.state === "failed" ? controller.saveState.message : null;
  useEffect(() => {
    if (failed) showToast("error", `Annotations could not be saved: ${failed}`);
  }, [failed, showToast]);

  return load.state === "loading" ? (
    <LoadingState label="Opening pages…" />
  ) : load.state === "failed" ? (
    <div className="tw:grid tw:justify-items-start tw:gap-3 tw:p-page tw:compact:p-4">
      <InlineNotice kind="error">{load.message}</InlineNotice>
      <Button onClick={controller.retryLoad}>Try again</Button>
    </div>
  ) : load.pages.length === 0 ? (
    <EmptyState
      title="No pages to annotate"
      description="This document has no page images. Pages are drawn when a PDF is imported."
    />
  ) : (
    <ReadyAnnotator documentId={documentId} title={title} pages={load.pages} controller={controller} focusPage={focusPage} />
  );
}

function ReadyAnnotator({ documentId, title, pages, controller, focusPage }: {
  documentId: string;
  title: string;
  pages: readonly AnnotatedPage[];
  controller: DocumentAnnotatorController;
  focusPage?: number;
}) {
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const viewport = useAnnotatorViewport(pages, viewportRef, contentRef);
  const cited = focusPage !== undefined && pages.some((page) => page.pageNumber === focusPage) ? focusPage : null;
  const { measured, showPage } = viewport;
  // Once the pages have their size, the cited page is scrolled to and marked for a moment.
  const [marked, setMarked] = useState<number | null>(null);
  const shown = useRef(false);
  useLayoutEffect(() => {
    if (cited === null || !measured || shown.current) return;
    shown.current = true;
    showPage(cited);
    setMarked(cited);
  }, [cited, measured, showPage]);
  useEffect(() => {
    if (marked === null) return undefined;
    const timer = setTimeout(() => setMarked(null), CITED_PAGE_MARK_MS);
    return () => clearTimeout(timer);
  }, [marked]);
  const { layoutMode, inputMode } = useLayoutMode();
  const [lastShape, setLastShape] = useState<ShapeTool>("arrow");
  const [lastNote, setLastNote] = useState<"text" | "sticky">("text");

  function chooseTool(tool: AnnotatorTool) {
    if (isShapeTool(tool)) setLastShape(tool);
    if (tool === "text" || tool === "sticky") setLastNote(tool);
    controller.setTool(tool);
  }

  function keyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.defaultPrevented) return;
    const target = event.target as HTMLElement;
    if (target.closest("textarea, input, select, [contenteditable='true'], [role='menu']")) return;
    // An open sticky note's dialog has buttons too; its keys are not shortcuts.
    if (controller.editing) return;
    const key = event.key.toLowerCase();
    if ((event.ctrlKey || event.metaKey) && !event.altKey) {
      const command = key === "z" ? (event.shiftKey ? controller.redo : controller.undo)
        : key === "y" ? controller.redo
          : key === "=" || key === "+" ? viewport.zoomIn
            : key === "-" ? viewport.zoomOut
              : key === "0" ? viewport.fitToWidth
                : null;
      if (command) {
        event.preventDefault();
        command();
      }
      return;
    }
    if (event.altKey || event.ctrlKey || event.metaKey) return;
    if (event.key === "Escape") {
      // With nothing selected, Escape leaves the annotator.
      if (controller.selected) {
        event.preventDefault();
        controller.select(null);
      }
      return;
    }
    if ((event.key === "Delete" || event.key === "Backspace") && controller.selected) {
      event.preventDefault();
      controller.deleteSelected();
      return;
    }
    const tool = event.shiftKey ? undefined : SHORTCUTS[key];
    if (tool) {
      event.preventDefault();
      chooseTool(tool);
    }
  }

  const selected = controller.selected;
  return (
    <div className="tw:flex tw:min-w-0 tw:flex-col" onKeyDown={keyDown}>
      <AnnotatorToolbar
        compact={layoutMode === "compact"}
        disabled={false}
        tool={controller.tool}
        lastShape={lastShape}
        lastNote={lastNote}
        onTool={chooseTool}
        style={controller.activeStyle}
        onStyle={controller.chooseStyle}
        selection={selected ? { isText: selected.shape.kind === "text", isSticky: isStickyNote(selected.shape) } : null}
        onDeleteSelection={controller.deleteSelected}
        onEditSelection={() => { if (selected) controller.editText(selected); }}
        canUndo={controller.canUndo}
        canRedo={controller.canRedo}
        onUndo={controller.undo}
        onRedo={controller.redo}
        penSeen={controller.penSeen}
        penOnly={controller.penOnly}
        onTogglePenOnly={controller.togglePenOnly}
        zoomPercent={Math.round(viewport.scale / ACTUAL_SIZE_SCALE * 100)}
        fit={viewport.fit}
        canZoomIn={viewport.scale < MAX_SCALE}
        canZoomOut={viewport.scale > MIN_SCALE}
        onZoomIn={viewport.zoomIn}
        onZoomOut={viewport.zoomOut}
        onFit={viewport.fitToWidth}
      />
      <AnnotatorCanvas
        documentId={documentId}
        title={title}
        pages={pages}
        controller={controller}
        viewport={viewport}
        viewportRef={viewportRef}
        contentRef={contentRef}
        coarse={inputMode === "coarse"}
        markedPage={marked}
      />
      {cited !== null ? <p role="status" className="tw:sr-only">Opened at page {cited}</p> : null}
    </div>
  );
}

export function SaveStatus({ state, onRetry }: { state: SaveState; onRetry: () => void }) {
  if (state.state === "failed") {
    return (
      <div className="tw:flex tw:items-center tw:gap-2">
        <span role="status" className="tw:text-label tw:font-medium tw:text-destructive">Not saved</span>
        <Button size="small" onClick={onRetry}>Retry</Button>
      </div>
    );
  }
  const label = state.state === "pending" || state.state === "saving" ? "Saving…" : state.state === "saved" ? "Saved" : "";
  return <span role="status" className="tw:text-label tw:whitespace-nowrap tw:text-muted-foreground">{label}</span>;
}
