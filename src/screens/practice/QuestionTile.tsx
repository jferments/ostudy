import { useId, type KeyboardEvent, type ReactNode } from "react";
import { cn } from "../../ui/internal/cn";
import { Button } from "../../ui/primitives/Button";
import { activePointerTile, answerTile, connectedTile, tileContent, tileHandle, tileResult } from "./questionPresentation";
import type { usePracticePointerDrop } from "./usePracticePointerDrop";

type PointerDrop = ReturnType<typeof usePracticePointerDrop>;

/** Enter and Space act on the key press itself, as a tile always has. The native click
 * that would follow is canceled so the action runs once. */
export function tileKeyDown(event: KeyboardEvent<HTMLElement>, act: () => void): void {
  if (event.target !== event.currentTarget || event.nativeEvent.isComposing) return;
  if (event.key !== " " && event.key !== "Enter") return;
  event.preventDefault();
  event.stopPropagation();
  act();
}

export function tileKeyUp(event: KeyboardEvent<HTMLElement>): void {
  // Canceling Space on key-up stops the native button click that would repeat the action.
  if (event.key === " ") event.preventDefault();
}

/** A tile's content stays readable and its own links and players keep their input. A real
 * button covers the tile and is named by the action and the tile's content, so assistive
 * technology announces what the tile says. The whole tile is the drag handle for pointers.
 * A checked tile has nothing left to do and is plain content. */
export function QuestionTile({ id, action, fallbackName, content, children, pointer, disabled, locked = false,
  selected = false, connected = false, correct, onSelect, dropTargetId }: {
  id: string;
  /** The action word that starts the button's name, such as "Select" or "Move". */
  action: string;
  /** Named when the content has no text of its own, such as "item 2". */
  fallbackName?: string;
  content: ReactNode;
  /** Status and feedback under the content, read as the button's description. */
  children?: ReactNode;
  pointer: PointerDrop; disabled: boolean; locked?: boolean;
  selected?: boolean; connected?: boolean; correct?: boolean; onSelect?: () => void; dropTargetId?: string;
}) {
  const ids = useId().replaceAll(":", "");
  const actionId = `${ids}-action`;
  const contentId = `${ids}-content`;
  const detailsId = `${ids}-details`;
  const dragging = pointer.draggingItemId === id;
  const interactive = !locked;
  function select(pointerClick = false): void {
    if (!disabled && !locked && (!pointerClick || !pointer.shouldSuppressClick())) onSelect?.();
  }
  return <div data-question-item-id={id} data-practice-drop-target-id={dropTargetId}
    data-drop-disabled={disabled || locked || undefined}
    data-connected={connected || undefined} data-dragging={dragging || undefined}
    data-result={correct === undefined ? undefined : correct ? "correct" : "incorrect"}
    className={cn(answerTile, correct === undefined && connected ? connectedTile : tileResult(correct), "tw:relative tw:grid tw:min-h-14 tw:gap-2",
      interactive && !disabled && "tw:touch-none tw:select-none tw:cursor-grab",
      selected && interactive && "tw:ring-2 tw:ring-primary",
      dragging && [activePointerTile, "tw:pointer-events-none tw:z-(--ostudy-layer-drag) tw:cursor-grabbing tw:opacity-100 tw:shadow-lg tw:[&_*]:select-none"])}
    style={dragging ? { transform: `translate(${pointer.offset.x}px, ${pointer.offset.y}px)` } : undefined}
    onPointerDown={interactive ? (event) => { if (!disabled) pointer.pointerDown(event, id); } : undefined}
    onPointerMove={interactive ? pointer.pointerMove : undefined} onPointerUp={interactive ? pointer.pointerUp : undefined}
    onPointerCancel={interactive ? pointer.pointerCancel : undefined} onLostPointerCapture={interactive ? pointer.pointerCancel : undefined}
    onDragStart={(event) => { if (dragging) event.preventDefault(); }}
    onClick={interactive ? (event) => {
      // Pointer capture can deliver the click to the tile rather than its button, so the tile
      // handles every click. Its own links and players are left alone.
      const control = event.target instanceof Element ? event.target.closest("a,button,input,textarea,select,audio,video,[contenteditable=true]") : null;
      if (control && !control.hasAttribute("data-drag-handle")) return;
      select(true);
    } : undefined}>
    {interactive ? <Button variant="ghost" className={tileHandle} disabled={disabled} aria-pressed={selected}
      aria-labelledby={`${actionId} ${contentId}`} aria-describedby={children ? detailsId : undefined}
      data-drag-handle="" onKeyDown={(event) => tileKeyDown(event, select)} onKeyUp={tileKeyUp}>
      <span id={actionId} className="tw:sr-only">{fallbackName ? `${action} ${fallbackName}` : action}</span>
    </Button> : null}
    <div id={contentId} className={cn(tileContent, interactive && "tw:pointer-events-none")}>{content}</div>
    {children ? <div id={detailsId} className={cn("tw:relative tw:grid tw:min-w-0 tw:gap-2", interactive && "tw:pointer-events-none")}>{children}</div> : null}
  </div>;
}
