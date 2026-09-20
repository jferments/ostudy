import { useLayoutEffect, useRef, useState, type FocusEvent } from "react";

import { useOverlayContext } from "../../ui/overlays/overlayContext";

interface ReturnPoint {
  element: HTMLElement;
  range: Range | null;
  selection: [number, number, "forward" | "backward" | "none"] | null;
}

/** Transient workspace navigation only. The editor stays mounted, so returning
 * never reconstructs a draft or writes a card. Scroll belongs to its actual
 * ancestors rather than an assumption that window is the scrolling element. */
export function useEditorPreview() {
  const [isOpen, setOpen] = useState(false);
  const editorRef = useRef<HTMLDivElement | null>(null);
  const returnPoint = useRef<ReturnPoint | null>(null);
  const scrollPositions = useRef<Array<[Element, number, number]>>([]);
  const restorePending = useRef(false);
  const { topId } = useOverlayContext();

  function rememberField(event: FocusEvent<HTMLDivElement>): void {
    const element = event.target;
    if (!(element instanceof HTMLElement)
      || !element.matches("input, textarea, select, [contenteditable='true']")) return;
    const selection = window.getSelection();
    const range = selection?.rangeCount ? selection.getRangeAt(0) : null;
    const textField = element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement;
    returnPoint.current = {
      element,
      range: range && element.contains(range.commonAncestorContainer) ? range.cloneRange() : null,
      selection: textField && element.selectionStart !== null && element.selectionEnd !== null
        ? [element.selectionStart, element.selectionEnd, element.selectionDirection ?? "none"]
        : null,
    };
  }

  function open(): void {
    if (isOpen) return;
    if (!returnPoint.current?.element.isConnected && document.activeElement instanceof HTMLElement) {
      returnPoint.current = { element: document.activeElement, range: null, selection: null };
    }
    const positions: Array<[Element, number, number]> = [];
    for (let element: Element | null = editorRef.current; element; element = element.parentElement) {
      positions.push([element, element.scrollLeft, element.scrollTop]);
    }
    if (document.scrollingElement && !positions.some(([element]) => element === document.scrollingElement)) {
      const element = document.scrollingElement;
      positions.push([element, element.scrollLeft, element.scrollTop]);
    }
    scrollPositions.current = positions;
    editorRef.current?.querySelectorAll<HTMLMediaElement>("audio, video").forEach((media) => {
      if (!media.paused) media.pause();
    });
    setOpen(true);
  }

  function close(): void {
    if (!isOpen) return;
    restorePending.current = true;
    setOpen(false);
  }

  useLayoutEffect(() => {
    if (isOpen || !restorePending.current) return;
    restorePending.current = false;
    const point = returnPoint.current;
    if (point?.element.isConnected) {
      point.element.focus({ preventScroll: true });
      if (point.selection && (point.element instanceof HTMLInputElement || point.element instanceof HTMLTextAreaElement)) {
        point.element.setSelectionRange(...point.selection);
      } else if (point.range?.commonAncestorContainer.isConnected) {
        const selection = window.getSelection();
        selection?.removeAllRanges();
        selection?.addRange(point.range);
      }
    }
    for (const [element, left, top] of scrollPositions.current) {
      if (element.isConnected) { element.scrollLeft = left; element.scrollTop = top; }
    }
    scrollPositions.current = [];
  }, [isOpen]);

  // Bubble phase lets media, menus and the existing overlay stack consume Escape
  // first. Native Back is dispatched by the editor's existing registered owner.
  useLayoutEffect(() => {
    if (!isOpen) return;
    function onKeyDown(event: KeyboardEvent): void {
      if (event.key !== "Escape" || event.defaultPrevented || event.isComposing || topId !== null) return;
      event.preventDefault();
      restorePending.current = true;
      setOpen(false);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isOpen, topId]);

  return { isOpen, open, close, editorRef, rememberField };
}
