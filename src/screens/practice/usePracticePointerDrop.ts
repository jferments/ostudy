import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent, type RefObject } from "react";

interface PointerDrag {
  pointerId: number;
  itemId: string;
  resetKey: string | undefined;
  startX: number;
  startY: number;
  x: number;
  y: number;
  active: boolean;
  owner: HTMLElement;
}
interface PracticePointerDropOptions {
  disabled: boolean;
  onDrop: (itemId: string, targetId: string) => void;
  scopeRef?: RefObject<HTMLElement | null>;
  resetKey?: string;
}
const DROP_TARGET_SELECTOR = "[data-practice-drop-target-id]";
const INTERACTIVE = "a,button,input,textarea,select,audio,video,[contenteditable=true],[role=checkbox],[role=radio],[data-no-drag]";
/** The tile's own button. Pressing it starts a drag; any other control inside a tile keeps its input. */
const DRAG_HANDLE = "[data-drag-handle]";

/** Some WebViews clear document selection as a side effect of programmatic
 * focus. Preserve it until this gesture actually owns a drag; keyboard focus
 * still moves to the tile's button whose native pointer-down we prevented.
 */
function focusDragHandle(owner: HTMLElement): void {
  const selection = window.getSelection();
  const ranges = selection && !selection.isCollapsed
    ? Array.from({ length: selection.rangeCount }, (_, index) => selection.getRangeAt(index).cloneRange())
    : [];
  (owner.querySelector<HTMLElement>(DRAG_HANDLE) ?? owner).focus({ preventScroll: true });
  if (selection && ranges.length > 0) {
    selection.removeAllRanges();
    for (const range of ranges) selection.addRange(range);
  }
}

export function usePracticePointerDrop({ disabled, onDrop, scopeRef, resetKey }: PracticePointerDropOptions) {
  const latest = useRef({ disabled, onDrop, scopeRef, resetKey });
  latest.current = { disabled, onDrop, scopeRef, resetKey };
  const [draggingItemId, setDraggingItemId] = useState<string | null>(null);
  const [offset, setOffset] = useState({ x: 0, y: 0 });
  const [overTargetId, setOverTargetId] = useState<string | null>(null);
  const pointerRef = useRef<PointerDrag | null>(null);
  const suppressClickRef = useRef(false);
  const frame = useRef<number | null>(null);
  const cancelRef = useRef<() => void>(() => undefined);

  function hit(x: number, y: number): string | null {
    const scope = latest.current.scopeRef?.current;
    if (latest.current.scopeRef && !scope) return null;
    // Pointer capture retargets events to the dragged tile. Rich descendants can
    // also restore pointer-events, so never mistake the moving tile for a drop
    // target. Inspect the hit stack, scoped to this question, instead.
    const stack = typeof document.elementsFromPoint === "function"
      ? document.elementsFromPoint(x, y)
      : [document.elementFromPoint(x, y)].filter((node): node is Element => node !== null);
    const owner = pointerRef.current?.owner;
    for (const element of stack) {
      if (owner?.contains(element)) continue;
      const node = element.closest<HTMLElement>(DROP_TARGET_SELECTOR);
      if (!node || owner?.contains(node) || (scope && !scope.contains(node))) continue;
      if (node.dataset.dropDisabled === "true") continue;
      return node.dataset.practiceDropTargetId ?? null;
    }
    return null;
  }
  function release(): void {
    const drag = pointerRef.current;
    pointerRef.current = null;
    if (frame.current !== null) cancelAnimationFrame(frame.current);
    frame.current = null;
    // Capture can already be lost after a native interruption or node removal.
    // UI cleanup must still finish; it must never commit an interrupted gesture.
    try {
      if (drag?.owner.hasPointerCapture(drag.pointerId)) drag.owner.releasePointerCapture(drag.pointerId);
    } catch { /* The browser already retired this pointer/capture. */ }
  }
  function cancel(): void {
    release();
    setDraggingItemId(null);
    setOverTargetId(null);
    setOffset({ x: 0, y: 0 });
  }
  cancelRef.current = cancel;
  function scrollEdges(): void {
    const drag = pointerRef.current;
    if (!drag?.active) return;
    if (latest.current.disabled || drag.resetKey !== latest.current.resetKey || !drag.owner.isConnected) {
      cancel(); return;
    }
    let parent = drag.owner.parentElement;
    while (parent) {
      const style = getComputedStyle(parent);
      if (/(auto|scroll)/u.test(style.overflowY) && parent.scrollHeight > parent.clientHeight) {
        const box = parent.getBoundingClientRect();
        const edge = 44;
        const delta = drag.y < box.top + edge ? -10 : drag.y > box.bottom - edge ? 10 : 0;
        if (delta) parent.scrollTop += delta;
        break;
      }
      parent = parent.parentElement;
    }
    setOverTargetId(hit(drag.x, drag.y));
    frame.current = requestAnimationFrame(scrollEdges);
  }
  useEffect(() => {
    const interrupted = () => cancelRef.current();
    const visibility = () => { if (document.hidden) cancelRef.current(); };
    const escape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && pointerRef.current) {
        event.preventDefault(); event.stopPropagation(); cancelRef.current();
      }
    };
    window.addEventListener("blur", interrupted);
    window.addEventListener("keydown", escape, true);
    document.addEventListener("visibilitychange", visibility);
    return () => {
      release();
      window.removeEventListener("blur", interrupted);
      window.removeEventListener("keydown", escape, true);
      document.removeEventListener("visibilitychange", visibility);
    };
  }, []);
  useEffect(() => { if (disabled) cancelRef.current(); }, [disabled]);
  useEffect(() => { cancelRef.current(); }, [resetKey]);

  function pointerDown(event: ReactPointerEvent<HTMLElement>, itemId: string): void {
    if (disabled || event.button !== 0) return;
    if (pointerRef.current) { cancel(); return; }
    const control = event.target instanceof Element && event.target !== event.currentTarget
      ? event.target.closest(INTERACTIVE) : null;
    if (control && !control.matches(DRAG_HANDLE)) return;
    suppressClickRef.current = false;
    try { event.currentTarget.setPointerCapture(event.pointerId); }
    catch { return; }
    // Cancel native text/image dragging at its starting event, not after the
    // browser has already extended a text selection across the destination.
    // Interactive descendants returned above keep their own native behavior.
    event.preventDefault();
    focusDragHandle(event.currentTarget);
    pointerRef.current = {
      pointerId: event.pointerId, itemId, resetKey, startX: event.clientX, startY: event.clientY,
      x: event.clientX, y: event.clientY, active: false, owner: event.currentTarget,
    };
  }
  function pointerMove(event: ReactPointerEvent<HTMLElement>): void {
    const drag = pointerRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    if (latest.current.disabled || drag.resetKey !== latest.current.resetKey) { cancel(); return; }
    drag.x = event.clientX; drag.y = event.clientY;
    if (!drag.active && Math.hypot(drag.x - drag.startX, drag.y - drag.startY) >= 8) {
      drag.active = true;
      suppressClickRef.current = true;
      const selection = window.getSelection();
      const scope = latest.current.scopeRef?.current ?? drag.owner;
      // Retire an existing selection in this board only. Unrelated page text and
      // selections in embedded controls are not globally cleared on pointer-down.
      if (selection?.anchorNode && selection.focusNode
        && scope.contains(selection.anchorNode) && scope.contains(selection.focusNode)) {
        selection.removeAllRanges();
      }
      setDraggingItemId(drag.itemId);
      frame.current = requestAnimationFrame(scrollEdges);
    }
    if (drag.active) {
      event.preventDefault();
      setOffset({ x: drag.x - drag.startX, y: drag.y - drag.startY });
      setOverTargetId(hit(drag.x, drag.y));
    }
  }
  function finish(event: ReactPointerEvent<HTMLElement>, commit: boolean): void {
    const drag = pointerRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    const current = !latest.current.disabled && drag.resetKey === latest.current.resetKey && drag.owner.isConnected;
    const targetId = commit && current && drag.active ? hit(event.clientX, event.clientY) : null;
    cancel();
    if (targetId) latest.current.onDrop(drag.itemId, targetId);
    // Touch/WebView compatibility clicks can arrive after a zero-delay timer.
    // Keep suppression until a fresh pointer-down; keyboard selection bypasses it.
  }
  return {
    draggingItemId, offset, overTargetId, pointerDown, pointerMove,
    pointerUp: (event: ReactPointerEvent<HTMLElement>) => finish(event, true),
    pointerCancel: (event: ReactPointerEvent<HTMLElement>) => finish(event, false),
    shouldSuppressClick: () => suppressClickRef.current,
  };
}
