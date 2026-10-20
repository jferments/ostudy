import { useCallback, useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent } from "react";

import type { ShowToast } from "../../../components/Toast";
import { getDocumentMarkedCards, type DocumentMarkedCards } from "../../../ipc/annotationCards";
import type { AnnotatedPage, AnnotationShape } from "../../../ipc/documentAnnotations";
import { registerPlatformBackHandler } from "../../../platform/backNavigation";
import { useLayoutMode } from "../../../ui/layout/LayoutModeProvider";
import { InlineWorkspace } from "../../../ui/overlays/Dialog";
import { useConfirm } from "../../../ui/overlays/overlayContext";
import { Button } from "../../../ui/primitives/Button";
import { EmptyState, LoadingState } from "../../../ui/primitives/EmptyState";
import { InlineNotice } from "../../../ui/primitives/InlineNotice";
import { cn } from "../../../ui/internal/cn";
import { Icon } from "../../../ui/icons/Icon";
import { useLibraryFullPage } from "../../library/LibraryFrame";
import { isShapeTool, isStickyNote, type AnnotatorTool, type ShapeTool } from "./annotationModel";
import type { SaveState } from "./annotationSaveQueue";
import { AnnotatorCanvas } from "./AnnotatorCanvas";
import { AnnotatorToolbar } from "./AnnotatorToolbar";
import { MarkCardEditor, type CardMark } from "./MarkCardEditor";
import { ACTUAL_SIZE_SCALE, MAX_SCALE, MIN_SCALE, useAnnotatorViewport } from "./useAnnotatorViewport";
import { useDocumentAnnotator, type DocumentAnnotatorController } from "./useDocumentAnnotator";

interface DocumentAnnotatorProps {
  documentId: string;
  title: string;
  onClose: () => void;
  showToast: ShowToast;
  /** Starts a practice session of these cards, leaving the annotator. */
  onPracticeCards?: (cardIds: string[]) => Promise<void>;
  /** After cards are made here, so deck counts elsewhere can refresh. */
  onCardsChanged?: () => void;
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
export function DocumentAnnotator({ documentId, title, onClose, showToast, onPracticeCards, onCardsChanged }: DocumentAnnotatorProps) {
  useLibraryFullPage(true);
  const controller = useDocumentAnnotator(documentId, showToast);
  const { leave, closing } = useAnnotatorLeave(controller);
  const close = useCallback(() => leave(onClose), [leave, onClose]);

  // The cards made from this document's marks; a newer read replaces an older one.
  const [marked, setMarked] = useState<DocumentMarkedCards | null>(null);
  const markedRequest = useRef(0);
  const refreshMarked = useCallback(() => {
    const request = ++markedRequest.current;
    getDocumentMarkedCards(documentId)
      .then((next) => { if (markedRequest.current === request) setMarked(next); })
      // Without a count the study action stays hidden; making cards still works.
      .catch(() => undefined);
  }, [documentId]);
  useEffect(() => { refreshMarked(); }, [refreshMarked]);
  const cardSaved = useCallback(() => {
    refreshMarked();
    onCardsChanged?.();
  }, [onCardsChanged, refreshMarked]);
  const markedCardIds = marked?.cardIds ?? [];

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
      actions={
        <div className="tw:flex tw:items-center tw:gap-3">
          <SaveStatus state={controller.saveState} onRetry={controller.retrySave} />
          {onPracticeCards && markedCardIds.length > 0 ? (
            <Button size="small" variant="secondary" disabled={closing}
              onClick={() => void leave(() => onPracticeCards(markedCardIds))}>
              <Icon name="practice" />
              Study marked cards
            </Button>
          ) : null}
        </div>
      }
    >
      <AnnotatorBody documentId={documentId} title={title} controller={controller} showToast={showToast}
        suggestedDeckId={marked?.suggestedDeckId ?? null} onCardSaved={cardSaved} />
    </InlineWorkspace>
  );
}

/**
 * Leaving the annotator, to wherever `then` goes, saves what is pending first
 * and asks before leaving annotations that could not be saved.
 */
export function useAnnotatorLeave(controller: DocumentAnnotatorController) {
  const confirm = useConfirm();
  const [closing, setClosing] = useState(false);
  const closingRef = useRef(false);
  const { flush } = controller;

  const leave = useCallback(async (then: () => void | Promise<void>) => {
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
      await then();
    } finally {
      closingRef.current = false;
      setClosing(false);
    }
  }, [confirm, flush]);
  return { leave, closing };
}

interface AnnotatorBodyProps {
  documentId: string;
  title: string;
  controller: DocumentAnnotatorController;
  showToast: ShowToast;
  /** The deck a card made from a mark goes to first; null uses the last one chosen. */
  suggestedDeckId?: string | null;
  onCardSaved?: () => void;
  /** Opens the pages at this page and points it out. */
  focusPage?: number;
}

/** The annotator under its frame: loading and failure states, then the toolbar and pages. */
export function AnnotatorBody({
  documentId, title, controller, showToast, suggestedDeckId = null, onCardSaved = ignore, focusPage,
}: AnnotatorBodyProps) {
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
    <ReadyAnnotator documentId={documentId} title={title} pages={load.pages} controller={controller}
      suggestedDeckId={suggestedDeckId} onCardSaved={onCardSaved} showToast={showToast} focusPage={focusPage} />
  );
}

function ignore(): void {}

function ReadyAnnotator({ documentId, title, pages, controller, suggestedDeckId, onCardSaved, showToast, focusPage }: {
  documentId: string;
  title: string;
  pages: readonly AnnotatedPage[];
  controller: DocumentAnnotatorController;
  suggestedDeckId: string | null;
  onCardSaved: () => void;
  showToast: ShowToast;
  focusPage?: number;
}) {
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const viewport = useAnnotatorViewport(pages, viewportRef, contentRef);
  const cited = focusPage !== undefined && pages.some((page) => page.pageNumber === focusPage) ? focusPage : null;
  const { measured, showPage } = viewport;
  // Once the pages have their size, the cited page is scrolled to and outlined for a moment.
  const [outlined, setOutlined] = useState<number | null>(null);
  const shown = useRef(false);
  useLayoutEffect(() => {
    if (cited === null || !measured || shown.current) return;
    shown.current = true;
    showPage(cited);
    setOutlined(cited);
  }, [cited, measured, showPage]);
  useEffect(() => {
    if (outlined === null) return undefined;
    const timer = setTimeout(() => setOutlined(null), CITED_PAGE_MARK_MS);
    return () => clearTimeout(timer);
  }, [outlined]);
  const { layoutMode, inputMode } = useLayoutMode();
  const [lastShape, setLastShape] = useState<ShapeTool>("arrow");
  const [lastNote, setLastNote] = useState<"text" | "sticky">("text");
  const compact = layoutMode === "compact";
  // The mark a card is being made from; its editor sits beside the pages, or over them on a phone.
  const [cardMark, setCardMark] = useState<CardMark | null>(null);
  const [openingCard, setOpeningCard] = useState(false);

  async function makeCard() {
    const mark = controller.selected;
    if (!mark || openingCard) return;
    setOpeningCard(true);
    try {
      // The link needs the mark saved; a mark still waiting to save is saved now.
      if (!(await controller.flush())) {
        showToast("error", "This mark is not saved yet, so a card cannot be made from it. Try again once it saves.");
        return;
      }
      setCardMark({ id: mark.id, quote: markQuote(mark.shape) });
    } finally {
      setOpeningCard(false);
    }
  }

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
    <div className="tw:flex tw:min-w-0">
      <div className={cn("tw:flex tw:min-w-0 tw:flex-1 tw:flex-col", compact && cardMark && "tw:hidden")} onKeyDown={keyDown}>
        <AnnotatorToolbar
          compact={compact}
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
          onMakeCard={cardMark || openingCard ? undefined : () => void makeCard()}
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
          markedPage={outlined}
        />
        {cited !== null ? <p role="status" className="tw:sr-only">Opened at page {cited}</p> : null}
      </div>
      {cardMark ? (
        <aside aria-label="New card from your mark"
          className="tw:flex tw:w-[28rem] tw:shrink-0 tw:flex-col tw:self-stretch tw:border-0 tw:border-l tw:border-solid tw:border-border tw:bg-background tw:compact:w-full tw:compact:border-l-0">
          {/* Fills the row's height without adding to it, and scrolls on its own. */}
          <div className="tw:h-0 tw:min-h-full tw:overflow-y-auto tw:px-3 tw:compact:h-auto tw:compact:px-0">
            <MarkCardEditor
              mark={cardMark}
              suggestedDeckId={suggestedDeckId}
              showToast={showToast}
              onClose={() => setCardMark(null)}
              onSaved={(closed) => {
                onCardSaved();
                if (closed) setCardMark(null);
              }}
            />
          </div>
        </aside>
      ) : null}
    </div>
  );
}

/** The words a mark holds: a text highlight's or a note's text; strokes and shapes hold none. */
function markQuote(shape: AnnotationShape): string {
  if (shape.kind === "highlight") return (shape.text ?? "").trim();
  if (shape.kind === "text") return shape.text.trim();
  return "";
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
