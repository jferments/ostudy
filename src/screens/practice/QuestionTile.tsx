import type { ReactNode } from "react";
import { cn } from "../../ui/internal/cn";
import { activePointerTile, answerTile, connectedTile, tileResult } from "./questionPresentation";
import type { usePracticePointerDrop } from "./usePracticePointerDrop";

type PointerDrop = ReturnType<typeof usePracticePointerDrop>;

/** The entire tile is the handle. Embedded rich links and players keep their own input. */
export function QuestionTile({ id, label, children, pointer, disabled, selected = false, connected = false, correct, onSelect, dropTargetId }: {
  id: string; label: string; children: ReactNode; pointer: PointerDrop; disabled: boolean;
  selected?: boolean; connected?: boolean; correct?: boolean; onSelect?: () => void; dropTargetId?: string;
}) {
  const dragging = pointer.draggingItemId === id;
  function select(pointerClick = false): void {
    if (!disabled && (!pointerClick || !pointer.shouldSuppressClick())) onSelect?.();
  }
  return <div role="button" tabIndex={disabled ? -1 : 0} aria-label={label} aria-disabled={disabled}
    aria-pressed={selected} data-question-item-id={id} data-practice-drop-target-id={dropTargetId}
    data-connected={connected || undefined} data-dragging={dragging || undefined}
    data-result={correct === undefined ? undefined : correct ? "correct" : "incorrect"}
    className={cn(answerTile, correct === undefined && connected ? connectedTile : tileResult(correct), "tw:relative tw:grid tw:min-h-14 tw:gap-2 tw:outline-none tw:focus-visible:ring-2 tw:focus-visible:ring-primary",
      !disabled && "tw:touch-none tw:select-none tw:cursor-grab",
      selected && "tw:ring-2 tw:ring-primary",
      dragging && [activePointerTile, "tw:pointer-events-none tw:z-20 tw:cursor-grabbing tw:opacity-100 tw:shadow-lg tw:[&_*]:select-none"])}
    style={dragging ? { transform: `translate(${pointer.offset.x}px, ${pointer.offset.y}px)` } : undefined}
    onPointerDown={(event) => { if (!disabled) pointer.pointerDown(event, id); }}
    onPointerMove={pointer.pointerMove} onPointerUp={pointer.pointerUp}
    onPointerCancel={pointer.pointerCancel} onLostPointerCapture={pointer.pointerCancel}
    onDragStart={(event) => { if (dragging) event.preventDefault(); }}
    onClick={(event) => {
      if (event.target instanceof Element && event.target !== event.currentTarget
        && event.target.closest("a,button,input,textarea,select,audio,video,[contenteditable=true]")) return;
      select(true);
    }}
    onKeyDown={(event) => {
      if (event.target !== event.currentTarget || event.nativeEvent.isComposing) return;
      if (event.key === " " || event.key === "Enter") { event.preventDefault(); event.stopPropagation(); select(); }
    }}>{children}</div>;
}
