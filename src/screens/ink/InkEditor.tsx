import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";

import type { ShowToast } from "../../components/Toast";
import type { AnnotatedPage } from "../../ipc/documentAnnotations";
import type { InkMaterial } from "../../ipc/materialInk";
import { registerPlatformBackHandler } from "../../platform/backNavigation";
import { useLayoutMode } from "../../ui/layout/LayoutModeProvider";
import { InlineWorkspace } from "../../ui/overlays/Dialog";
import { Button } from "../../ui/primitives/Button";
import { LoadingState } from "../../ui/primitives/EmptyState";
import { InlineNotice } from "../../ui/primitives/InlineNotice";
import {
  isCollapsedSticky,
  isShapeTool,
  isStickyNote,
  type AnnotatorTool,
  type ShapeTool,
} from "../documents/annotator/annotationModel";
import { AnnotatorCanvas } from "../documents/annotator/AnnotatorCanvas";
import type { PaperPages } from "../documents/annotator/AnnotatorPage";
import { AnnotatorToolbar } from "../documents/annotator/AnnotatorToolbar";
import { SaveStatus, SHORTCUTS, useAnnotatorLeave } from "../documents/annotator/DocumentAnnotator";
import { ACTUAL_SIZE_SCALE, MAX_SCALE, MIN_SCALE, useAnnotatorViewport } from "../documents/annotator/useAnnotatorViewport";
import { useDocumentAnnotator, type DocumentAnnotatorController } from "../documents/annotator/useDocumentAnnotator";
import { inkMaterialKey, materialInkSource, type InkPage } from "./materialInkSource";

interface InkEditorProps {
  material: InkMaterial;
  /** The slides or page as they are shown, in order. */
  pages: readonly InkPage[];
  /** The page to open at; the first otherwise. */
  pageKey?: string;
  title: string;
  /** Names the material in the frame, such as "Lecture slides". */
  description: string;
  closeLabel: string;
  onClose: () => void;
  showToast: ShowToast;
}

/**
 * Designed slides or study guide pages open for writing: the PDF annotator's
 * tools, keys and undo over the pages' pictures. Marks save as they are made;
 * closing saves what is pending first.
 */
export function InkEditor({ material, pages, pageKey, title, description, closeLabel, onClose, showToast }: InkEditorProps) {
  const key = inkMaterialKey(material);
  // The pages are fixed while the editor is open, so the source is too.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const source = useMemo(() => materialInkSource(material, pages), [key]);
  const controller = useDocumentAnnotator(key, showToast, source);
  const { leave, closing } = useAnnotatorLeave(controller);
  const close = useCallback(() => leave(onClose), [leave, onClose]);

  // Android Back closes the open note, then the selection, then the editor.
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
    if (failed) showToast("error", `Your marks could not be saved: ${failed}`);
  }, [failed, showToast]);

  const { load } = controller;
  return (
    <InlineWorkspace
      presentation="page"
      title={title}
      description={description}
      closeLabel={closeLabel}
      busy={closing}
      onClose={() => void close()}
      actions={<SaveStatus state={controller.saveState} onRetry={controller.retrySave} />}
    >
      {load.state === "loading" ? (
        <LoadingState label="Opening pages…" />
      ) : load.state === "failed" ? (
        <div className="tw:grid tw:justify-items-start tw:gap-3 tw:p-page tw:compact:p-4">
          <InlineNotice kind="error">{load.message}</InlineNotice>
          <Button onClick={controller.retryLoad}>Try again</Button>
        </div>
      ) : (
        <ReadyInk materialKey={key} title={title} inkPages={pages} pages={load.pages} controller={controller}
          openAt={pageKey === undefined ? 0 : pages.findIndex((page) => page.key === pageKey)} />
      )}
    </InlineWorkspace>
  );
}

function ReadyInk({ materialKey, title, inkPages, pages, controller, openAt }: {
  materialKey: string;
  title: string;
  inkPages: readonly InkPage[];
  pages: readonly AnnotatedPage[];
  controller: DocumentAnnotatorController;
  /** The index of the page to open at. */
  openAt: number;
}) {
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const contentRef = useRef<HTMLDivElement | null>(null);
  const viewport = useAnnotatorViewport(pages, viewportRef, contentRef);
  const { layoutMode, inputMode } = useLayoutMode();
  const compact = layoutMode === "compact";
  const [lastShape, setLastShape] = useState<ShapeTool>("arrow");
  const [lastNote, setLastNote] = useState<"text" | "sticky">("text");

  const paperPages = useMemo<PaperPages>(() => ({
    background: (page) => {
      const image = inkPages[page.pageNumber - 1]?.image;
      return image ? (
        <img src={image} alt="" draggable={false}
          className="tw:pointer-events-none tw:absolute tw:inset-0 tw:size-full tw:select-none" />
      ) : null;
    },
    unavailableLinks: new Set(),
  }), [inkPages]);

  const { measured, showPage } = viewport;
  const opened = useRef(false);
  useLayoutEffect(() => {
    if (!measured || opened.current) return;
    opened.current = true;
    if (openAt > 0) showPage(openAt + 1);
  }, [measured, openAt, showPage]);

  function chooseTool(tool: AnnotatorTool) {
    if (isShapeTool(tool)) setLastShape(tool);
    if (tool === "text" || tool === "sticky") setLastNote(tool);
    controller.setTool(tool);
  }

  function keyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.defaultPrevented) return;
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
        compact={compact}
        disabled={false}
        tool={controller.tool}
        lastShape={lastShape}
        lastNote={lastNote}
        onTool={chooseTool}
        style={controller.activeStyle}
        onStyle={controller.chooseStyle}
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
          if (selected) controller.editText(selected);
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
        documentId={materialKey}
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
  );
}
