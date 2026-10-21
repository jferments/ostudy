import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";

import type { ShowToast } from "../../components/Toast";
import type { SourcePassage } from "../../components/sourcePassage";
import { beginAssetDraft, cancelAssetDraft, importImage } from "../../ipc/assets";
import { getErrorMessage } from "../../ipc/common";
import type { AnnotatedPage, ImageShape, NoteLink } from "../../ipc/documentAnnotations";
import {
  addNotebookPage,
  deleteNotebookPage,
  moveNotebookPage,
  type NotebookContents,
} from "../../ipc/notebooks";
import { managedAssetUrl } from "../../platform/assets";
import { registerPlatformBackHandler } from "../../platform/backNavigation";
import { selectExternalImport } from "../../platform/files";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { InlineWorkspace } from "../../ui/overlays/Dialog";
import { OverflowMenu } from "../../ui/overlays/OverflowMenu";
import { useConfirm } from "../../ui/overlays/overlayContext";
import { Button } from "../../ui/primitives/Button";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import { Icon } from "../../ui/icons/Icon";
import { cn } from "../../ui/internal/cn";
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
import { useLibraryFullPage } from "../library/LibraryFrame";
import { ChooseDocumentDialog, DocumentBeside } from "./DocumentBeside";
import { LinkDialog, type LinkChoice } from "./LinkDialog";
import { notebookSource, rememberPictureDraft } from "./notebookSource";
import { PaperBackground } from "./paper";
import { fitPicture, naturalSize } from "./pictures";

/** Where the notebook's links lead outside it. */
export interface NotebookLinkActions {
  /** Starts Practice on a deck's cards. */
  onPracticeDeck: (deckId: string) => void | Promise<void>;
  /** Opens a document's study guide. */
  onOpenGuide: (documentId: string, title: string) => void;
  /** Opens another notebook at one of its pages. */
  onOpenNotebook: (notebookId: string, pageId: string) => void;
}

interface NotebookEditorProps extends NotebookLinkActions {
  notebookId: string;
  /** Shown until the notebook has loaded. */
  title: string;
  /** The page to open at; the first page otherwise. */
  pageId?: string;
  onClose: () => void;
  showToast: ShowToast;
}

const MAX_LINK_WIDTH = 320;

/**
 * A notebook open for writing: the PDF annotator's tools on pages of paper,
 * with pages to add, move and delete, and links to study materials. Adding,
 * moving or deleting a page saves what is pending first and opens the
 * notebook again, so undo starts afresh.
 */
export function NotebookEditor({ notebookId, pageId, ...props }: NotebookEditorProps) {
  useLibraryFullPage(true);
  const [session, setSession] = useState<{ generation: number; pageId?: string }>({ generation: 0, pageId });
  const reopen = useCallback((nextPageId?: string) => {
    setSession((current) => ({ generation: current.generation + 1, pageId: nextPageId }));
  }, []);
  return (
    <NotebookSession
      key={`${notebookId}:${session.generation}`}
      notebookId={notebookId}
      pageId={session.pageId}
      onReopen={reopen}
      {...props}
    />
  );
}

function NotebookSession({
  notebookId, title, pageId, onClose, showToast, onReopen, ...links
}: Omit<NotebookEditorProps, "pageId"> & { pageId?: string; onReopen: (pageId?: string) => void }) {
  const [contents, setContents] = useState<NotebookContents | null>(null);
  const source = useMemo(() => notebookSource(setContents), []);
  const base = useDocumentAnnotator(notebookId, showToast, source);
  const { leave, closing } = useAnnotatorLeave(base);
  const close = useCallback(() => leave(async () => {
    // The document beside saves its marks too; its unmount saves anything left.
    await besideFlush.current?.();
    await onClose();
  }), [leave, onClose]);
  const [passage, setPassage] = useState<SourcePassage | null>(null);
  // On a wide screen, a document can be open beside the notebook.
  const { layoutMode } = useLayoutMode();
  const wide = layoutMode !== "compact";
  const [beside, setBeside] = useState<SourcePassage | null>(null);
  const [choosingBeside, setChoosingBeside] = useState(false);
  const besideFlush = useRef<(() => Promise<boolean>) | null>(null);
  const shownBeside = wide ? beside : null;
  const [showPage, setShowPage] = useState<((pageNumber: number) => void) | null>(null);

  const unavailable = useMemo(() => new Set(contents?.unavailableLinkIds ?? []), [contents]);
  const follow = useCallback((annotation: Annotation) => {
    const link = linkOf(annotation);
    if (!link) return;
    if (unavailable.has(annotation.id)) {
      showToast("error", "What this link opened is no longer in your library.");
      return;
    }
    const label = annotation.shape.kind === "text" ? annotation.shape.text : "";
    const target = link.target;
    switch (target.kind) {
      case "documentPage":
        // With a document beside the notes, a page link opens there.
        if (shownBeside) setBeside({ documentId: target.documentId, title: label, pageNumber: target.pageNumber });
        else setPassage({ documentId: target.documentId, title: label, pageNumber: target.pageNumber });
        return;
      case "studyGuide":
        void leave(() => links.onOpenGuide(target.documentId, label));
        return;
      case "deck":
        void leave(() => links.onPracticeDeck(target.deckId));
        return;
      case "notebookPage": {
        const index = contents?.pages.findIndex((page) => page.id === target.pageId) ?? -1;
        if (target.notebookId === notebookId && index >= 0) showPage?.(index + 1);
        else void leave(() => links.onOpenNotebook(target.notebookId, target.pageId));
      }
    }
  }, [contents, leave, links, notebookId, shownBeside, showPage, showToast, unavailable]);

  // A link opens rather than being typed in: a tap selects it, a second tap follows it.
  const { editText: typeInto, select } = base;
  const selectedId = base.selected?.id ?? null;
  const editText = useCallback((annotation: Annotation) => {
    if (!linkOf(annotation)) typeInto(annotation);
    else if (selectedId === annotation.id) follow(annotation);
    else select(annotation.id, annotation.pageNumber);
  }, [follow, select, selectedId, typeInto]);
  const controller = useMemo(() => ({ ...base, editText }), [base, editText]);

  // Android Back closes the open note, then the selection, then the notebook.
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

  const failed = controller.saveState.state === "failed" ? controller.saveState.message : null;
  useEffect(() => {
    if (failed) showToast("error", `Notes could not be saved: ${failed}`);
  }, [failed, showToast]);

  const { load } = controller;
  return (
    <InlineWorkspace
      presentation="page"
      title={contents?.notebook.title ?? title}
      description="Notes"
      closeLabel="Back to notes"
      busy={closing}
      onClose={() => void close()}
      actions={<SaveStatus state={controller.saveState} onRetry={controller.retrySave} />}
    >
      {load.state === "loading" ? (
        <LoadingState label="Opening notebook…" />
      ) : load.state === "failed" ? (
        <div className="tw:grid tw:justify-items-start tw:gap-3 tw:p-page tw:compact:p-4">
          <InlineNotice kind="error">{load.message}</InlineNotice>
          <Button onClick={controller.retryLoad}>Try again</Button>
        </div>
      ) : contents ? (
        <div className="tw:flex tw:min-h-0 tw:min-w-0">
        {shownBeside ? (
          <DocumentBeside
            key={`${shownBeside.documentId}:${shownBeside.pageNumber}`}
            passage={shownBeside}
            showToast={showToast}
            flushRef={besideFlush}
            onClose={() => setBeside(null)}
          />
        ) : null}
        <div className="tw:flex tw:min-w-0 tw:flex-1 tw:basis-0 tw:flex-col">
        <ReadyNotebook
          notebookId={notebookId}
          contents={contents}
          pages={load.pages}
          controller={controller}
          unavailable={unavailable}
          openAtPageId={pageId}
          showToast={showToast}
          onFollow={follow}
          onReopen={onReopen}
          onShowPage={setShowPage}
          onOpenBeside={wide ? () => setChoosingBeside(true) : undefined}
          besideOpen={shownBeside !== null}
        />
        </div>
        </div>
      ) : null}
      {choosingBeside ? (
        <ChooseDocumentDialog
          onClose={() => setChoosingBeside(false)}
          onChoose={(document) => {
            setChoosingBeside(false);
            setBeside({ documentId: document.id, title: document.title, pageNumber: 1 });
          }}
        />
      ) : null}
      {passage ? <SourcePassageDialog passage={passage} onClose={() => setPassage(null)} showToast={showToast} /> : null}
    </InlineWorkspace>
  );
}

function linkOf(annotation: Annotation | null): NoteLink | null {
  return annotation?.shape.kind === "text" ? annotation.shape.link ?? null : null;
}

function ReadyNotebook({
  notebookId, contents, pages, controller, unavailable, openAtPageId, showToast, onFollow, onReopen, onShowPage,
  onOpenBeside, besideOpen,
}: {
  notebookId: string;
  contents: NotebookContents;
  pages: readonly AnnotatedPage[];
  controller: DocumentAnnotatorController;
  unavailable: ReadonlySet<string>;
  openAtPageId?: string;
  showToast: ShowToast;
  onFollow: (annotation: Annotation) => void;
  onReopen: (pageId?: string) => void;
  onShowPage: (show: (pageNumber: number) => void) => void;
  /** Chooses a document to open beside the notebook; wide screens only. */
  onOpenBeside?: () => void;
  besideOpen: boolean;
}) {
  const confirm = useConfirm();
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const viewport = useAnnotatorViewport(pages, viewportRef, contentRef);
  const { layoutMode, inputMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const [lastShape, setLastShape] = useState<ShapeTool>("arrow");
  const [lastNote, setLastNote] = useState<"text" | "sticky">("text");
  const [linkDialog, setLinkDialog] = useState<{ editing: Annotation | null } | null>(null);
  const [busy, setBusy] = useState(false);
  const [addingPicture, setAddingPicture] = useState(false);
  const textFormat = useTextFormat(controller);
  const [current, setCurrent] = useState(1);

  const paperPages = useMemo<PaperPages>(() => ({
    background: (page) => (
      <PaperBackground paper={contents.pages[page.pageNumber - 1]?.paper ?? "blank"} width={page.width} height={page.height} />
    ),
    unavailableLinks: unavailable,
  }), [contents.pages, unavailable]);

  const { measured, showPage } = viewport;
  useEffect(() => { onShowPage(() => showPage); }, [onShowPage, showPage]);
  const opened = useRef(false);
  useLayoutEffect(() => {
    if (!measured || opened.current) return;
    opened.current = true;
    const index = contents.pages.findIndex((page) => page.id === openAtPageId);
    if (index > 0) showPage(index + 1);
  }, [contents.pages, measured, openAtPageId, showPage]);

  /** The page most in view: the last one whose top is above a third of the way down. */
  const pageInView = useCallback((): number => {
    const element = viewportRef.current;
    const content = contentRef.current;
    if (!element || !content) return 1;
    const line = element.getBoundingClientRect().top + element.clientHeight / 3;
    let found = 1;
    content.querySelectorAll<HTMLElement>("[data-annotator-page]").forEach((page) => {
      if (page.getBoundingClientRect().top <= line) found = Number(page.dataset.annotatorPage);
    });
    return found;
  }, []);
  useEffect(() => {
    const element = viewportRef.current;
    if (!element) return undefined;
    const update = () => setCurrent(pageInView());
    update();
    element.addEventListener("scroll", update, { passive: true });
    return () => element.removeEventListener("scroll", update);
  }, [pageInView]);

  /** Saves what is pending, changes the pages, then opens the notebook again at `then`'s page. */
  async function changePages(change: () => Promise<string | undefined>) {
    if (busy) return;
    setBusy(true);
    try {
      if (!(await controller.flush())) {
        showToast("error", "Your latest marks are not saved yet. Try again once they save.");
        return;
      }
      onReopen(await change());
    } catch (error) {
      showToast("error", getErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  const pageCount = contents.pages.length;
  const currentPage = contents.pages[current - 1];
  const addPage = () => void changePages(async () => {
    const next = await addNotebookPage(notebookId, current + 1);
    return next.pages[current]?.id;
  });
  const movePage = (to: number) => void changePages(async () => {
    if (!currentPage) return undefined;
    await moveNotebookPage(notebookId, currentPage.id, to);
    return currentPage.id;
  });
  const deletePage = async () => {
    if (!currentPage || pageCount <= 1) return;
    const marks = controller.annotations.get(current)?.length ?? 0;
    if (marks > 0 && !(await confirm({
      title: `Delete page ${current}?`,
      description: "The page and everything on it are deleted. This cannot be undone.",
      confirmLabel: "Delete page",
      cancelLabel: "Keep page",
      destructive: true,
    }))) return;
    void changePages(async () => {
      const next = await deleteNotebookPage(notebookId, currentPage.id);
      return next.pages[Math.min(current, next.pages.length) - 1]?.id;
    });
  };

  /** Puts a new link near the top of what is in view on the current page. */
  function placeLink({ link, label }: LinkChoice) {
    const editing = linkDialog?.editing ?? null;
    setLinkDialog(null);
    if (editing && editing.shape.kind === "text") {
      const shape: TextShape = { ...editing.shape, text: label, link };
      controller.commit([{ id: editing.id, before: editing, after: { ...editing, shape } }]);
      return;
    }
    const pageNumber = pageInView();
    const page = pages[pageNumber - 1];
    if (!page) return;
    const visibleTop = visibleTopOf(pageNumber);
    const fontSize = controller.styles.text.size;
    // About as wide as the label, so the selection hugs it.
    const width = Math.round(Math.min(MAX_LINK_WIDTH, page.width - 96, label.length * fontSize * 0.6 + fontSize * 3));
    const shape: TextShape = {
      kind: "text",
      x: 48,
      y: Math.round(Math.min(Math.max(48, visibleTop + 48), page.height - fontSize * 3)),
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

  /** The top of what is in view on a page, in points. */
  function visibleTopOf(pageNumber: number): number {
    const element = viewportRef.current;
    const pageElement = contentRef.current?.querySelector<HTMLElement>(`[data-annotator-page="${pageNumber}"]`);
    if (!element || !pageElement) return 0;
    return (element.getBoundingClientRect().top - pageElement.getBoundingClientRect().top) / viewport.scale;
  }

  /**
   * Asks for an image and puts it near the top of what is in view on the
   * current page, selected. Each picture stages its image in a draft of its
   * own, which the save that stores the picture keeps.
   */
  async function addPicture() {
    if (addingPicture) return;
    setAddingPicture(true);
    let draftId: string | null = null;
    try {
      draftId = (await beginAssetDraft("import", notebookId)).id;
      const selectedFile = await selectExternalImport("image");
      if (!selectedFile) {
        void cancelAssetDraft(draftId).catch(() => undefined);
        return;
      }
      const asset = await importImage(selectedFile.id, draftId);
      rememberPictureDraft(asset.id, draftId);
      draftId = null;
      const pageNumber = pageInView();
      const page = pages[pageNumber - 1];
      if (!page) return;
      const { width, height } = fitPicture(await naturalSize(managedAssetUrl(asset.id)), page);
      const top = Math.min(Math.max(48, visibleTopOf(pageNumber) + 48), page.height - height);
      const shape: ImageShape = {
        kind: "image",
        x: Math.round((page.width - width) / 2),
        y: Math.round(Math.max(0, top)),
        width,
        height,
        assetId: asset.id,
      };
      const annotation: Annotation = { id: createAnnotationId(), pageNumber, shape, createdAt: controller.now() };
      controller.setTool("select");
      controller.commit([{ id: annotation.id, before: null, after: annotation }]);
      controller.select(annotation.id, pageNumber);
    } catch (error) {
      if (draftId) void cancelAssetDraft(draftId).catch(() => undefined);
      showToast("error", `The picture could not be added: ${getErrorMessage(error)}`);
    } finally {
      setAddingPicture(false);
    }
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
    const tool = event.shiftKey ? undefined : key === "k" ? undefined : SHORTCUTS[key];
    if (key === "k" && !event.shiftKey) {
      event.preventDefault();
      setLinkDialog({ editing: null });
      return;
    }
    if (tool) {
      event.preventDefault();
      chooseTool(tool);
    }
  }

  const selected = controller.selected;
  const selectedLink = linkOf(selected);
  return (
    <div className="tw:flex tw:min-w-0 tw:flex-col" onKeyDown={keyDown}>
      <div className="tw:flex tw:min-w-0 tw:flex-wrap tw:items-center tw:gap-2 tw:border-0 tw:border-b tw:border-solid tw:border-border tw:bg-background tw:px-3 tw:py-1.5">
        <Button size="small" variant="secondary" aria-keyshortcuts="K" onClick={() => setLinkDialog({ editing: null })}>
          <Icon name="link" />
          Add link
        </Button>
        <Button size="small" variant="secondary" loading={addingPicture} onClick={() => void addPicture()}>
          <Icon name="image" />
          Add picture
        </Button>
        {onOpenBeside ? (
          <Button size="small" variant="secondary" onClick={onOpenBeside}>
            <Icon name="document" />
            {besideOpen ? "Change document" : "Document beside"}
          </Button>
        ) : null}
        {selected && selectedLink ? (
          <>
            <Button size="small" variant="secondary" onClick={() => onFollow(selected)}>
              <Icon name={selectedLink.button ? "practice" : "arrow-right"} />
              {unavailable.has(selected.id) ? "Removed" : "Open link"}
            </Button>
            <Button size="small" variant="ghost" onClick={() => setLinkDialog({ editing: selected })}>
              <Icon name="edit" />
              Edit link
            </Button>
          </>
        ) : null}
        {onOpenBeside ? (
          // Viewing sits with the pages, apart from what is added to them.
          <Button size="small" variant="ghost" className="tw:ms-auto" onClick={onOpenBeside}>
            <Icon name="document" />
            {besideOpen ? "Change document" : "Document beside"}
          </Button>
        ) : null}
        <span className={cn("tw:text-label tw:whitespace-nowrap tw:text-muted-foreground", !onOpenBeside && "tw:ms-auto")} role="status">
          {`Page ${current} of ${pageCount}`}
        </span>
        <Button size="small" variant="secondary" aria-label="Add page" disabled={busy} onClick={addPage}>
          <Icon name="plus" />
          <span className="tw:compact:hidden">Add page</span>
          <span className="tw:hidden tw:compact:inline">Page</span>
        </Button>
        <OverflowMenu
          label={`Page ${current} actions`}
          disabled={busy}
          items={[
            { id: "earlier", label: "Move page earlier", disabled: current <= 1, onSelect: () => movePage(current - 1) },
            { id: "later", label: "Move page later", disabled: current >= pageCount, onSelect: () => movePage(current + 1) },
            { id: "delete", label: "Delete page", destructive: true, disabled: pageCount <= 1, onSelect: () => void deletePage() },
          ]}
        />
      </div>
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
        } : null}
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
        documentId={notebookId}
        title={contents.notebook.title}
        pages={pages}
        controller={controller}
        viewport={viewport}
        viewportRef={viewportRef}
        contentRef={contentRef}
        coarse={inputMode === "coarse"}
        paperPages={paperPages}
      />
      {textFormat.dialog}
      {linkDialog ? (
        <LinkDialog
          notebookId={notebookId}
          initial={linkDialog.editing && linkDialog.editing.shape.kind === "text" && linkDialog.editing.shape.link
            ? { link: linkDialog.editing.shape.link, label: linkDialog.editing.shape.text }
            : null}
          onClose={() => setLinkDialog(null)}
          onSave={placeLink}
        />
      ) : null}
    </div>
  );
}
