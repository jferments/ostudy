import { useRef, useState, type PointerEvent as ReactPointerEvent } from "react";

interface PointerDrag {
  pointerId: number;
  itemId: string;
  startX: number;
  startY: number;
  active: boolean;
}

interface PracticePointerDropOptions {
  disabled: boolean;
  onDrop: (itemId: string, targetId: string) => void;
}

export interface PracticePointerDrop {
  draggingItemId: string | null;
  pointerDown: (event: ReactPointerEvent<HTMLElement>, itemId: string) => void;
  pointerMove: (event: ReactPointerEvent<HTMLElement>) => void;
  pointerUp: (event: ReactPointerEvent<HTMLElement>) => void;
  pointerCancel: (event: ReactPointerEvent<HTMLElement>) => void;
  shouldSuppressClick: () => boolean;
}

const DRAG_THRESHOLD_PX = 8;
const DROP_TARGET_SELECTOR = "[data-practice-drop-target-id]";

export function usePracticePointerDrop({ disabled, onDrop }: PracticePointerDropOptions): PracticePointerDrop {
  const [draggingItemId, setDraggingItemId] = useState<string | null>(null);
  const pointerRef = useRef<PointerDrag | null>(null);
  const suppressClickRef = useRef(false);

  function pointerDown(event: ReactPointerEvent<HTMLElement>, itemId: string): void {
    if (disabled || event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    pointerRef.current = {
      pointerId: event.pointerId,
      itemId,
      startX: event.clientX,
      startY: event.clientY,
      active: false,
    };
  }

  function pointerMove(event: ReactPointerEvent<HTMLElement>): void {
    const drag = pointerRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    const distance = Math.hypot(event.clientX - drag.startX, event.clientY - drag.startY);
    if (!drag.active && distance >= DRAG_THRESHOLD_PX) {
      drag.active = true;
      suppressClickRef.current = true;
      setDraggingItemId(drag.itemId);
    }
    if (drag.active) event.preventDefault();
  }

  function finish(event: ReactPointerEvent<HTMLElement>, commit: boolean): void {
    const drag = pointerRef.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    pointerRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    if (commit && drag.active) {
      const target = document.elementFromPoint(event.clientX, event.clientY)
        ?.closest<HTMLElement>(DROP_TARGET_SELECTOR);
      const targetId = target?.dataset.practiceDropTargetId;
      if (targetId) onDrop(drag.itemId, targetId);
    }
    setDraggingItemId(null);
    window.setTimeout(() => { suppressClickRef.current = false; }, 0);
  }

  return {
    draggingItemId,
    pointerDown,
    pointerMove,
    pointerUp: (event) => finish(event, true),
    pointerCancel: (event) => finish(event, false),
    shouldSuppressClick: () => suppressClickRef.current,
  };
}
