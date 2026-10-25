import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent, type MutableRefObject } from "react";

import type { ShowToast } from "../../components/Toast";
import type { SourcePassage } from "../../components/sourcePassage";
import { latestResult, useRequestLane } from "../../controllers/requestLane";
import { getErrorMessage } from "../../ipc/common";
import type { AnnotatedPage, NoteLink } from "../../ipc/documentAnnotations";
import {
  getDocumentStudyGuideCanvas,
  type StudyGuideCanvas,
  type StudyGuideCanvasEntry,
  type StudyGuidePdfTheme,
} from "../../ipc/studyGuides";
import { registerPlatformBackHandler } from "../../platform/backNavigation";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { Sheet } from "../../ui/overlays/Dialog";
import { OverflowMenu, type OverflowMenuItem } from "../../ui/overlays/OverflowMenu";
import { Button } from "../../ui/primitives/Button";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { ItemTrigger } from "../../ui/primitives/Item";
import {
  createAnnotationId,
  isCollapsedSticky,
  isShapeTool,
  isStickyNote,
  type Annotation,
  type AnnotatorTool,
  type ShapeTool,
  type TextShape,
} from "../documents/annotator/annotationModel";
import { AnnotatorCanvas } from "../documents/annotator/AnnotatorCanvas";
import type { PaperPages } from "../documents/annotator/AnnotatorPage";
import { AnnotatorToolbar } from "../documents/annotator/AnnotatorToolbar";
import { SaveStatus, SHORTCUTS, useAnnotatorLeave } from "../documents/annotator/DocumentAnnotator";
import { SourcePassageDialog } from "../documents/annotator/SourcePassageDialog";
import { ACTUAL_SIZE_SCALE, MAX_SCALE, MIN_SCALE, useAnnotatorViewport } from "../documents/annotator/useAnnotatorViewport";
import { useTextFormat } from "../documents/annotator/textFormat";
import { useDocumentAnnotator, type DocumentAnnotatorController } from "../documents/annotator/useDocumentAnnotator";
import { inkMaterialKey, materialInkSource, type InkPage } from "../ink/materialInkSource";
import { LinkDialog, type LinkChoice } from "../notes/LinkDialog";
import { GuideSourcesSheet } from "./GuideSources";

/** Where links on the guide's pages lead outside it. Each is optional: a link with nowhere to go says so. */
export interface GuideCanvasLinkActions {
  onPracticeDeck?: (deckId: string) => void | Promise<void>;
  onOpenGuide?: (documentId: string, title: string) => void;
  onOpenNotebook?: (notebookId: string, pageId: string) => void;
}

/** Saves the marks still pending, then does `then`; asks first when they could not be saved. */
export type GuideCanvasLeave = (then: () => void | Promise<void>) => Promise<void>;

interface GuideCanvasProps extends GuideCanvasLinkActions {
  documentId: string;
  /** Changes whenever the laid-out text does: a new guide revision or a section edit. */
  contentKey: string;
  title: string;
  theme: StudyGuidePdfTheme;
  showToast: ShowToast;
  /** Sections the learner has read, checked in the contents. */
  readTopicIds: ReadonlySet<string>;
  /** The section to open at; the cover otherwise. */
  openAtTopicId: string | null;
  /** The contents as a sheet, for screens without room for them beside the pages. */
  contentsOpen: boolean;
  onContentsClose: () => void;
  /** A section's own actions, in its contents row's menu. */
  sectionMenu: (topicId: string) => OverflowMenuItem[];
  onPractice: (topicId: string) => void;
  /** The section in view changed. */
  onCurrentSection: (topicId: string) => void;
  /** Holds how to leave the canvas once its marks are saved, while it is open. */
  leaveRef: MutableRefObject<GuideCanvasLeave | null>;
  /** The page could not be laid out; the dialog offers the plain text instead. */
  onReadAsText: () => void;
}

type Layout =
  | { state: "loading" }
  | { state: "failed"; message: string }
  | { state: "ready"; canvas: StudyGuideCanvas; pages: InkPage[] };

/**
 * The guide as the PDF export sets it, every page on one annotator canvas: the
 * learner writes, draws and adds links on any page, practices a section from
 * the button beside its heading, and moves around from the contents beside the
 * pages. Marks are saved under each page's key, so a section edit or a new
 * revision moves only the marks of the chapter it changed.
 */
export function GuideCanvas(props: GuideCanvasProps) {
  const { documentId, theme, contentKey } = props;
  const lane = useRequestLane();
  const [attempt, setAttempt] = useState(0);
  const [layout, setLayout] = useState<{ request: string; value: Layout } | null>(null);
  const request = `${documentId}:${theme}:${contentKey}:${attempt}`;

  useEffect(() => {
    let live = true;
    let made: string[] = [];
    void latestResult(lane, () => getDocumentStudyGuideCanvas(documentId, theme)).then(
      (canvas) => {
        if (canvas === undefined || !live) return;
        made = canvas.pages.map((page) => URL.createObjectURL(new Blob([page.svg], { type: "image/svg+xml" })));
        const pages = canvas.pages.map((page, index) => ({
          key: page.key, width: page.width, height: page.height, image: made[index] ?? "",
        }));
        setLayout({ request, value: { state: "ready", canvas, pages } });
      },
      (error: unknown) => {
        if (live) setLayout({ request, value: { state: "failed", message: getErrorMessage(error) } });
      },
    );
    return () => {
      live = false;
      made.forEach((url) => URL.revokeObjectURL(url));
    };
  }, [lane, documentId, theme, request]);

  const value: Layout = layout?.request === request ? layout.value : { state: "loading" };
  if (value.state === "loading") return <LoadingState label="Laying out the study guide…" />;
  if (value.state === "failed") {
    return (
      <div className="tw:grid tw:justify-items-start tw:gap-3 tw:p-page tw:compact:p-4">
        <InlineNotice kind="error">{`The study guide's pages could not be laid out: ${value.message}`}</InlineNotice>
        <div className="tw:flex tw:flex-wrap tw:gap-2">
          <Button onClick={() => setAttempt((count) => count + 1)}>Try again</Button>
          <Button variant="secondary" onClick={props.onReadAsText}>Read as plain text</Button>
        </div>
      </div>
    );
  }
  // A new layout is a new set of pages, so the annotator starts afresh on it.
  return <GuideCanvasSession key={request} {...props} canvas={value.canvas} inkPages={value.pages} />;
}

function GuideCanvasSession({
  documentId, title, showToast, readTopicIds, openAtTopicId, contentsOpen, onContentsClose, sectionMenu, onPractice,
  onCurrentSection, leaveRef, canvas, inkPages, ...links
}: GuideCanvasProps & { canvas: StudyGuideCanvas; inkPages: InkPage[] }) {
  const material = useMemo(() => ({ kind: "studyGuide" as const, documentId }), [documentId]);
  const key = inkMaterialKey(material);
  const [unavailable, setUnavailable] = useState<ReadonlySet<string>>(() => new Set());
  // The pages are fixed for the session, so the source is too.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const source = useMemo(() => materialInkSource(material, inkPages, setUnavailable), [key]);
  const base = useDocumentAnnotator(key, showToast, source);
  const { leave, closing } = useAnnotatorLeave(base);
  useEffect(() => {
    leaveRef.current = leave;
    return () => { if (leaveRef.current === leave) leaveRef.current = null; };
  }, [leave, leaveRef]);
  const [passage, setPassage] = useState<SourcePassage | null>(null);
  const [cited, setCited] = useState<{ topicId: string; number: number } | null>(null);
  const [showPage, setShowPage] = useState<((pageNumber: number, top?: number) => void) | null>(null);

  const follow = useCallback((annotation: Annotation) => {
    const link = linkOf(annotation);
    if (!link) return;
    if (unavailable.has(annotation.id)) {
      showToast("error", "What this link opened is no longer in your library.");
      return;
    }
    const label = annotation.shape.kind === "text" ? annotation.shape.text : "";
    const target = link.target;
    const elsewhere = () => showToast("info", "That link opens from Notes or Library, not from here.");
    switch (target.kind) {
      case "documentPage":
        setPassage({ documentId: target.documentId, title: label, pageNumber: target.pageNumber });
        return;
      case "studyGuide":
        if (target.documentId === documentId) showPage?.(1);
        else if (links.onOpenGuide) { const open = links.onOpenGuide; void leave(() => open(target.documentId, label)); }
        else elsewhere();
        return;
      case "deck":
        if (links.onPracticeDeck) { const practice = links.onPracticeDeck; void leave(() => practice(target.deckId)); }
        else elsewhere();
        return;
      case "notebookPage":
        if (links.onOpenNotebook) { const open = links.onOpenNotebook; void leave(() => open(target.notebookId, target.pageId)); }
        else elsewhere();
    }
  }, [documentId, leave, links, showPage, showToast, unavailable]);

  // A link opens rather than being typed in: a tap selects it, a second tap follows it.
  const { editText: typeInto, select } = base;
  const selectedId = base.selected?.id ?? null;
  const editText = useCallback((annotation: Annotation) => {
    if (!linkOf(annotation)) typeInto(annotation);
    else if (selectedId === annotation.id) follow(annotation);
    else select(annotation.id, annotation.pageNumber);
  }, [follow, select, selectedId, typeInto]);
  const controller = useMemo(() => ({ ...base, editText }), [base, editText]);

  // Android Back closes the open note, then the selection, then the contents sheet; the
  // dialog's own handler takes it from there.
  const back = useRef<() => boolean>(() => false);
  back.current = () => {
    if (controller.editing) controller.finishEditing();
    else if (controller.selected) controller.select(null);
    else return false;
    return true;
  };
  useEffect(() => registerPlatformBackHandler(() => back.current()), []);

  const failed = controller.saveState.state === "failed" ? controller.saveState.message : null;
  useEffect(() => {
    if (failed) showToast("error", `Your marks could not be saved: ${failed}`);
  }, [failed, showToast]);

  const { load } = controller;
  return (
    <>
      {load.state === "loading" ? (
        <LoadingState label="Opening the study guide…" />
      ) : load.state === "failed" ? (
        <div className="tw:grid tw:justify-items-start tw:gap-3 tw:p-page tw:compact:p-4">
          <InlineNotice kind="error">{load.message}</InlineNotice>
          <Button onClick={controller.retryLoad}>Try again</Button>
        </div>
      ) : (
        <ReadyGuide documentId={documentId} title={title} canvas={canvas} inkPages={inkPages} pages={load.pages}
          controller={controller} unavailable={unavailable} busy={closing} readTopicIds={readTopicIds}
          openAtTopicId={openAtTopicId} contentsOpen={contentsOpen} onContentsClose={onContentsClose}
          sectionMenu={sectionMenu} onPractice={onPractice} onCurrentSection={onCurrentSection}
          onFollow={follow} onShowPage={setShowPage} onCite={(topicId, number) => setCited({ topicId, number })} />
      )}
      {cited ? (
        <GuideSourcesSheet sources={canvas.sources.filter((source) => source.topicId === cited.topicId)}
          current={cited.number} sectionTitle={sectionTitle(canvas, cited.topicId)}
          documentId={documentId} documentTitle={title} onOpen={setPassage} onClose={() => setCited(null)} />
      ) : null}
      {passage ? <SourcePassageDialog passage={passage} onClose={() => setPassage(null)} showToast={showToast} /> : null}
    </>
  );
}

/** "1.2 Afterload", as the contents list it. */
function sectionTitle(canvas: StudyGuideCanvas, topicId: string): string {
  const entry = canvas.contents.find((item) => item.kind === "section" && item.key === topicId);
  return entry ? `${entry.number} ${entry.title}`.trim() : "This section";
}

function linkOf(annotation: Annotation | null): NoteLink | null {
  return annotation?.shape.kind === "text" ? annotation.shape.link ?? null : null;
}

const MAX_LINK_WIDTH = 320;

function ReadyGuide({
  documentId, title, canvas, inkPages, pages, controller, unavailable, busy, readTopicIds, openAtTopicId, contentsOpen,
  onContentsClose, sectionMenu, onPractice, onCurrentSection, onFollow, onShowPage, onCite,
}: {
  documentId: string;
  title: string;
  canvas: StudyGuideCanvas;
  inkPages: readonly InkPage[];
  pages: readonly AnnotatedPage[];
  controller: DocumentAnnotatorController;
  unavailable: ReadonlySet<string>;
  busy: boolean;
  readTopicIds: ReadonlySet<string>;
  openAtTopicId: string | null;
  contentsOpen: boolean;
  onContentsClose: () => void;
  sectionMenu: (topicId: string) => OverflowMenuItem[];
  onPractice: (topicId: string) => void;
  onCurrentSection: (topicId: string) => void;
  onFollow: (annotation: Annotation) => void;
  onShowPage: (show: (pageNumber: number, top?: number) => void) => void;
  /** A key point's source number was chosen. */
  onCite: (topicId: string, number: number) => void;
}) {
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const viewport = useAnnotatorViewport(pages, viewportRef, contentRef);
  const { layoutMode, inputMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const wide = layoutMode === "wide";
  const [lastShape, setLastShape] = useState<ShapeTool>("arrow");
  const [lastNote, setLastNote] = useState<"text" | "sticky">("text");
  const [linkDialog, setLinkDialog] = useState<{ editing: Annotation | null } | null>(null);
  const textFormat = useTextFormat(controller);
  const [current, setCurrent] = useState(1);
  const [currentKey, setCurrentKey] = useState<string | null>(null);

  const titles = useMemo(() => new Map(canvas.contents
    .filter((entry) => entry.kind === "section")
    .map((entry) => [entry.key, `${entry.number} ${entry.title}`.trim()])), [canvas.contents]);
  const paperPages = useMemo<PaperPages>(() => ({
    background: (page) => {
      const image = inkPages[page.pageNumber - 1]?.image;
      return image ? (
        <img src={image} alt="" draggable={false}
          className="tw:pointer-events-none tw:absolute tw:inset-0 tw:size-full tw:select-none" />
      ) : null;
    },
    foreground: (page, scale) => [...canvas.practice
      .filter((button) => button.pageIndex === page.pageNumber - 1)
      .map((button) => (
        // Fills the room the heading keeps for it; on a touch screen it reaches past that room.
        <ItemTrigger key={button.topicId} data-guide-practice={button.topicId}
          aria-label={`Practice flashcards for ${titles.get(button.topicId) ?? "this section"}`}
          className="tw:absolute tw:flex tw:items-center tw:justify-center tw:gap-[0.3em] tw:rounded-full tw:border-0 tw:bg-primary tw:font-sans tw:font-semibold tw:text-primary-foreground tw:shadow-sm tw:outline-none tw:hover:bg-primary/90 tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:coarse:min-h-0 tw:coarse:after:absolute tw:coarse:after:-inset-3 tw:coarse:after:content-['']"
          style={{
            left: button.x * scale,
            top: button.y * scale,
            width: button.width * scale,
            height: button.height * scale,
            fontSize: Math.max(8, 8.5 * scale),
          }}
          onPointerDown={(event) => event.stopPropagation()}
          onClick={() => onPractice(button.topicId)}>
          <Icon name="practice" size={Math.max(10, 9 * scale)} />
          Practice
        </ItemTrigger>
      )), ...canvas.citations
      .filter((mark) => mark.pageIndex === page.pageNumber - 1)
      .map((mark) => (
        // Lies over the printed number; on a touch screen it reaches well past it.
        <ItemTrigger key={`cite:${mark.topicId}:${mark.number}:${mark.x}:${mark.y}`}
          data-guide-citation={`${mark.topicId}:${mark.number}`}
          aria-label={`Source ${mark.number} for ${titles.get(mark.topicId) ?? "this section"}`}
          className="tw:absolute tw:rounded-sm tw:border-0 tw:bg-transparent tw:p-0 tw:outline-none tw:hover:bg-primary/15 tw:focus-visible:ring-2 tw:focus-visible:ring-ring/60 tw:coarse:min-h-0 tw:coarse:after:absolute tw:coarse:after:-inset-2.5 tw:coarse:after:content-['']"
          style={{
            left: (mark.x - 1) * scale,
            top: (mark.y - 1) * scale,
            width: (mark.width + 2) * scale,
            height: (mark.height + 2) * scale,
          }}
          onPointerDown={(event) => event.stopPropagation()}
          onClick={() => onCite(mark.topicId, mark.number)} />
      ))],
    unavailableLinks: unavailable,
  }), [canvas.citations, canvas.practice, inkPages, onCite, onPractice, titles, unavailable]);

  const { measured, showPage } = viewport;
  useEffect(() => { onShowPage(() => showPage); }, [onShowPage, showPage]);
  const opened = useRef(false);
  useLayoutEffect(() => {
    if (!measured || opened.current) return;
    opened.current = true;
    const entry = canvas.contents.find((item) => item.kind === "section" && item.key === openAtTopicId);
    if (entry) showPage(entry.pageIndex + 1, Math.max(0, entry.y - 12));
  }, [canvas.contents, measured, openAtTopicId, showPage]);

  /** Where an entry starts in the scrolled content, in CSS pixels. */
  const entryTop = useCallback((entry: StudyGuideCanvasEntry): number | null => {
    const page = contentRef.current?.querySelector<HTMLElement>(`[data-annotator-page="${entry.pageIndex + 1}"]`);
    return page ? page.offsetTop + entry.y * viewport.scale : null;
  }, [viewport.scale]);

  /** The page most in view, and the last entry starting above a third of the way down. */
  const updateCurrent = useCallback(() => {
    const element = viewportRef.current;
    const content = contentRef.current;
    if (!element || !content) return;
    const line = element.getBoundingClientRect().top + element.clientHeight / 3;
    let found = 1;
    content.querySelectorAll<HTMLElement>("[data-annotator-page]").forEach((page) => {
      if (page.getBoundingClientRect().top <= line) found = Number(page.dataset.annotatorPage);
    });
    setCurrent(found);
    const reached = element.scrollTop + element.clientHeight / 3;
    let entryKey: string | null = null;
    for (const entry of canvas.contents) {
      const top = entryTop(entry);
      if (top === null || top > reached) break;
      entryKey = `${entry.kind}:${entry.key}`;
    }
    setCurrentKey(entryKey);
  }, [canvas.contents, entryTop]);
  useEffect(() => {
    const element = viewportRef.current;
    if (!element) return undefined;
    updateCurrent();
    element.addEventListener("scroll", updateCurrent, { passive: true });
    return () => element.removeEventListener("scroll", updateCurrent);
  }, [updateCurrent, measured]);
  const currentTopicId = currentKey?.startsWith("section:") ? currentKey.slice("section:".length) : null;
  const reportSection = useRef(onCurrentSection);
  reportSection.current = onCurrentSection;
  useEffect(() => {
    if (currentTopicId) reportSection.current(currentTopicId);
  }, [currentTopicId]);

  function go(entry: StudyGuideCanvasEntry) {
    onContentsClose();
    showPage(entry.pageIndex + 1, Math.max(0, entry.y - 12));
  }

  /** The top of what is in view on a page, in points. */
  function visibleTopOf(pageNumber: number): number {
    const element = viewportRef.current;
    const pageElement = contentRef.current?.querySelector<HTMLElement>(`[data-annotator-page="${pageNumber}"]`);
    if (!element || !pageElement) return 0;
    return (element.getBoundingClientRect().top - pageElement.getBoundingClientRect().top) / viewport.scale;
  }

  /** Puts a new link near the top of what is in view on the current page, or changes the one being edited. */
  function placeLink({ link, label }: LinkChoice) {
    const editing = linkDialog?.editing ?? null;
    setLinkDialog(null);
    if (editing && editing.shape.kind === "text") {
      const shape: TextShape = { ...editing.shape, text: label, link };
      controller.commit([{ id: editing.id, before: editing, after: { ...editing, shape } }]);
      return;
    }
    const pageNumber = current;
    const page = pages[pageNumber - 1];
    if (!page) return;
    const fontSize = controller.styles.text.size;
    // About as wide as the label, so the selection hugs it.
    const width = Math.round(Math.min(MAX_LINK_WIDTH, page.width - 96, label.length * fontSize * 0.6 + fontSize * 3));
    const shape: TextShape = {
      kind: "text",
      x: 48,
      y: Math.round(Math.min(Math.max(48, visibleTopOf(pageNumber) + 48), page.height - fontSize * 3)),
      width,
      text: label,
      color: link.button ? "#2563eb" : controller.styles.text.color,
      fontSize,
      link,
    };
    const annotation: Annotation = { id: createAnnotationId(), pageNumber, shape, createdAt: controller.now() };
    controller.setTool("select");
    controller.commit([{ id: annotation.id, before: null, after: annotation }]);
    controller.select(annotation.id, pageNumber);
  }

  function chooseTool(tool: AnnotatorTool) {
    if (isShapeTool(tool)) setLastShape(tool);
    if (tool === "text" || tool === "sticky") setLastNote(tool);
    controller.setTool(tool);
  }

  function keyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.defaultPrevented || linkDialog) return;
    const target = event.target as HTMLElement;
    if (target.closest("textarea, input, select, [contenteditable='true'], [role='menu']")) return;
    if (controller.editing) return;
    const key = event.key.toLowerCase();
    if ((event.ctrlKey || event.metaKey) && !event.altKey) {
      const command = key === "z" ? (event.shiftKey ? controller.redo : controller.undo)
        : key === "y" ? controller.redo
          : key === "g" && controller.group.length > 0 ? () => controller.setGrouped(!event.shiftKey)
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
      if (controller.selected) {
        event.preventDefault();
        controller.select(null);
      } else if (controller.group.length > 0) {
        event.preventDefault();
        controller.selectGroup(0, []);
      }
      return;
    }
    if ((event.key === "Delete" || event.key === "Backspace") && controller.selected) {
      event.preventDefault();
      controller.deleteSelected();
      return;
    }
    if ((event.key === "Delete" || event.key === "Backspace") && controller.group.length > 0) {
      event.preventDefault();
      controller.deleteGroup();
      return;
    }
    if (key === "k" && !event.shiftKey) {
      event.preventDefault();
      setLinkDialog({ editing: null });
      return;
    }
    const tool = event.shiftKey ? undefined : SHORTCUTS[key];
    if (tool) {
      event.preventDefault();
      chooseTool(tool);
    }
  }

  const selected = controller.selected;
  const selectedLink = linkOf(selected);
  const contents = (
    <GuideCanvasContents canvas={canvas} currentKey={currentKey} readTopicIds={readTopicIds}
      sectionMenu={sectionMenu} onGo={go} showLabel={wide} />
  );
  return (
    <div className="tw:flex tw:min-h-0 tw:min-w-0">
      {wide ? (
        // As tall as the pages beside it, scrolling on its own.
        <aside className="tw:relative tw:w-[clamp(13rem,17vw,18rem)] tw:shrink-0 tw:border-0 tw:border-e tw:border-solid tw:border-border tw:bg-foreground/3">
          <div className="tw:absolute tw:inset-0 tw:overflow-y-auto tw:overscroll-contain tw:p-2">{contents}</div>
        </aside>
      ) : null}
      <div className="tw:flex tw:min-w-0 tw:flex-1 tw:basis-0 tw:flex-col" onKeyDown={keyDown}>
        <AnnotatorToolbar
          compact={compact}
          disabled={busy}
          tool={controller.tool}
          lastShape={lastShape}
          lastNote={lastNote}
          onTool={chooseTool}
          style={controller.activeStyle}
          onStyle={controller.chooseStyle}
          textFormat={textFormat.format}
          selection={selected ? {
            isText: selected.shape.kind === "text",
            isSticky: isStickyNote(selected.shape),
            isCollapsed: isCollapsedSticky(selected.shape),
            isLink: selectedLink !== null,
            linkRemoved: unavailable.has(selected.id),
          } : null}
          onOpenLink={selected && selectedLink ? () => onFollow(selected) : undefined}
          onAddLink={busy ? undefined : () => setLinkDialog({ editing: null })}
          status={<>
            <span role="status" className="tw:tabular-nums">{`Page ${current} of ${pages.length}`}</span>
            <SaveStatus state={controller.saveState} onRetry={controller.retrySave} />
          </>}
          onToggleCollapsed={() => {
            const page = selected ? pages[selected.pageNumber - 1] : undefined;
            if (page) controller.toggleStickyCollapsed(page);
          }}
          onDeleteSelection={controller.deleteSelected}
          onEditSelection={() => {
            if (!selected) return;
            if (selectedLink) setLinkDialog({ editing: selected });
            else controller.editText(selected);
          }}
          canUndo={controller.canUndo}
          canRedo={controller.canRedo}
          onUndo={controller.undo}
          onRedo={controller.redo}
          penSeen={controller.penSeen}
          penOnly={controller.penOnly}
          onTogglePenOnly={controller.togglePenOnly}
          groupSize={controller.group.length}
          onDeleteGroup={controller.deleteGroup}
          isOneGroup={controller.isOneGroup}
          onGroup={controller.setGrouped}
          eraseMode={controller.eraseMode}
          onEraseMode={controller.setEraseMode}
          zoomPercent={Math.round(viewport.scale / ACTUAL_SIZE_SCALE * 100)}
          fit={viewport.fit}
          canZoomIn={viewport.scale < MAX_SCALE}
          canZoomOut={viewport.scale > MIN_SCALE}
          onZoomIn={viewport.zoomIn}
          onZoomOut={viewport.zoomOut}
          onFit={viewport.fitToWidth}
        />
        <AnnotatorCanvas
          documentId={inkMaterialKey({ kind: "studyGuide", documentId })}
          title={title}
          pages={pages}
          controller={controller}
          viewport={viewport}
          viewportRef={viewportRef}
          contentRef={contentRef}
          coarse={inputMode === "coarse"}
          paperPages={paperPages}
        />
      </div>
      {textFormat.dialog}
      {linkDialog ? (
        <LinkDialog
          initial={linkDialog.editing && linkDialog.editing.shape.kind === "text" && linkDialog.editing.shape.link
            ? { link: linkDialog.editing.shape.link, label: linkDialog.editing.shape.text }
            : null}
          onClose={() => setLinkDialog(null)}
          onSave={placeLink}
        />
      ) : null}
      {contentsOpen && !wide ? (
        <Sheet title="Contents" width="small" onClose={onContentsClose}>{contents}</Sheet>
      ) : null}
    </div>
  );
}

const rowBase = "tw:flex tw:min-h-10 tw:min-w-0 tw:flex-1 tw:items-center tw:gap-2.5 tw:rounded-md tw:border-0 tw:bg-transparent tw:px-2 tw:py-1.5 tw:text-start tw:font-sans tw:outline-none tw:transition-colors tw:duration-(--ostudy-motion-duration-fast) tw:focus-visible:ring-3 tw:focus-visible:ring-ring/50 tw:reduce-motion:transition-none tw:coarse:min-h-touch";
const otherRow = "tw:text-(--ostudy-color-text-secondary) tw:hover:bg-muted tw:hover:text-foreground";
const currentRow = "tw:bg-accent tw:font-semibold tw:text-foreground";
const marker = "tw:grid tw:min-w-6 tw:h-6 tw:shrink-0 tw:place-items-center tw:rounded-full tw:border tw:border-solid tw:px-1 tw:text-caption tw:tabular-nums";

/**
 * The guide's contents in page order: the cover, each chapter with its
 * sections, the cram sheet and the self-test. Choosing one scrolls the pages
 * to it; a section's menu holds its own actions.
 */
export function GuideCanvasContents({ canvas, currentKey, readTopicIds, sectionMenu, onGo, showLabel }: {
  canvas: StudyGuideCanvas;
  /** `kind:key` of the entry in view. */
  currentKey: string | null;
  readTopicIds: ReadonlySet<string>;
  sectionMenu: (topicId: string) => OverflowMenuItem[];
  onGo: (entry: StudyGuideCanvasEntry) => void;
  showLabel: boolean;
}) {
  const cover: StudyGuideCanvasEntry = { kind: "chapter", key: "", title: "Cover", number: "", pageIndex: 0, y: 0 };
  return (
    <nav aria-label="Study guide contents" className="tw:grid tw:min-w-0 tw:gap-1 tw:text-small">
      {showLabel ? (
        <p className="tw:m-0 tw:mb-1 tw:px-2 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide tw:text-muted-foreground">Contents</p>
      ) : null}
      <ol className="tw:m-0 tw:grid tw:list-none tw:gap-0.5 tw:p-0">
        <li className="tw:flex tw:min-w-0">
          <ItemTrigger className={cn(rowBase, currentKey === null ? currentRow : otherRow)}
            aria-current={currentKey === null ? "location" : undefined} onClick={() => onGo(cover)}>
            <span aria-hidden="true" className={cn(marker, "tw:border-border-strong tw:text-muted-foreground")}><Icon name="book" size={14} /></span>
            <span className="tw:min-w-0 tw:flex-1 tw:break-words">Cover and contents</span>
          </ItemTrigger>
        </li>
        {canvas.contents.map((entry) => {
          const entryKey = `${entry.kind}:${entry.key}`;
          const current = entryKey === currentKey;
          if (entry.kind === "chapter") {
            return (
              <li key={entryKey} className="tw:mt-3 tw:flex tw:min-w-0">
                <ItemTrigger className={cn(rowBase, "tw:min-h-9 tw:text-caption tw:font-semibold tw:uppercase tw:tracking-wide",
                  current ? currentRow : "tw:text-muted-foreground tw:hover:bg-muted tw:hover:text-foreground")}
                  aria-current={current ? "location" : undefined} onClick={() => onGo(entry)}>
                  <span className="tw:min-w-0 tw:flex-1 tw:break-words">{`${entry.number}. ${entry.title}`}</span>
                </ItemTrigger>
              </li>
            );
          }
          if (entry.kind !== "section") {
            return (
              <li key={entryKey} className="tw:mt-3 tw:flex tw:min-w-0">
                <ItemTrigger className={cn(rowBase, current ? currentRow : otherRow)}
                  aria-current={current ? "location" : undefined} onClick={() => onGo(entry)}>
                  <span aria-hidden="true" className={cn(marker, "tw:border-border-strong tw:text-muted-foreground")}>
                    <Icon name={entry.kind === "cram" ? "lightbulb" : "list-checks"} size={14} />
                  </span>
                  <span className="tw:min-w-0 tw:flex-1 tw:break-words">{entry.title}</span>
                </ItemTrigger>
              </li>
            );
          }
          const read = readTopicIds.has(entry.key);
          return (
            <li key={entryKey} className="tw:flex tw:min-w-0 tw:items-center tw:gap-1">
              <ItemTrigger className={cn(rowBase, current ? currentRow : otherRow)}
                aria-current={current ? "location" : undefined} onClick={() => onGo(entry)}>
                <span aria-hidden="true" className={cn(marker, current
                  ? "tw:border-transparent tw:bg-primary tw:font-semibold tw:text-primary-foreground"
                  : read ? "tw:border-success/30 tw:bg-success-soft tw:text-success"
                  : "tw:border-border-strong tw:text-muted-foreground")}>
                  {read && !current ? <Icon name="check" size={14} /> : entry.number}
                </span>
                <span className="tw:min-w-0 tw:flex-1 tw:break-words">{entry.title}</span>
                {read ? <span className="tw:sr-only">, read</span> : null}
              </ItemTrigger>
              <OverflowMenu label={`${entry.number} ${entry.title} actions`} triggerVariant="ghost" items={sectionMenu(entry.key)}
                className="tw:size-8 tw:shrink-0 tw:coarse:size-11" />
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
